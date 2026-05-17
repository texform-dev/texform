//! Build-time logic for the LowerAttributes phase.
//!
//! This module owns the data.yaml schema, a canonical representation of
//! attribute values, validation against the builtin command registry, and
//! rendering of `generated.rs`. It is consumed by `build.rs` via
//! `#[path = ".../codegen.rs"] mod ...` and is **not** part of the runtime
//! crate (do not reference `crate::...` here).
//!
//! ## Schema deserialization style
//!
//! The `SetYaml` enum uses `#[serde(untagged)]` so that the YAML keeps the
//! ergonomic form `set: { math_font: VARIANT.BOLD }` instead of a tagged
//! `{ kind: font, value: ... }`. The trade-off is poorer error messages when a
//! `set` block does not match any variant. Since the data set is small (40
//! entries) and `validate()` below performs a stronger cross-check against the
//! builtin KB, this trade-off is acceptable.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use texform_specs::builtin::{self, BuiltinPackage};
use texform_specs::specs::{
    AllowedMode, ArgForm, BuiltinCommandRecord, CommandKind, ContentMode, ValueKind,
};

// === YAML schema ===

#[derive(Debug, serde::Deserialize)]
struct DataYaml {
    declaratives: Vec<DeclarativeYaml>,
    attribute_targets: AttributeTargetsYaml,
}

