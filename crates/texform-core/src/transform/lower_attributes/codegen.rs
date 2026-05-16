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
//! ergonomic form `set: { font: VARIANT.BOLD }` instead of a tagged
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
    Font { font: String },
    Size { size: f64 },
    Style { style: StyleValueYaml },
}

#[derive(Debug, serde::Deserialize)]
struct StyleValueYaml {
    letter: String,
    display: bool,
    level: u8,
}

#[derive(Debug, serde::Deserialize)]
struct AttributeTargetsYaml {
    font: Vec<AttributeTargetYaml<String>>,
    size: Vec<AttributeTargetYaml<f64>>,
    style: Vec<AttributeTargetYaml<StyleValueYaml>>,
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
    Font,
    Size,
    Style,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Font(String),
    Size(i32),
    Style {
        letter: String,
        display: bool,
        level: u8,
    },
}

impl Value {
    fn attr(&self) -> Attr {
        match self {
            Self::Font(_) => Attr::Font,
            Self::Size(_) => Attr::Size,
            Self::Style { .. } => Attr::Style,
        }
    }

    fn from_set(set: &SetYaml) -> Self {
        match set {
            SetYaml::Font { font } => Self::Font(font.clone()),
            SetYaml::Size { size } => Self::Size(size_key(*size)),
            SetYaml::Style {
                style:
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
        }
    }

    /// Render this value as a Rust expression evaluating to `AttrValue::...`.
    fn code(&self) -> String {
        match self {
            Self::Font(value) => format!("AttrValue::Font({value:?})"),
            Self::Size(value) => format!("AttrValue::Size(SizeValue({value}))"),
            Self::Style {
                letter,
                display,
                level,
            } => format!(
                "AttrValue::Style(StyleValue {{ letter: {letter:?}, display: {display}, level: {level} }})"
            ),
        }
    }
}

fn font_value(value: &str) -> Value {
    Value::Font(value.to_string())
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

    validate_targets(Attr::Font, &data.attribute_targets.font, |v| font_value(v));
    validate_targets(Attr::Size, &data.attribute_targets.size, |v| size_value(*v));
    validate_targets(Attr::Style, &data.attribute_targets.style, style_value);

    for declarative in &data.declaratives {
        let value = Value::from_set(&declarative.set);
        let mode = command_allowed_mode(&declarative.command);
        let target = find_target(data, &value, mode).unwrap_or_else(|| {
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
    let mandatory = record
        .argspec
        .args
        .iter()
        .find(|arg| {
            arg.required
                && matches!(arg.form, ArgForm::Standard)
                && matches!(arg.kind, ValueKind::Content { .. })
        })
        .unwrap_or_else(|| {
            panic!("lower attributes prefix `{command}` must have mandatory content")
        });
    assert_eq!(
        mandatory.kind.content_mode(),
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
        assert_eq!(value.attr(), attr, "attribute target value mismatch");
        if !seen.insert(value.clone()) {
            panic!("duplicate lower attributes attribute target value {value:?}");
        }
        if let Some(math) = &target.math {
            validate_mode_target(math, ContentMode::Math);
        }
        if let Some(text) = &target.text {
            validate_mode_target(text, ContentMode::Text);
        }
    }
}

fn find_target<'a>(
    data: &'a DataYaml,
    needle: &Value,
    mode: ContentMode,
) -> Option<&'a ModeTargetYaml> {
    match needle {
        Value::Font(_) => find_target_in(
            &data.attribute_targets.font,
            needle,
            |v| font_value(v),
            mode,
        ),
        Value::Size(_) => find_target_in(
            &data.attribute_targets.size,
            needle,
            |v| size_value(*v),
            mode,
        ),
        Value::Style { .. } => {
            find_target_in(&data.attribute_targets.style, needle, style_value, mode)
        }
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
    writeln!(code, "use super::{{AttrValue, SizeValue, StyleValue}};").unwrap();
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
             pub(crate) allowed_mode: ContentMode,\n    \
             pub(crate) set: AttrValue,\n\
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
             pub(crate) value: AttrValue,\n    \
             pub(crate) math: Option<ModeTarget>,\n    \
             pub(crate) text: Option<ModeTarget>,\n\
         }}\n"
    )
    .unwrap();

    writeln!(
        code,
        "pub(crate) static DECLARATIVES: &[DeclarativeEntry] = &["
    )
    .unwrap();
    for declarative in &data.declaratives {
        let parts = parse_command_ref(&declarative.command, "generated output");
        let mode = command_allowed_mode(&declarative.command);
        writeln!(
            code,
            "    DeclarativeEntry {{ package: {:?}, name: {:?}, allowed_mode: {}, set: {} }},",
            parts.package,
            parts.name,
            mode_code(mode),
            Value::from_set(&declarative.set).code()
        )
        .unwrap();
    }
    writeln!(code, "];\n").unwrap();

    write_targets(
        &mut code,
        "ATTRIBUTE_TARGETS_FONT",
        &data.attribute_targets.font,
        |v| font_value(v),
    );
    write_targets(
        &mut code,
        "ATTRIBUTE_TARGETS_SIZE",
        &data.attribute_targets.size,
        |v| size_value(*v),
    );
    write_targets(
        &mut code,
        "ATTRIBUTE_TARGETS_STYLE",
        &data.attribute_targets.style,
        style_value,
    );

    code
}

fn write_targets<T>(
    code: &mut String,
    name: &str,
    targets: &[AttributeTargetYaml<T>],
    to_value: impl Fn(&T) -> Value,
) {
    writeln!(
        code,
        "pub(crate) static {name}: &[AttributeTargetEntry] = &["
    )
    .unwrap();
    for target in targets {
        writeln!(
            code,
            "    AttributeTargetEntry {{ value: {}, math: {}, text: {} }},",
            to_value(&target.value).code(),
            mode_target_code(target.math.as_ref()),
            mode_target_code(target.text.as_ref())
        )
        .unwrap();
    }
    writeln!(code, "];\n").unwrap();
}

// === Entry point ===

/// Read `data.yaml`, validate it against the builtin KB, and (re)write
/// `generated.rs`. Called once from `build.rs`.
pub(crate) fn generate(manifest_dir: &Path) {
    let base = manifest_dir.join("src/transform/lower_attributes");
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
        fs::write(&out_path, code)
            .expect("failed to write transform/lower_attributes/generated.rs");
    }
}
