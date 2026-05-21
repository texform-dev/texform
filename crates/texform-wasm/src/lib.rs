use serde::Deserialize;
use texform_argspec::{ArgForm, ArgSpec, DelimiterToken, ValueKind, parse_arg_specs};
use texform_core::api;
use texform_core::parse::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, AllowedMode, CommandItem,
    CommandKind, ContentMode, ContextItem, DelimiterControlItem, EnvironmentItem, ParseConfig,
    ParseContextBuildError, ParseOutput, ParseResult,
};
use texform_core::serialize::SerializeOptions;
use texform_core::serialize::serialize as serialize_ast;
use texform_transform::{
    FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, RewriteConfig as CoreRewriteConfig,
    RuleClassSet, RuleSelection, TransformConfig as CoreTransformConfig, run as transform_run,
};
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;

// Additional TypeScript declarations for JS-shaped API values that are not
// generated from exported wasm-bindgen signatures.
#[wasm_bindgen(typescript_custom_section)]
const WASM_API_TYPES: &str = r#"
export type ArgumentSlot = Argument | null | undefined;

export type ParseConfigInput = {
    strict?: boolean;
    recover?: boolean;
    maxGroupDepth?: number;
};

export type ParseDiagnosticKind =
    | "ambiguous-infix"
    | "argument-validation"
    | "command-mode-error"
    | "comment-truncated-argument"
    | "environment-mode-error"
    | "environment-name-mismatch"
    | "left-right-delimiter"
    | "max-group-depth-exceeded"
    | "raw-expected-found"
    | "text-script-error"
    | "unclosed-inline-math"
    | "unexpected-math-shift"
    | "unknown-command"
    | "unknown-environment";

export type ArgSpecInfo = {
    required: boolean;
    no_leading_space: boolean;
    nullable: boolean;
    kind: unknown;
    form: unknown;
};

export type CommandInfo = {
    name: string;
    kind: "prefix" | "infix" | "declarative";
    allowed_mode: "math" | "text" | "both";
    spec_string: string;
    from_packages: string[];
    tags: string[];
    args: ArgSpecInfo[];
};

export type EnvInfo = {
    name: string;
    allowed_mode: "math" | "text" | "both";
    body_mode: "math" | "text";
    spec_string: string;
    from_packages: string[];
    tags: string[];
    args: ArgSpecInfo[];
};

export type CharacterAttributesInfo = {
    mathvariant?: string;
};

export type CharacterInfo = {
    name: string;
    allowed_mode: "math" | "text" | "both";
    unicode_value: string;
    attributes: CharacterAttributesInfo;
    package: string;
};

export type ContextItem =
    | {
          target: "command";
          name: string;
          kind: "prefix" | "infix" | "declarative";
          allowed_mode: "math" | "text" | "both";
          argspec: string;
          tags?: string[];
      }
    | {
          target: "environment";
          name: string;
          allowed_mode: "math" | "text" | "both";
          body_mode: "math" | "text";
          argspec: string;
          tags?: string[];
      }
    | {
          target: "delimiter";
          name: string;
      };

export type TransformResult = {
    normalized: string;
    report: {
        iterations: number;
        applied: Array<{ key: string; count: number; skipped_count: number }>;
        lower_attributes: {
            eliminated_empty_segments: number;
        };
        flatten_groups: {
            removed_empty: number;
            replaced_single_child: number;
            inlined_multi_child: number;
            unwrapped_slot: number;
            preserved_group_containing_declarative_command: number;
            preserved_group_in_script_base_slot: number;
            preserved_group_inside_env_body: number;
            preserved_group_containing_infix: number;
            preserved_group_adjacent_to_command_like: number;
            preserved_group_after_scripted_command_like: number;
            preserved_empty_group: number;
            preserved_group_with_lone_atom_spacing_char: number;
            preserved_group_starting_with_atom_spacing_char: number;
            preserved_group_containing_delimited_pair: number;
        };
    };
};
"#;

#[wasm_bindgen(typescript_custom_section)]
const SERIALIZE_OPTION_TYPES: &str = r#"
export type CommandSpacing = "spaced" | "minimal";
export type MathGroupInnerSpacing = "padded" | "compact";
export type AdjacentCharSpacing = "spaced" | "compact";
export type ScriptSpacing = "spaced" | "compact";
export type ScriptOrder = "sub_first" | "sup_first";
export type EnvironmentNameSpacing = "spaced" | "compact";

export interface MathSpacingOptions {
    commands?: CommandSpacing;
    group_inner_spacing?: MathGroupInnerSpacing;
    adjacent_chars?: AdjacentCharSpacing;
}

export interface MathScriptOptions {
    spacing?: ScriptSpacing;
    order?: ScriptOrder;
}

export interface MathSerializeOptions {
    spacing?: MathSpacingOptions;
    scripts?: MathScriptOptions;
}

export interface EnvironmentSerializeOptions {
    name_spacing?: EnvironmentNameSpacing;
}

export interface SyntaxSerializeOptions {
    environments?: EnvironmentSerializeOptions;
}

export interface SerializeOptions {
    math?: MathSerializeOptions;
    syntax?: SyntaxSerializeOptions;
}
"#;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "target", rename_all = "lowercase")]
enum ContextItemInput {
    Command {
        name: String,
        kind: String,
        allowed_mode: String,
        argspec: String,
        tags: Option<Vec<String>>,
    },
    Environment {
        name: String,
        allowed_mode: String,
        body_mode: String,
        argspec: String,
        tags: Option<Vec<String>>,
    },
    Delimiter {
        name: String,
    },
}

#[derive(Debug, Clone, Deserialize, Default, Tsify)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase", default)]
pub struct ParseConfigInput {
    strict: Option<bool>,
    recover: Option<bool>,
    max_group_depth: Option<usize>,
}

impl ParseConfigInput {
    fn into_config(self) -> ParseConfig {
        let mut config = ParseConfig::default();
        if let Some(strict) = self.strict {
            config.strict = strict;
        }
        if let Some(recover) = self.recover {
            config.recover = recover;
        }
        if let Some(max_group_depth) = self.max_group_depth {
            config.max_group_depth = max_group_depth;
        }
        config
    }
}