#[derive(Debug, serde::Deserialize)]
struct DeclarativeYaml {
    command: String,
    set: SetYaml,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum SetYaml {
    MathFont { math_font: String },
    MathSize { math_size: f64 },
    MathStyle { math_style: StyleValueYaml },
    TextFamily { text_family: TextFamilyYaml },
    TextSeries { text_series: TextSeriesYaml },
    TextShape { text_shape: TextShapeYaml },
    TextSize { text_size: f64 },
}

#[derive(Debug, serde::Deserialize)]
struct StyleValueYaml {
    letter: String,
    display: bool,
    level: u8,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TextFamilyYaml {
    Roman,
    #[serde(rename = "sansserif")]
    SansSerif,
    Typewriter,
    Calligraphic,
    Italic,
    Oldstyle,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TextSeriesYaml {
    Medium,
    Bold,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TextShapeYaml {
    Upright,
    Italic,
    Slanted,
    SmallCaps,
}

#[derive(Debug, serde::Deserialize)]
struct AttributeTargetsYaml {
    math_font: Vec<AttributeTargetYaml<String>>,
    math_size: Vec<AttributeTargetYaml<f64>>,
    math_style: Vec<AttributeTargetYaml<StyleValueYaml>>,
    text_family: Vec<AttributeTargetYaml<TextFamilyYaml>>,
    text_series: Vec<AttributeTargetYaml<TextSeriesYaml>>,
    text_shape: Vec<AttributeTargetYaml<TextShapeYaml>>,
    text_size: Vec<AttributeTargetYaml<f64>>,
}

#[derive(Debug, serde::Deserialize)]
struct AttributeTargetYaml<T> {
    value: T,
    math: Option<ModeTargetYaml>,
    text: Option<ModeTargetYaml>,
}

#[derive(Debug, serde::Deserialize)]
struct ModeTargetYaml {
    prefix: Option<String>,
    declarative: String,
}

// === Canonical attribute representation ===

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Attr {
    MathFont,
    MathSize,
    MathStyle,
    TextFamily,
    TextSeries,
    TextShape,
    TextSize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    MathFont(String),
    Size(i32),
    Style {
        letter: String,
        display: bool,
        level: u8,
    },
    TextFamily(TextFamilyValue),
    TextSeries(TextSeriesValue),
    TextShape(TextShapeValue),
}

impl Value {
    fn from_set(set: &SetYaml) -> Self {
        match set {
            SetYaml::MathFont { math_font } => Self::MathFont(math_font.clone()),
            SetYaml::MathSize { math_size }
            | SetYaml::TextSize {
                text_size: math_size,
            } => Self::Size(size_key(*math_size)),
            SetYaml::MathStyle {
                math_style:
                    StyleValueYaml {
                        letter,
                        display,
                        level,
                    },
            } => Self::Style {
                letter: letter.clone(),
                display: *display,
                level: *level,
            },
            SetYaml::TextFamily { text_family } => Self::TextFamily(text_family_value(text_family)),
            SetYaml::TextSeries { text_series } => Self::TextSeries(text_series_value(text_series)),
            SetYaml::TextShape { text_shape } => Self::TextShape(text_shape_value(text_shape)),
        }
    }

    /// Render this value as a Rust expression evaluating to `AttrValue::...`.
    fn code(&self) -> String {
        match self {
            Self::MathFont(value) => {
                format!("AttrValue::MathFont(MathFontValue({value:?}))")
            }
            Self::Size(value) => format!("AttrValue::Size(SizeValue({value}))"),
            Self::Style {
                letter,
                display,
                level,
            } => format!(
                "AttrValue::Style(StyleValue {{ letter: {letter:?}, display: {display}, level: {level} }})"
            ),
            Self::TextFamily(value) => format!("AttrValue::TextFamily({})", value.code()),
            Self::TextSeries(value) => format!("AttrValue::TextSeries({})", value.code()),
            Self::TextShape(value) => format!("AttrValue::TextShape({})", value.code()),
        }
    }
}

fn attr_from_set(set: &SetYaml) -> Attr {
    match set {
        SetYaml::MathFont { .. } => Attr::MathFont,
        SetYaml::MathSize { .. } => Attr::MathSize,
        SetYaml::MathStyle { .. } => Attr::MathStyle,
        SetYaml::TextFamily { .. } => Attr::TextFamily,
        SetYaml::TextSeries { .. } => Attr::TextSeries,
        SetYaml::TextShape { .. } => Attr::TextShape,
        SetYaml::TextSize { .. } => Attr::TextSize,
    }
}

fn attr_code(attr: Attr) -> &'static str {
    match attr {
        Attr::MathFont => "Attr::MathFont",
        Attr::MathSize => "Attr::MathSize",
        Attr::MathStyle => "Attr::MathStyle",
        Attr::TextFamily => "Attr::TextFamily",
        Attr::TextSeries => "Attr::TextSeries",
        Attr::TextShape => "Attr::TextShape",
        Attr::TextSize => "Attr::TextSize",
    }
}

fn attribute_set_code(attr: Attr, value: &Value) -> String {
    format!(
        "AttributeSet {{ attr: {}, value: {} }}",
        attr_code(attr),
        value.code()
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TextFamilyValue {
    Roman,
    SansSerif,
    Typewriter,
    Calligraphic,
    Italic,
    Oldstyle,
}

impl TextFamilyValue {
    fn code(self) -> &'static str {
        match self {
            Self::Roman => "TextFamily::Roman",
            Self::SansSerif => "TextFamily::SansSerif",
            Self::Typewriter => "TextFamily::Typewriter",
            Self::Calligraphic => "TextFamily::Calligraphic",
            Self::Italic => "TextFamily::Italic",
            Self::Oldstyle => "TextFamily::Oldstyle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TextSeriesValue {
    Medium,
    Bold,
}

impl TextSeriesValue {
    fn code(self) -> &'static str {
        match self {
            Self::Medium => "TextSeries::Medium",
            Self::Bold => "TextSeries::Bold",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum TextShapeValue {
    Upright,
    Italic,
    Slanted,
    SmallCaps,
}

impl TextShapeValue {
    fn code(self) -> &'static str {
        match self {
            Self::Upright => "TextShape::Upright",
            Self::Italic => "TextShape::Italic",
            Self::Slanted => "TextShape::Slanted",
            Self::SmallCaps => "TextShape::SmallCaps",
        }
    }
}

fn math_font_value(value: &str) -> Value {
    Value::MathFont(value.to_string())
}

fn size_value(value: f64) -> Value {
    Value::Size(size_key(value))
}

fn style_value(value: &StyleValueYaml) -> Value {
    Value::Style {
        letter: value.letter.clone(),
        display: value.display,
        level: value.level,
    }
}

fn text_family_value(value: &TextFamilyYaml) -> TextFamilyValue {
    match value {
        TextFamilyYaml::Roman => TextFamilyValue::Roman,
        TextFamilyYaml::SansSerif => TextFamilyValue::SansSerif,
        TextFamilyYaml::Typewriter => TextFamilyValue::Typewriter,
        TextFamilyYaml::Calligraphic => TextFamilyValue::Calligraphic,
        TextFamilyYaml::Italic => TextFamilyValue::Italic,
        TextFamilyYaml::Oldstyle => TextFamilyValue::Oldstyle,
    }
}

fn text_series_value(value: &TextSeriesYaml) -> TextSeriesValue {
    match value {
        TextSeriesYaml::Medium => TextSeriesValue::Medium,
        TextSeriesYaml::Bold => TextSeriesValue::Bold,
    }
}

fn text_shape_value(value: &TextShapeYaml) -> TextShapeValue {
    match value {
        TextShapeYaml::Upright => TextShapeValue::Upright,
        TextShapeYaml::Italic => TextShapeValue::Italic,
        TextShapeYaml::Slanted => TextShapeValue::Slanted,
        TextShapeYaml::SmallCaps => TextShapeValue::SmallCaps,
    }
}

fn size_key(value: f64) -> i32 {
    (value * 100.0).round() as i32
}

// === Command reference parsing ===

#[derive(Clone, Debug)]
struct CommandRefParts {
    package: String,
    name: String,
}

fn parse_command_ref(value: &str, context: &str) -> CommandRefParts {
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        panic!("{context} command `{value}` must have exactly one package:name separator");
    }
    CommandRefParts {
        package: parts[0].to_string(),
        name: parts[1].to_string(),
    }
}

fn find_package(name: &str) -> &'static BuiltinPackage {
    builtin::lookup_package(name)
        .unwrap_or_else(|| panic!("lower attributes package `{name}` is not in builtin specs"))
}

fn find_command(parts: &CommandRefParts, original: &str) -> &'static BuiltinCommandRecord {
    find_package(&parts.package)
        .commands
        .iter()
        .copied()
        .find(|record| record.name == parts.name)
        .unwrap_or_else(|| panic!("lower attributes command `{original}` is not in builtin specs"))
}

fn command_allowed_mode(command: &str) -> ContentMode {
    let parts = parse_command_ref(command, "lower attributes");
    let record = find_command(&parts, command);
    match record.allowed_mode {
        AllowedMode::Math => ContentMode::Math,
        AllowedMode::Text => ContentMode::Text,
        AllowedMode::Both => {
            panic!("lower attributes command `{command}` must have one allowed mode")
        }
    }
}

// === Validation ===

fn validate(data: &DataYaml) {
    let mut seen_commands = BTreeSet::new();
    let mut seen_mode_names = BTreeSet::new();
    for declarative in &data.declaratives {
        let parts = parse_command_ref(&declarative.command, "lower attributes");
        if !seen_commands.insert((parts.package.clone(), parts.name.clone())) {
            panic!(
                "duplicate lower attributes command `{}`",
                declarative.command
            );
        }
        validate_declarative_command(&declarative.command);
        let mode = command_allowed_mode(&declarative.command);
        if !seen_mode_names.insert((mode_code(mode), parts.name.clone())) {
            panic!(
                "duplicate lower attributes mode/name {:?}:{}",
                mode, parts.name
            );
        }
    }

    validate_targets(Attr::MathFont, &data.attribute_targets.math_font, |v| {
        math_font_value(v)
    });
    validate_targets(Attr::MathSize, &data.attribute_targets.math_size, |v| {
        size_value(*v)
    });
    validate_targets(
        Attr::MathStyle,
        &data.attribute_targets.math_style,
        style_value,
    );
    validate_targets(Attr::TextFamily, &data.attribute_targets.text_family, |v| {
        Value::TextFamily(text_family_value(v))
    });
    validate_targets(Attr::TextSeries, &data.attribute_targets.text_series, |v| {
        Value::TextSeries(text_series_value(v))
    });
    validate_targets(Attr::TextShape, &data.attribute_targets.text_shape, |v| {
        Value::TextShape(text_shape_value(v))
    });
    validate_targets(Attr::TextSize, &data.attribute_targets.text_size, |v| {
        size_value(*v)
    });
    validate_prefixes(data);

    for declarative in &data.declaratives {
        let attr = attr_from_set(&declarative.set);
        let value = Value::from_set(&declarative.set);
        let mode = command_allowed_mode(&declarative.command);
        let target = find_target(data, attr, &value, mode).unwrap_or_else(|| {
            panic!(
                "lower attributes command `{}` has no matching attribute target",
                declarative.command
            )
        });
        assert_eq!(
            target.declarative, declarative.command,
            "lower attributes target for `{}` must point back to the declarative command",
            declarative.command
        );
    }
}

fn validate_declarative_command(command: &str) {
    validate_command_kind(command, CommandKind::Declarative);
}

fn validate_command_kind(command: &str, expected: CommandKind) {
    let parts = parse_command_ref(command, "lower attributes");
    let record = find_command(&parts, command);
    if record.kind != expected {
        panic!(
            "lower attributes command `{command}` must be {}, got {}",
            expected.label(),
            record.kind.label()
        );
    }
}

fn validate_prefix_command(command: &str, mode: ContentMode) {
    let parts = parse_command_ref(command, "lower attributes");
    let record = find_command(&parts, command);
    if record.kind != CommandKind::Prefix {
        panic!(
            "lower attributes prefix `{command}` must be prefix, got {}",
            record.kind.label()
        );
    }
    if command_allowed_mode(command) != mode {
        panic!(
            "lower attributes prefix `{command}` allowed mode must be {:?}",
            mode
        );
    }
    let mandatory: Vec<_> = record
        .argspec
        .args
        .iter()
        .filter(|arg| {
            arg.required
                && matches!(arg.form, ArgForm::Standard)
                && matches!(arg.kind, ValueKind::Content { .. })
        })
        .collect();
    if mandatory.len() != 1 {
        panic!(
            "lower attributes prefix `{command}` must have exactly one mandatory content argument"
        );
    }
    assert_eq!(
        mandatory[0].kind.content_mode(),
        Some(mode),
        "lower attributes prefix `{command}` mandatory content mode must be {:?}",
        mode
    );
}

fn validate_mode_target(target: &ModeTargetYaml, mode: ContentMode) {
    validate_declarative_command(&target.declarative);
    if let Some(prefix) = &target.prefix {
        validate_prefix_command(prefix, mode);
    }
}

fn validate_targets<T>(
    attr: Attr,
    targets: &[AttributeTargetYaml<T>],
    to_value: impl Fn(&T) -> Value,
) {
    let mut seen = BTreeSet::new();
    for target in targets {
        let value = to_value(&target.value);
        if !seen.insert(value.clone()) {
            panic!("duplicate lower attributes {attr:?} target value {value:?}");
        }
        if let Some(math) = &target.math {
            validate_mode_target(math, ContentMode::Math);
        }
        if let Some(text) = &target.text {
            validate_mode_target(text, ContentMode::Text);
        }
    }
}

fn validate_prefixes(data: &DataYaml) {
    let mut seen = BTreeSet::new();
    for target in iter_targets(data) {
        if let Some(math) = target.math {
            validate_prefix_mode_name(math, ContentMode::Math, &mut seen);
        }
        if let Some(text) = target.text {
            validate_prefix_mode_name(text, ContentMode::Text, &mut seen);
        }
    }
}

fn validate_prefix_mode_name(
    target: &ModeTargetYaml,
    mode: ContentMode,
    seen: &mut BTreeSet<(&'static str, String)>,
) {
    let Some(prefix) = &target.prefix else {
        return;
    };
    let parts = parse_command_ref(prefix, "lower attributes prefix");
    if !seen.insert((mode_code(mode), parts.name.clone())) {
        panic!(
            "duplicate lower attributes prefix mode/name {:?}:{}",
            mode, parts.name
        );
    }
}

fn find_target<'a>(
    data: &'a DataYaml,
    attr: Attr,
    needle: &Value,
    mode: ContentMode,
) -> Option<&'a ModeTargetYaml> {
    match attr {
        Attr::MathFont => find_target_in(
            &data.attribute_targets.math_font,
            needle,
            |v| math_font_value(v),
            mode,
        ),
        Attr::MathSize => find_target_in(
            &data.attribute_targets.math_size,
            needle,
            |v| size_value(*v),
            mode,
        ),
        Attr::MathStyle => find_target_in(
            &data.attribute_targets.math_style,
            needle,
            style_value,
            mode,
        ),
        Attr::TextFamily => find_target_in(
            &data.attribute_targets.text_family,
            needle,
            |v| Value::TextFamily(text_family_value(v)),
            mode,
        ),
        Attr::TextSeries => find_target_in(
            &data.attribute_targets.text_series,
            needle,
            |v| Value::TextSeries(text_series_value(v)),
            mode,
        ),
        Attr::TextShape => find_target_in(
            &data.attribute_targets.text_shape,
            needle,
            |v| Value::TextShape(text_shape_value(v)),
            mode,
        ),
        Attr::TextSize => find_target_in(
            &data.attribute_targets.text_size,
            needle,
            |v| size_value(*v),
            mode,
        ),
    }
}

fn find_target_in<'a, T>(
    targets: &'a [AttributeTargetYaml<T>],
    needle: &Value,
    to_value: impl Fn(&T) -> Value,
    mode: ContentMode,
) -> Option<&'a ModeTargetYaml> {
    targets.iter().find_map(|target| {
        if to_value(&target.value) == *needle {
            match mode {
                ContentMode::Math => target.math.as_ref(),
                ContentMode::Text => target.text.as_ref(),
            }
        } else {
            None
        }
    })
}

struct TargetView<'a> {
    attr: Attr,
    value: Value,
    math: Option<&'a ModeTargetYaml>,
    text: Option<&'a ModeTargetYaml>,
}

fn iter_targets(data: &DataYaml) -> Vec<TargetView<'_>> {
    let mut targets = Vec::new();
    extend_targets(
        &mut targets,
        Attr::MathFont,
        &data.attribute_targets.math_font,
        |v| math_font_value(v),
    );
    extend_targets(
        &mut targets,
        Attr::MathSize,
        &data.attribute_targets.math_size,
        |v| size_value(*v),
    );
    extend_targets(
        &mut targets,
        Attr::MathStyle,
        &data.attribute_targets.math_style,
        style_value,
    );
    extend_targets(
        &mut targets,
        Attr::TextFamily,
        &data.attribute_targets.text_family,
        |v| Value::TextFamily(text_family_value(v)),
    );
    extend_targets(
        &mut targets,
        Attr::TextSeries,
        &data.attribute_targets.text_series,
        |v| Value::TextSeries(text_series_value(v)),
    );
    extend_targets(
        &mut targets,
        Attr::TextShape,
        &data.attribute_targets.text_shape,
        |v| Value::TextShape(text_shape_value(v)),
    );
    extend_targets(
        &mut targets,
        Attr::TextSize,
        &data.attribute_targets.text_size,
        |v| size_value(*v),
    );
    targets
}

fn extend_targets<'a, T>(
    out: &mut Vec<TargetView<'a>>,
    attr: Attr,
    targets: &'a [AttributeTargetYaml<T>],
    to_value: impl Fn(&T) -> Value,
) {
    out.extend(targets.iter().map(|target| TargetView {
        attr,
        value: to_value(&target.value),
        math: target.math.as_ref(),
        text: target.text.as_ref(),
    }));
}