fn parse_config_input(value: Option<ParseConfigInput>) -> ParseConfig {
    match value {
        Some(value) => value.into_config(),
        None => ParseConfig::default(),
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct LowerAttributesConfigInput {
    enabled: Option<bool>,
}

#[wasm_bindgen]
pub struct LowerAttributesConfig {
    enabled: bool,
}

#[wasm_bindgen]
impl LowerAttributesConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<LowerAttributesConfig, JsValue> {
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<LowerAttributesConfigInput>(value).map_err(
                    |error| {
                        JsValue::from_str(&format!("invalid lowerAttributes config: {}", error))
                    },
                )?
            }
            _ => LowerAttributesConfigInput::default(),
        };
        Ok(Self {
            enabled: input.enabled.unwrap_or(true),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[wasm_bindgen(setter)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl LowerAttributesConfig {
    fn from_core(config: CoreLowerAttributesConfig) -> Self {
        Self {
            enabled: config.enabled,
        }
    }

    fn to_core(&self) -> CoreLowerAttributesConfig {
        CoreLowerAttributesConfig {
            enabled: self.enabled,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RewriteConfigInput {
    enabled: Option<bool>,
    classes: Option<Vec<String>>,
    max_iterations: Option<usize>,
}

#[wasm_bindgen]
pub struct RewriteConfig {
    enabled: bool,
    classes: Vec<String>,
    max_iterations: usize,
}

#[wasm_bindgen]
impl RewriteConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<RewriteConfig, JsValue> {
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<RewriteConfigInput>(value).map_err(|error| {
                    JsValue::from_str(&format!("invalid rewrite config: {}", error))
                })?
            }
            _ => RewriteConfigInput::default(),
        };
        Ok(Self {
            enabled: input.enabled.unwrap_or(true),
            classes: input.classes.unwrap_or_default(),
            max_iterations: input.max_iterations.unwrap_or(100),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[wasm_bindgen(setter)]
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    #[wasm_bindgen(getter)]
    pub fn classes(&self) -> Vec<String> {
        self.classes.clone()
    }

    #[wasm_bindgen(setter)]
    pub fn set_classes(&mut self, classes: Vec<String>) {
        self.classes = classes;
    }

    #[wasm_bindgen(getter)]
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_iterations(&mut self, max_iterations: usize) {
        self.max_iterations = max_iterations;
    }
}

impl RewriteConfig {
    fn from_core(config: CoreRewriteConfig) -> Self {
        Self {
            enabled: config.enabled,
            classes: class_names(config.classes),
            max_iterations: config.max_iterations,
        }
    }

    fn to_core(&self) -> Result<CoreRewriteConfig, JsValue> {
        Ok(CoreRewriteConfig {
            enabled: self.enabled,
            classes: class_set_from_names(&self.classes)?,
            max_iterations: self.max_iterations,
            selection: RuleSelection::All,
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct FlattenGroupsConfigInput {
    enabled: Option<bool>,
    preserve_group_containing_declarative_command: Option<bool>,
    preserve_group_in_script_base_slot: Option<bool>,
    preserve_group_inside_env_body: Option<bool>,
    preserve_group_containing_infix: Option<bool>,
    preserve_group_adjacent_to_command_like: Option<bool>,
    preserve_group_after_scripted_command_like: Option<bool>,
    preserve_empty_group: Option<bool>,
    preserve_group_with_lone_atom_spacing_char: Option<bool>,
    preserve_group_starting_with_atom_spacing_char: Option<bool>,
    preserve_group_containing_delimited_pair: Option<bool>,
}

#[wasm_bindgen]
pub struct FlattenGroupsConfig {
    enabled: bool,
    preserve_group_containing_declarative_command: bool,
    preserve_group_in_script_base_slot: bool,
    preserve_group_inside_env_body: bool,
    preserve_group_containing_infix: bool,
    preserve_group_adjacent_to_command_like: bool,
    preserve_group_after_scripted_command_like: bool,
    preserve_empty_group: bool,
    preserve_group_with_lone_atom_spacing_char: bool,
    preserve_group_starting_with_atom_spacing_char: bool,
    preserve_group_containing_delimited_pair: bool,
}

#[wasm_bindgen]
impl FlattenGroupsConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<FlattenGroupsConfig, JsValue> {
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<FlattenGroupsConfigInput>(value).map_err(
                    |error| JsValue::from_str(&format!("invalid flattenGroups config: {}", error)),
                )?
            }
            _ => FlattenGroupsConfigInput::default(),
        };
        Ok(Self {
            enabled: input.enabled.unwrap_or(true),
            preserve_group_containing_declarative_command: input
                .preserve_group_containing_declarative_command
                .unwrap_or(true),
            preserve_group_in_script_base_slot: input
                .preserve_group_in_script_base_slot
                .unwrap_or(true),
            preserve_group_inside_env_body: input.preserve_group_inside_env_body.unwrap_or(true),
            preserve_group_containing_infix: input.preserve_group_containing_infix.unwrap_or(true),
            preserve_group_adjacent_to_command_like: input
                .preserve_group_adjacent_to_command_like
                .unwrap_or(true),
            preserve_group_after_scripted_command_like: input
                .preserve_group_after_scripted_command_like
                .unwrap_or(true),
            preserve_empty_group: input.preserve_empty_group.unwrap_or(true),
            preserve_group_with_lone_atom_spacing_char: input
                .preserve_group_with_lone_atom_spacing_char
                .unwrap_or(true),
            preserve_group_starting_with_atom_spacing_char: input
                .preserve_group_starting_with_atom_spacing_char
                .unwrap_or(true),
            preserve_group_containing_delimited_pair: input
                .preserve_group_containing_delimited_pair
                .unwrap_or(true),
        })
    }

    #[wasm_bindgen(getter)]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[wasm_bindgen(setter)]
    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_containing_declarative_command(&self) -> bool {
        self.preserve_group_containing_declarative_command
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_containing_declarative_command(&mut self, value: bool) {
        self.preserve_group_containing_declarative_command = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_in_script_base_slot(&self) -> bool {
        self.preserve_group_in_script_base_slot
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_in_script_base_slot(&mut self, value: bool) {
        self.preserve_group_in_script_base_slot = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_inside_env_body(&self) -> bool {
        self.preserve_group_inside_env_body
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_inside_env_body(&mut self, value: bool) {
        self.preserve_group_inside_env_body = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_containing_infix(&self) -> bool {
        self.preserve_group_containing_infix
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_containing_infix(&mut self, value: bool) {
        self.preserve_group_containing_infix = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_adjacent_to_command_like(&self) -> bool {
        self.preserve_group_adjacent_to_command_like
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_adjacent_to_command_like(&mut self, value: bool) {
        self.preserve_group_adjacent_to_command_like = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_after_scripted_command_like(&self) -> bool {
        self.preserve_group_after_scripted_command_like
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_after_scripted_command_like(&mut self, value: bool) {
        self.preserve_group_after_scripted_command_like = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_empty_group(&self) -> bool {
        self.preserve_empty_group
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_empty_group(&mut self, value: bool) {
        self.preserve_empty_group = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_with_lone_atom_spacing_char(&self) -> bool {
        self.preserve_group_with_lone_atom_spacing_char
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_with_lone_atom_spacing_char(&mut self, value: bool) {
        self.preserve_group_with_lone_atom_spacing_char = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_starting_with_atom_spacing_char(&self) -> bool {
        self.preserve_group_starting_with_atom_spacing_char
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_starting_with_atom_spacing_char(&mut self, value: bool) {
        self.preserve_group_starting_with_atom_spacing_char = value;
    }

    #[wasm_bindgen(getter)]
    pub fn preserve_group_containing_delimited_pair(&self) -> bool {
        self.preserve_group_containing_delimited_pair
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_containing_delimited_pair(&mut self, value: bool) {
        self.preserve_group_containing_delimited_pair = value;
    }
}

impl FlattenGroupsConfig {
    fn from_core(config: CoreFlattenGroupsConfig) -> Self {
        Self {
            enabled: config.enabled,
            preserve_group_containing_declarative_command: config
                .preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot: config.preserve_group_in_script_base_slot,
            preserve_group_inside_env_body: config.preserve_group_inside_env_body,
            preserve_group_containing_infix: config.preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like: config.preserve_group_adjacent_to_command_like,
            preserve_group_after_scripted_command_like: config
                .preserve_group_after_scripted_command_like,
            preserve_empty_group: config.preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char: config
                .preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char: config
                .preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair: config
                .preserve_group_containing_delimited_pair,
        }
    }

    fn to_core(&self) -> CoreFlattenGroupsConfig {
        CoreFlattenGroupsConfig {
            enabled: self.enabled,
            preserve_group_containing_declarative_command: self
                .preserve_group_containing_declarative_command,
            preserve_group_in_script_base_slot: self.preserve_group_in_script_base_slot,
            preserve_group_inside_env_body: self.preserve_group_inside_env_body,
            preserve_group_containing_infix: self.preserve_group_containing_infix,
            preserve_group_adjacent_to_command_like: self.preserve_group_adjacent_to_command_like,
            preserve_group_after_scripted_command_like: self
                .preserve_group_after_scripted_command_like,
            preserve_empty_group: self.preserve_empty_group,
            preserve_group_with_lone_atom_spacing_char: self
                .preserve_group_with_lone_atom_spacing_char,
            preserve_group_starting_with_atom_spacing_char: self
                .preserve_group_starting_with_atom_spacing_char,
            preserve_group_containing_delimited_pair: self.preserve_group_containing_delimited_pair,
        }
    }
}

#[wasm_bindgen]
pub struct TransformConfig {
    lower_attributes: LowerAttributesConfig,
    rewrite: RewriteConfig,
    flatten_groups: FlattenGroupsConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct TransformConfigInput {
    lower_attributes: Option<LowerAttributesConfigInput>,
    rewrite: Option<RewriteConfigInput>,
    flatten_groups: Option<FlattenGroupsConfigInput>,
}

#[wasm_bindgen]
impl TransformConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<TransformConfig, JsValue> {
        let mut config = Self::from_core(CoreTransformConfig::AUTHORING.clone());
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<TransformConfigInput>(value).map_err(|error| {
                    JsValue::from_str(&format!("invalid transform config: {}", error))
                })?
            }
            _ => TransformConfigInput::default(),
        };
        if let Some(lower_attributes) = input.lower_attributes {
            config.lower_attributes = LowerAttributesConfig {
                enabled: lower_attributes.enabled.unwrap_or(true),
            };
        }
        if let Some(rewrite) = input.rewrite {
            config.rewrite = RewriteConfig {
                enabled: rewrite.enabled.unwrap_or(true),
                classes: rewrite.classes.unwrap_or_default(),
                max_iterations: rewrite.max_iterations.unwrap_or(100),
            };
        }
        if let Some(flatten_groups) = input.flatten_groups {
            config.flatten_groups = FlattenGroupsConfig {
                enabled: flatten_groups.enabled.unwrap_or(true),
                preserve_group_containing_declarative_command: flatten_groups
                    .preserve_group_containing_declarative_command
                    .unwrap_or(true),
                preserve_group_in_script_base_slot: flatten_groups
                    .preserve_group_in_script_base_slot
                    .unwrap_or(true),
                preserve_group_inside_env_body: flatten_groups
                    .preserve_group_inside_env_body
                    .unwrap_or(true),
                preserve_group_containing_infix: flatten_groups
                    .preserve_group_containing_infix
                    .unwrap_or(true),
                preserve_group_adjacent_to_command_like: flatten_groups
                    .preserve_group_adjacent_to_command_like
                    .unwrap_or(true),
                preserve_group_after_scripted_command_like: flatten_groups
                    .preserve_group_after_scripted_command_like
                    .unwrap_or(true),
                preserve_empty_group: flatten_groups.preserve_empty_group.unwrap_or(true),
                preserve_group_with_lone_atom_spacing_char: flatten_groups
                    .preserve_group_with_lone_atom_spacing_char
                    .unwrap_or(true),
                preserve_group_starting_with_atom_spacing_char: flatten_groups
                    .preserve_group_starting_with_atom_spacing_char
                    .unwrap_or(true),
                preserve_group_containing_delimited_pair: flatten_groups
                    .preserve_group_containing_delimited_pair
                    .unwrap_or(true),
            };
        }
        Ok(config)
    }

    pub fn authoring() -> TransformConfig {
        Self::from_core(CoreTransformConfig::AUTHORING.clone())
    }

    pub fn corpus() -> TransformConfig {
        Self::from_core(CoreTransformConfig::CORPUS.clone())
    }

    pub fn corpus_drop() -> TransformConfig {
        Self::from_core(CoreTransformConfig::CORPUS_DROP.clone())
    }

    pub fn equiv() -> TransformConfig {
        Self::from_core(CoreTransformConfig::EQUIV.clone())
    }

    #[wasm_bindgen(getter)]
    pub fn lower_attributes(&self) -> LowerAttributesConfig {
        LowerAttributesConfig::from_core(self.lower_attributes.to_core())
    }

    #[wasm_bindgen(setter)]
    pub fn set_lower_attributes(&mut self, lower_attributes: LowerAttributesConfig) {
        self.lower_attributes = lower_attributes;
    }

    #[wasm_bindgen(getter)]
    pub fn rewrite(&self) -> RewriteConfig {
        RewriteConfig::from_core(
            self.rewrite
                .to_core()
                .expect("stored rewrite config is valid"),
        )
    }

    #[wasm_bindgen(setter)]
    pub fn set_rewrite(&mut self, rewrite: RewriteConfig) {
        self.rewrite = rewrite;
    }

    #[wasm_bindgen(getter)]
    pub fn flatten_groups(&self) -> FlattenGroupsConfig {
        FlattenGroupsConfig::from_core(self.flatten_groups.to_core())
    }

    #[wasm_bindgen(setter)]
    pub fn set_flatten_groups(&mut self, flatten_groups: FlattenGroupsConfig) {
        self.flatten_groups = flatten_groups;
    }
}

impl TransformConfig {
    fn from_core(config: CoreTransformConfig) -> Self {
        Self {
            lower_attributes: LowerAttributesConfig::from_core(config.lower_attributes),
            rewrite: RewriteConfig::from_core(config.rewrite),
            flatten_groups: FlattenGroupsConfig::from_core(config.flatten_groups),
        }
    }

    fn to_core(&self) -> Result<CoreTransformConfig, JsValue> {
        Ok(CoreTransformConfig {
            lower_attributes: self.lower_attributes.to_core(),
            rewrite: self.rewrite.to_core()?,
            flatten_groups: self.flatten_groups.to_core(),
        })
    }
}

fn class_names(classes: RuleClassSet) -> Vec<String> {
    classes
        .iter()
        .map(|class| class.as_str().to_string())
        .collect()
}

fn class_set_from_names(names: &[String]) -> Result<RuleClassSet, JsValue> {
    let mut set = RuleClassSet::empty();
    for name in names {
        let class = match name.as_str() {
            "standard" => RuleClassSet::STANDARD,
            "expand" => RuleClassSet::EXPAND,
            "drop" => RuleClassSet::DROP,
            "equiv" => RuleClassSet::EQUIV,
            other => {
                return Err(JsValue::from_str(&format!(
                    "unknown rewrite rule class: {}",
                    other
                )));
            }
        };
        set |= class;
    }
    Ok(set)
}

#[wasm_bindgen]
pub struct ParseContext {
    inner: texform_core::parse::ParseContext,
}

#[wasm_bindgen]
impl ParseContext {
    #[wasm_bindgen(constructor)]
    pub fn new(
        packages: Option<Vec<String>>,
        items: Option<JsValue>,
    ) -> Result<ParseContext, JsValue> {
        let mut builder = match packages {
            Some(pkgs) => {
                let refs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
                texform_core::parse::ParseContextBuilder::empty().packages(refs.as_slice())
            }
            _ => texform_core::parse::ParseContextBuilder::default(),
        };
        if let Some(items) = items {
            let items: Vec<ContextItemInput> = serde_wasm_bindgen::from_value(items)
                .map_err(|error| JsValue::from_str(&format!("invalid context items: {}", error)))?;
            for item in items {
                builder = builder.insert_item(parse_context_item_input(item)?);
            }
        }

        let inner = builder.build().map_err(parse_context_build_error_to_js)?;
        Ok(ParseContext { inner })
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    pub fn parse(&self, src: &str, config: Option<ParseConfigInput>) -> Result<JsValue, JsValue> {
        let config = parse_config_input(config);
        parse_output_to_result(self.inner.parse(src, &config))
    }

    pub fn serialize(
        &self,
        src: &str,
        config: Option<ParseConfigInput>,
        options: Option<JsValue>,
    ) -> Result<String, JsValue> {
        let config = parse_config_input(config);
        let output = self.inner.parse(src, &config);

        let parsed_options = match options {
            Some(opts_js) => Some(
                serde_wasm_bindgen::from_value(opts_js)
                    .map_err(|e| JsValue::from_str(&format!("invalid serialize options: {}", e)))?,
            ),
            None => None,
        };

        serialize_parse_output(output, parsed_options.as_ref())
    }

    pub fn lookup_command(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        Ok(match self.lookup_command_meta(name, mode)? {
            Some(meta) => command_meta_to_js(meta),
            None => JsValue::NULL,
        })
    }

    pub fn lookup_explicit_command(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        Ok(match self.lookup_explicit_command_meta(name, mode)? {
            Some(meta) => command_meta_to_js(meta),
            None => JsValue::NULL,
        })
    }

    pub fn lookup_character(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        Ok(match self.lookup_character_meta(name, mode)? {
            Some(meta) => character_meta_to_js(meta),
            None => JsValue::NULL,
        })
    }

    pub fn lookup_env(&self, name: &str, mode: &str) -> Result<JsValue, JsValue> {
        Ok(match self.lookup_env_meta(name, mode)? {
            Some(meta) => env_meta_to_js(meta),
            None => JsValue::NULL,
        })
    }

    pub fn knows_command_name(&self, name: &str) -> bool {
        self.inner.knows_command_name(name)
    }

    pub fn knows_env_name(&self, name: &str) -> bool {
        self.inner.knows_env_name(name)
    }
}

impl ParseContext {
    fn lookup_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_command(name, mode))
    }

    fn lookup_explicit_command_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCommandRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_explicit_command(name, mode))
    }

    fn lookup_character_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveCharacterRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_character(name, mode))
    }

    fn lookup_env_meta(
        &self,
        name: &str,
        mode: &str,
    ) -> Result<Option<&ActiveEnvironmentRecord>, JsValue> {
        let mode = parse_content_mode(mode)?;
        Ok(self.inner.lookup_env(name, mode))
    }
}

/// Parse a LaTeX formula.
///
/// Returns a JS object with `node` and `span` on success.
/// Throws an error object with `diagnostics` and `partial_result` when
/// diagnostics are present.
#[wasm_bindgen]
pub fn parse(src: &str, config: Option<ParseConfigInput>) -> Result<JsValue, JsValue> {
    let config = parse_config_input(config);
    parse_output_to_result(api::parse_latex(src, &config))
}

/// Test one or more context items by injecting them and parsing one or more inputs.
///
/// Supported targets are command, environment, and delimiter control.
#[wasm_bindgen]
pub fn parse_with_context_items(
    items: JsValue,
    inputs: Vec<String>,
    packages: Option<Vec<String>>,
    config: Option<ParseConfigInput>,
) -> Result<JsValue, JsValue> {
    let config = parse_config_input(config);
    let items: Vec<ContextItemInput> = serde_wasm_bindgen::from_value(items)
        .map_err(|error| JsValue::from_str(&format!("invalid context items: {}", error)))?;

    let input_refs: Vec<&str> = inputs.iter().map(String::as_str).collect();
    let package_refs = packages
        .as_ref()
        .map(|values| values.iter().map(String::as_str).collect::<Vec<_>>());

    let core_items: Vec<ContextItem> = items
        .into_iter()
        .map(parse_context_item_input)
        .collect::<Result<_, _>>()?;

    let output = api::parse_with_context_items(
        &core_items,
        input_refs.as_slice(),
        package_refs.as_deref(),
        &config,
    );
    parse_with_context_output_to_result(output)
}

fn parse_context_item_input(input: ContextItemInput) -> Result<ContextItem, JsValue> {
    match input {
        ContextItemInput::Command {
            name,
            kind,
            allowed_mode,
            argspec,
            tags,
        } => {
            let kind = parse_command_kind(kind.as_str())?;
            let allowed_mode = parse_allowed_mode(allowed_mode.as_str())?;
            Ok(CommandItem::new(name, kind, allowed_mode, argspec)
                .with_tags(tags.unwrap_or_default())
                .into())
        }
        ContextItemInput::Environment {
            name,
            allowed_mode,
            body_mode,
            argspec,
            tags,
        } => {
            let allowed_mode = parse_allowed_mode(allowed_mode.as_str())?;
            let body_mode = parse_content_mode(body_mode.as_str())?;
            Ok(EnvironmentItem::new(name, allowed_mode, body_mode, argspec)
                .with_tags(tags.unwrap_or_default())
                .into())
        }
        ContextItemInput::Delimiter { name } => Ok(DelimiterControlItem::new(name).into()),
    }
}

#[wasm_bindgen]
pub fn validate_spec(spec: &str) -> JsValue {
    let value = js_sys::Object::new();

    match parse_arg_specs(spec, "validate_spec") {
        Ok(args) => {
            let parsed = js_sys::Array::new();
            for arg in &args {
                parsed.push(&arg_spec_to_js(arg));
            }
            js_sys::Reflect::set(&value, &"valid".into(), &JsValue::TRUE).unwrap();
            js_sys::Reflect::set(
                &value,
                &"arg_count".into(),
                &JsValue::from_f64(args.len() as f64),
            )
            .unwrap();
            js_sys::Reflect::set(&value, &"parsed".into(), &parsed.into()).unwrap();
            js_sys::Reflect::set(&value, &"error".into(), &JsValue::NULL).unwrap();
        }
        Err(error) => {
            js_sys::Reflect::set(&value, &"valid".into(), &JsValue::FALSE).unwrap();
            js_sys::Reflect::set(&value, &"error".into(), &error.to_string().into()).unwrap();
            js_sys::Reflect::set(&value, &"parsed".into(), &JsValue::NULL).unwrap();
        }
    }

    value.into()
}

/// Parse a LaTeX formula and serialize the result back to canonical LaTeX text.
///
/// An optional `options` object may be passed to control formatting; fields
/// that are omitted default to the canonical style. Returns the serialized
/// string on success; throws when parsing produces no result at all.
#[wasm_bindgen]
pub fn serialize(
    src: &str,
    config: Option<ParseConfigInput>,
    options: Option<JsValue>,
) -> Result<String, JsValue> {
    let config = parse_config_input(config);
    let output = api::parse_latex(src, &config);

    let parsed_options = match options {
        Some(opts_js) => Some(
            serde_wasm_bindgen::from_value(opts_js)
                .map_err(|e| JsValue::from_str(&format!("invalid serialize options: {}", e)))?,
        ),
        None => None,
    };

    serialize_parse_output(output, parsed_options.as_ref())
}

#[wasm_bindgen]
pub fn transform(src: &str, config: Option<TransformConfig>) -> Result<JsValue, JsValue> {
    let ctx = texform_core::parse::ParseContext::shared();
    let config = match config {
        Some(config) => config.to_core()?,
        None => CoreTransformConfig::AUTHORING.clone(),
    };
    let parse_config = ParseConfig::STRICT_NO_RECOVER;
    let mut ast = ctx
        .parse_to_ast(src, &parse_config)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let report = transform_run(&mut ast, ctx, &config)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"normalized".into(), &serialize_ast(&ast).into()).unwrap();
    js_sys::Reflect::set(&value, &"report".into(), &transform_report_to_js(&report)).unwrap();
    Ok(value.into())
}

fn transform_report_to_js(report: &texform_transform::TransformReport) -> JsValue {
    let value = js_sys::Object::new();
    set_number(&value, "iterations", report.rewrite.iterations);

    let applied = js_sys::Array::new();
    for stat in &report.rewrite.applied {
        let item = js_sys::Object::new();
        js_sys::Reflect::set(&item, &"key".into(), &stat.key.to_string().into()).unwrap();
        set_number(&item, "count", stat.count);
        set_number(&item, "skipped_count", stat.skipped_count);
        applied.push(&item.into());
    }
    js_sys::Reflect::set(&value, &"applied".into(), &applied.into()).unwrap();

    let lower = js_sys::Object::new();
    set_number(
        &lower,
        "eliminated_empty_segments",
        report.lower_attributes.eliminated_empty_segments,
    );
    js_sys::Reflect::set(&value, &"lower_attributes".into(), &lower.into()).unwrap();

    let flatten = js_sys::Object::new();
    set_number(
        &flatten,
        "removed_empty",
        report.flatten_groups.removed_empty,
    );
    set_number(
        &flatten,
        "replaced_single_child",
        report.flatten_groups.replaced_single_child,
    );
    set_number(
        &flatten,
        "inlined_multi_child",
        report.flatten_groups.inlined_multi_child,
    );
    set_number(
        &flatten,
        "unwrapped_slot",
        report.flatten_groups.unwrapped_slot,
    );
    set_number(
        &flatten,
        "preserved_group_containing_declarative_command",
        report
            .flatten_groups
            .preserved_group_containing_declarative_command,
    );
    set_number(
        &flatten,
        "preserved_group_in_script_base_slot",
        report.flatten_groups.preserved_group_in_script_base_slot,
    );
    set_number(
        &flatten,
        "preserved_group_inside_env_body",
        report.flatten_groups.preserved_group_inside_env_body,
    );
    set_number(
        &flatten,
        "preserved_group_containing_infix",
        report.flatten_groups.preserved_group_containing_infix,
    );
    set_number(
        &flatten,
        "preserved_group_adjacent_to_command_like",
        report
            .flatten_groups
            .preserved_group_adjacent_to_command_like,
    );
    set_number(
        &flatten,
        "preserved_group_after_scripted_command_like",
        report
            .flatten_groups
            .preserved_group_after_scripted_command_like,
    );
    set_number(
        &flatten,
        "preserved_empty_group",
        report.flatten_groups.preserved_empty_group,
    );
    set_number(
        &flatten,
        "preserved_group_with_lone_atom_spacing_char",
        report
            .flatten_groups
            .preserved_group_with_lone_atom_spacing_char,
    );
    set_number(
        &flatten,
        "preserved_group_starting_with_atom_spacing_char",
        report
            .flatten_groups
            .preserved_group_starting_with_atom_spacing_char,
    );
    set_number(
        &flatten,
        "preserved_group_containing_delimited_pair",
        report
            .flatten_groups
            .preserved_group_containing_delimited_pair,
    );
    js_sys::Reflect::set(&value, &"flatten_groups".into(), &flatten.into()).unwrap();

    value.into()
}

fn set_number(value: &js_sys::Object, key: &str, number: usize) {
    js_sys::Reflect::set(value, &key.into(), &JsValue::from_f64(number as f64)).unwrap();
}

fn serialize_parse_output(
    output: ParseOutput,
    options: Option<&SerializeOptions>,
) -> Result<String, JsValue> {
    let result = match prepare_parse_output_for_serialize(&output) {
        Ok(result) => result,
        Err(SerializePrepareError::DiagnosticsPresent) => return Err(build_parse_error(&output)?),
        Err(SerializePrepareError::NoResult) => {
            return Err(JsValue::from_str("parse produced no result to serialize"));
        }
    };

    let node = &result.node;

    let result = match options {
        Some(opts) => api::serialize_latex_with(node, opts),
        None => api::serialize_latex(node),
    };

    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SerializePrepareError {
    DiagnosticsPresent,
    NoResult,
}

fn prepare_parse_output_for_serialize(
    output: &ParseOutput,
) -> Result<&ParseResult, SerializePrepareError> {
    if !output.diagnostics.is_empty() {
        return Err(SerializePrepareError::DiagnosticsPresent);
    }

    output
        .result
        .as_ref()
        .ok_or(SerializePrepareError::NoResult)
}

fn format_parse_context_build_error(error: ParseContextBuildError) -> String {
    match error {
        ParseContextBuildError::PackageLoad(error) => format!("package loading failed: {}", error),
        ParseContextBuildError::InvalidContextItem { name, source } => {
            format!("spec validation failed for {}: {}", name, source)
        }
    }
}

fn parse_context_build_error_to_js(error: ParseContextBuildError) -> JsValue {
    JsValue::from_str(&format_parse_context_build_error(error))
}

fn parse_with_context_output_to_result(
    output: api::ParseWithContextOutput,
) -> Result<JsValue, JsValue> {
    let results = js_sys::Array::new();
    for item in &output {
        results.push(&parse_output_to_batch_entry(&item.input, &item.output)?);
    }
    Ok(results.into())
}

fn parse_output_to_batch_entry(input: &str, output: &ParseOutput) -> Result<JsValue, JsValue> {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"input".into(), &input.into()).unwrap();

    if output.diagnostics.is_empty() {
        match &output.result {
            Some(result) => {
                let display = result.node.to_string();
                let js = serde_wasm_bindgen::to_value(&result)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                js_sys::Reflect::set(&js, &"display".into(), &display.clone().into()).unwrap();
                let diagnostics = js_sys::Array::new();

                js_sys::Reflect::set(&value, &"success".into(), &JsValue::TRUE).unwrap();
                js_sys::Reflect::set(&value, &"result".into(), &js).unwrap();
                js_sys::Reflect::set(&value, &"display".into(), &display.into()).unwrap();
                js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics.into()).unwrap();
                js_sys::Reflect::set(&value, &"partial_result".into(), &JsValue::NULL).unwrap();
                Ok(value.into())
            }
            None => {
                let diagnostics = js_sys::Array::new();
                js_sys::Reflect::set(&value, &"success".into(), &JsValue::FALSE).unwrap();
                js_sys::Reflect::set(&value, &"result".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"display".into(), &JsValue::NULL).unwrap();
                js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics.into()).unwrap();
                js_sys::Reflect::set(&value, &"partial_result".into(), &JsValue::NULL).unwrap();
                Ok(value.into())
            }
        }
    } else {
        let diagnostics = serde_wasm_bindgen::to_value(&output.diagnostics)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let partial_result = match &output.result {
            Some(result) => serde_wasm_bindgen::to_value(result)
                .map_err(|e| JsValue::from_str(&e.to_string()))?,
            None => JsValue::NULL,
        };

        js_sys::Reflect::set(&value, &"success".into(), &JsValue::FALSE).unwrap();
        js_sys::Reflect::set(&value, &"result".into(), &JsValue::NULL).unwrap();
        js_sys::Reflect::set(&value, &"display".into(), &JsValue::NULL).unwrap();
        js_sys::Reflect::set(&value, &"diagnostics".into(), &diagnostics).unwrap();
        js_sys::Reflect::set(&value, &"partial_result".into(), &partial_result).unwrap();

        Ok(value.into())
    }
}

fn parse_output_to_result(output: ParseOutput) -> Result<JsValue, JsValue> {
    if output.diagnostics.is_empty() {
        match output.result {
            Some(result) => {
                let display = result.node.to_string();
                let js = serde_wasm_bindgen::to_value(&result)
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                js_sys::Reflect::set(&js, &"display".into(), &display.into()).unwrap();
                Ok(js)
            }
            None => Err(JsValue::from_str(
                "parse produced no output and no diagnostics",
            )),
        }
    } else {
        Err(build_parse_error(&output)?)
    }
}

fn build_parse_error(output: &ParseOutput) -> Result<JsValue, JsValue> {
    let diagnostics = serde_wasm_bindgen::to_value(&output.diagnostics)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let partial_result = match &output.result {
        Some(r) => {
            serde_wasm_bindgen::to_value(r).map_err(|e| JsValue::from_str(&e.to_string()))?
        }
        None => JsValue::NULL,
    };

    let err = js_sys::Object::new();
    js_sys::Reflect::set(&err, &"diagnostics".into(), &diagnostics).unwrap();
    js_sys::Reflect::set(&err, &"partial_result".into(), &partial_result).unwrap();

    Ok(err.into())
}

fn parse_command_kind(value: &str) -> Result<CommandKind, JsValue> {
    match value {
        "prefix" => Ok(CommandKind::Prefix),
        "infix" => Ok(CommandKind::Infix),
        "declarative" => Ok(CommandKind::Declarative),
        _ => Err(JsValue::from_str(&format!(
            "unsupported command kind: {}",
            value
        ))),
    }
}

fn command_kind_to_string(kind: CommandKind) -> &'static str {
    match kind {
        CommandKind::Prefix => "prefix",
        CommandKind::Infix => "infix",
        CommandKind::Declarative => "declarative",
    }
}

fn parse_allowed_mode(value: &str) -> Result<AllowedMode, JsValue> {
    match value {
        "math" => Ok(AllowedMode::Math),
        "text" => Ok(AllowedMode::Text),
        "both" => Ok(AllowedMode::Both),
        _ => Err(JsValue::from_str(&format!(
            "unsupported allowed mode: {}",
            value
        ))),
    }
}

fn parse_content_mode(value: &str) -> Result<ContentMode, JsValue> {
    match value {
        "math" => Ok(ContentMode::Math),
        "text" => Ok(ContentMode::Text),
        _ => Err(JsValue::from_str(&format!(
            "unsupported content mode: {}",
            value
        ))),
    }
}

fn content_mode_to_string(mode: ContentMode) -> &'static str {
    match mode {
        ContentMode::Math => "math",
        ContentMode::Text => "text",
    }
}

fn command_meta_to_js(meta: &ActiveCommandRecord) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"name".into(), &meta.name.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"kind".into(),
        &command_kind_to_string(meta.kind).into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"allowed_mode".into(),
        &meta.allowed_mode.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"spec_string".into(), &meta.argspec.source.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"from_packages".into(),
        &string_slice_to_js_array(meta.from_packages).into(),
    )
    .unwrap();

    let tags = js_sys::Array::new();
    for &tag in meta.tags {
        tags.push(&tag.into());
    }
    js_sys::Reflect::set(&value, &"tags".into(), &tags.into()).unwrap();

    let args = js_sys::Array::new();
    for spec in meta.argspec.args {
        args.push(&arg_spec_to_js(spec));
    }
    js_sys::Reflect::set(&value, &"args".into(), &args.into()).unwrap();

    value.into()
}

fn env_meta_to_js(meta: &ActiveEnvironmentRecord) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"name".into(), &meta.name.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"allowed_mode".into(),
        &meta.allowed_mode.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"body_mode".into(),
        &content_mode_to_string(meta.body_mode).into(),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"spec_string".into(), &meta.argspec.source.into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"from_packages".into(),
        &string_slice_to_js_array(meta.from_packages).into(),
    )
    .unwrap();

    let tags = js_sys::Array::new();
    for &tag in meta.tags {
        tags.push(&tag.into());
    }
    js_sys::Reflect::set(&value, &"tags".into(), &tags.into()).unwrap();

    let args = js_sys::Array::new();
    for spec in meta.argspec.args {
        args.push(&arg_spec_to_js(spec));
    }
    js_sys::Reflect::set(&value, &"args".into(), &args.into()).unwrap();

    value.into()
}

fn character_meta_to_js(meta: &ActiveCharacterRecord) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"name".into(), &meta.name.as_str().into()).unwrap();
    js_sys::Reflect::set(
        &value,
        &"allowed_mode".into(),
        &meta.allowed_mode.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"unicode_value".into(),
        &meta.unicode_value.as_str().into(),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"attributes".into(),
        &character_attributes_to_js(meta).into(),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"package".into(), &meta.package.as_str().into()).unwrap();
    value.into()
}

fn character_attributes_to_js(meta: &ActiveCharacterRecord) -> js_sys::Object {
    let value = js_sys::Object::new();
    match meta.attributes.mathvariant.as_deref() {
        Some(mathvariant) => {
            js_sys::Reflect::set(&value, &"mathvariant".into(), &mathvariant.into()).unwrap();
        }
        None => {
            js_sys::Reflect::set(&value, &"mathvariant".into(), &JsValue::UNDEFINED).unwrap();
        }
    }
    value
}

fn string_slice_to_js_array(values: &[&str]) -> js_sys::Array {
    let array = js_sys::Array::new();
    for &value in values {
        array.push(&value.into());
    }
    array
}

fn arg_spec_to_js(spec: &ArgSpec) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(
        &value,
        &"required".into(),
        &JsValue::from_bool(spec.required),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"no_leading_space".into(),
        &JsValue::from_bool(spec.no_leading_space),
    )
    .unwrap();
    js_sys::Reflect::set(
        &value,
        &"nullable".into(),
        &JsValue::from_bool(spec.nullable),
    )
    .unwrap();
    js_sys::Reflect::set(&value, &"kind".into(), &value_kind_to_js(&spec.kind)).unwrap();
    js_sys::Reflect::set(&value, &"form".into(), &arg_form_to_js(&spec.form)).unwrap();
    value.into()
}

fn value_kind_to_js(kind: &ValueKind) -> JsValue {
    let value = js_sys::Object::new();
    match kind {
        ValueKind::Content { mode } => {
            js_sys::Reflect::set(&value, &"type".into(), &"content".into()).unwrap();
            let mode_str = match mode {
                ContentMode::Math => "math",
                ContentMode::Text => "text",
            };
            js_sys::Reflect::set(&value, &"mode".into(), &mode_str.into()).unwrap();
        }
        ValueKind::Delimiter => {
            js_sys::Reflect::set(&value, &"type".into(), &"delimiter".into()).unwrap();
        }
        ValueKind::CSName => {
            js_sys::Reflect::set(&value, &"type".into(), &"csname".into()).unwrap();
        }
        ValueKind::Dimension => {
            js_sys::Reflect::set(&value, &"type".into(), &"dimension".into()).unwrap();
        }
        ValueKind::Integer => {
            js_sys::Reflect::set(&value, &"type".into(), &"integer".into()).unwrap();
        }
        ValueKind::KeyVal => {
            js_sys::Reflect::set(&value, &"type".into(), &"keyval".into()).unwrap();
        }
        ValueKind::Column => {
            js_sys::Reflect::set(&value, &"type".into(), &"column".into()).unwrap();
        }
        ValueKind::Star => {
            js_sys::Reflect::set(&value, &"type".into(), &"star".into()).unwrap();
        }
    }
    value.into()
}

fn arg_form_to_js(form: &ArgForm) -> JsValue {
    let value = js_sys::Object::new();
    match form {
        ArgForm::Standard => {
            js_sys::Reflect::set(&value, &"type".into(), &"standard".into()).unwrap();
        }
        ArgForm::Star => {
            js_sys::Reflect::set(&value, &"type".into(), &"star".into()).unwrap();
        }
        ArgForm::Group => {
            js_sys::Reflect::set(&value, &"type".into(), &"group".into()).unwrap();
        }
        ArgForm::Delimited { open, close } => {
            js_sys::Reflect::set(&value, &"type".into(), &"delimited".into()).unwrap();
            js_sys::Reflect::set(&value, &"open".into(), &delimiter_token_to_js(open)).unwrap();
            js_sys::Reflect::set(&value, &"close".into(), &delimiter_token_to_js(close)).unwrap();
        }
        ArgForm::Paired { pairs } => {
            js_sys::Reflect::set(&value, &"type".into(), &"paired".into()).unwrap();
            let pairs_value = js_sys::Array::new();
            for (open, close) in pairs.iter() {
                let pair = js_sys::Object::new();
                js_sys::Reflect::set(&pair, &"open".into(), &delimiter_token_to_js(open)).unwrap();
                js_sys::Reflect::set(&pair, &"close".into(), &delimiter_token_to_js(close))
                    .unwrap();
                pairs_value.push(&pair.into());
            }
            js_sys::Reflect::set(&value, &"pairs".into(), &pairs_value.into()).unwrap();
        }
    }
    value.into()
}

fn delimiter_token_to_js(token: &DelimiterToken) -> JsValue {
    let value = js_sys::Object::new();
    match token {
        DelimiterToken::Char(ch) => {
            js_sys::Reflect::set(&value, &"type".into(), &"char".into()).unwrap();
            js_sys::Reflect::set(&value, &"value".into(), &ch.to_string().into()).unwrap();
        }
        DelimiterToken::ControlSeq(name) => {
            js_sys::Reflect::set(&value, &"type".into(), &"control-seq".into()).unwrap();
            js_sys::Reflect::set(&value, &"value".into(), &name.as_ref().into()).unwrap();
        }
    }
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use texform_core::parse::{PackageLoadError, ParseContextBuildError};

    fn custom_command_context() -> ParseContext {
        ParseContext {
            inner: texform_core::parse::ParseContextBuilder::default()
                .insert_item(CommandItem::new(
                    "probe",
                    CommandKind::Prefix,
                    AllowedMode::Math,
                    "m",
                ))
                .build()
                .expect("custom parse context should build"),
        }
    }

    #[test]
    fn serialize_rejects_diagnostics_bearing_parse_results() {
        let config = ParseConfig::STRICT_NO_RECOVER;
        let output = api::parse_latex(r"\unknowncmd", &config);

        let err = prepare_parse_output_for_serialize(&output)
            .expect_err("should reject diagnostics-bearing parse results");
        assert_eq!(err, SerializePrepareError::DiagnosticsPresent);
    }

    #[test]
    fn package_load_build_errors_keep_package_loading_prefix() {
        let error = format_parse_context_build_error(ParseContextBuildError::PackageLoad(
            PackageLoadError::UnknownPackage {
                name: "missing".to_string(),
            },
        ));

        assert_eq!(error, "package loading failed: unknown package: missing");
    }

    #[test]
    fn invalid_context_item_build_errors_include_item_name() {
        let error = format_parse_context_build_error(ParseContextBuildError::InvalidContextItem {
            name: "foo".to_string(),
            source: texform_core::parse::ArgSpecParseError {
                context: "foo".to_string(),
                char_index: 0,
                message: "expected argument kind".to_string(),
            },
        });

        assert_eq!(
            error,
            "spec validation failed for foo: invalid argspec (foo) at char 0: expected argument kind"
        );
    }

    #[test]
    fn empty_package_list_is_not_treated_like_default_packages() {
        let default_ctx =
            ParseContext::new(None, None).expect("default parse context should build");
        let empty_packages_ctx = ParseContext::new(Some(vec![]), None)
            .expect("empty package list parse context should build");
        let explicit_braket_ctx = ParseContext::new(Some(vec!["braket".into()]), None)
            .expect("explicit braket parse context should build");

        assert!(
            default_ctx
                .inner
                .lookup_command("frac", ContentMode::Math)
                .is_some()
        );
        assert!(
            default_ctx
                .inner
                .lookup_command("Bra", ContentMode::Math)
                .is_none()
        );
        assert!(
            empty_packages_ctx
                .inner
                .lookup_command("frac", ContentMode::Math)
                .is_none()
        );
        assert!(
            explicit_braket_ctx
                .inner
                .lookup_command("Bra", ContentMode::Math)
                .is_some()
        );
    }

    #[test]
    fn parse_context_serialize_uses_context_items() {
        let ctx = custom_command_context();
        let config = ParseConfig::STRICT_NO_RECOVER;
        let global_output = api::parse_latex(r"\probe{x}", &config);

        let global_err = prepare_parse_output_for_serialize(&global_output)
            .expect_err("global parse path should reject the custom command");
        assert_eq!(global_err, SerializePrepareError::DiagnosticsPresent);

        let output = ctx
            .serialize(r"\probe{x}", None, None)
            .expect("custom command should serialize with instance context");

        assert_eq!(output, r"\probe { x }");
    }

    #[test]
    fn lookup_command_is_mode_specific() {
        let ctx = ParseContext::new(Some(vec!["base".into(), "textmacros".into()]), None)
            .expect("parse context should build");

        let math = ctx
            .lookup_command_meta("underline", "math")
            .expect("math lookup should succeed")
            .expect("underline should be known in math mode");
        let text = ctx
            .lookup_command_meta("underline", "text")
            .expect("text lookup should succeed")
            .expect("underline should be known in text mode");

        assert_eq!(math.argspec.source, "m");
        assert_eq!(text.argspec.source, "m:T");
        assert!(ctx.knows_command_name("underline"));
    }
}