// === Code emission ===

fn mode_code(mode: ContentMode) -> &'static str {
    match mode {
        ContentMode::Math => "ContentMode::Math",
        ContentMode::Text => "ContentMode::Text",
    }
}

fn command_ref_code(value: &str) -> String {
    let parts = parse_command_ref(value, "generated output");
    format!(
        "CommandRef {{ package: {:?}, name: {:?} }}",
        parts.package, parts.name
    )
}

fn option_command_ref_code(value: Option<&String>) -> String {
    value.map_or_else(
        || "None".to_string(),
        |command| format!("Some({})", command_ref_code(command)),
    )
}

fn mode_target_code(target: Option<&ModeTargetYaml>) -> String {
    target.map_or_else(
        || "None".to_string(),
        |target| {
            format!(
                "Some(ModeTarget {{ prefix: {}, declarative: {} }})",
                option_command_ref_code(target.prefix.as_ref()),
                command_ref_code(&target.declarative)
            )
        },
    )
}

fn render(data: &DataYaml) -> String {
    let mut code = String::new();
    writeln!(
        code,
        "// Auto-generated by build.rs - do not edit.\n\
         //\n\
         // Declarative-scope command data generated from data.yaml.\n"
    )
    .unwrap();
    writeln!(
        code,
        "use super::{{\n    Attr, AttrValue, AttributeSet, MathFontValue, SizeValue, StyleValue, TextFamily, TextSeries,\n    TextShape,\n}};"
    )
    .unwrap();
    writeln!(code, "use crate::ast::ContentMode;\n").unwrap();

    writeln!(
        code,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub(crate) struct CommandRef {{\n    \
             pub(crate) package: &'static str,\n    \
             pub(crate) name: &'static str,\n\
         }}\n"
    )
    .unwrap();
    writeln!(
        code,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub(crate) struct DeclarativeEntry {{\n    \
             pub(crate) package: &'static str,\n    \
             pub(crate) name: &'static str,\n    \
             pub(crate) key: &'static str,\n    \
             pub(crate) allowed_mode: ContentMode,\n    \
             pub(crate) set: AttributeSet,\n\
         }}\n"
    )
    .unwrap();
    writeln!(
        code,
        "#[allow(dead_code)]\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub(crate) struct PrefixEntry {{\n    \
             pub(crate) package: &'static str,\n    \
             pub(crate) name: &'static str,\n    \
             pub(crate) key: &'static str,\n    \
             pub(crate) allowed_mode: ContentMode,\n    \
             pub(crate) set: AttributeSet,\n\
         }}\n"
    )
    .unwrap();
    writeln!(
        code,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub(crate) struct ModeTarget {{\n    \
             pub(crate) prefix: Option<CommandRef>,\n    \
             pub(crate) declarative: CommandRef,\n\
         }}\n"
    )
    .unwrap();
    writeln!(
        code,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub(crate) struct AttributeTargetEntry {{\n    \
             pub(crate) attr: Attr,\n    \
             pub(crate) value: AttrValue,\n    \
             pub(crate) math: Option<ModeTarget>,\n    \
             pub(crate) text: Option<ModeTarget>,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        code,
        "#[rustfmt::skip]\n\
         pub(crate) static DECLARATIVES: &[DeclarativeEntry] = &["
    )
    .unwrap();
    for declarative in &data.declaratives {
        let parts = parse_command_ref(&declarative.command, "generated output");
        let mode = command_allowed_mode(&declarative.command);
        let attr = attr_from_set(&declarative.set);
        let value = Value::from_set(&declarative.set);
        writeln!(
            code,
            "    DeclarativeEntry {{ package: {:?}, name: {:?}, key: {:?}, allowed_mode: {}, set: {} }},",
            parts.package,
            parts.name,
            declarative.command,
            mode_code(mode),
            attribute_set_code(attr, &value)
        )
        .unwrap();
    }
    writeln!(code, "];").unwrap();

    write_prefixes(&mut code, data);
    write_targets(&mut code, data);

    code
}

fn write_prefixes(code: &mut String, data: &DataYaml) {
    writeln!(
        code,
        "#[allow(dead_code)]\n\
         #[rustfmt::skip]\n\
         pub(crate) static PREFIXES: &[PrefixEntry] = &["
    )
    .unwrap();
    for target in iter_targets(data) {
        write_prefix(
            code,
            target.attr,
            &target.value,
            target.math,
            ContentMode::Math,
        );
        write_prefix(
            code,
            target.attr,
            &target.value,
            target.text,
            ContentMode::Text,
        );
    }
    writeln!(code, "];").unwrap();
}

fn write_prefix(
    code: &mut String,
    attr: Attr,
    value: &Value,
    target: Option<&ModeTargetYaml>,
    mode: ContentMode,
) {
    let Some(prefix) = target.and_then(|target| target.prefix.as_ref()) else {
        return;
    };
    let parts = parse_command_ref(prefix, "generated output");
    writeln!(
        code,
        "    PrefixEntry {{ package: {:?}, name: {:?}, key: {:?}, allowed_mode: {}, set: {} }},",
        parts.package,
        parts.name,
        prefix,
        mode_code(mode),
        attribute_set_code(attr, value)
    )
    .unwrap();
}

fn write_targets(code: &mut String, data: &DataYaml) {
    writeln!(
        code,
        "#[rustfmt::skip]\n\
         pub(crate) static ATTRIBUTE_TARGETS: &[AttributeTargetEntry] = &["
    )
    .unwrap();
    for target in iter_targets(data) {
        writeln!(
            code,
            "    AttributeTargetEntry {{ attr: {}, value: {}, math: {}, text: {} }},",
            attr_code(target.attr),
            target.value.code(),
            mode_target_code(target.math),
            mode_target_code(target.text)
        )
        .unwrap();
    }
    writeln!(code, "];").unwrap();
}

// === Entry point ===

/// Read `data.yaml`, validate it against the builtin KB, and (re)write
/// `generated.rs`. Called once from `build.rs`.
pub(crate) fn generate(manifest_dir: &Path) {
    let base = manifest_dir.join("src/lower_attributes");
    let data_path = base.join("data.yaml");
    let codegen_path = base.join("codegen.rs");
    println!("cargo:rerun-if-changed={}", data_path.display());
    println!("cargo:rerun-if-changed={}", codegen_path.display());

    let yaml = fs::read_to_string(&data_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", data_path.display()));
    let data: DataYaml = serde_yaml::from_str(&yaml)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", data_path.display()));
    validate(&data);

    let code = render(&data);
    let out_path = base.join("generated.rs");
    let should_write = fs::read_to_string(&out_path).map_or(true, |existing| existing != code);
    if should_write {
        fs::write(&out_path, code).expect("failed to write lower_attributes/generated.rs");
    }
}
