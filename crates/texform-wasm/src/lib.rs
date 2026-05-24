use serde::Deserialize;
use texform::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, AllowedMode, CommandItem,
    CommandKind, ContentMode, ContextItem, DelimiterControlItem, EnvironmentItem, ParseConfig,
    ParseOutput, ParserBuildError, SerializeOptions, SyntaxNode,
};
use texform::{
    FlattenGroupsConfig as CoreFlattenGroupsConfig,
    LowerAttributesConfig as CoreLowerAttributesConfig, Profile as CoreProfile,
};
use texform_argspec::{ArgForm, ArgSpec, DelimiterToken, ValueKind, parse_arg_specs};
use tsify_next::Tsify;
use wasm_bindgen::prelude::*;

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
    max_iterations: Option<usize>,
}

#[wasm_bindgen]
pub struct RewriteConfig {
    enabled: bool,
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
    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_iterations(&mut self, max_iterations: usize) {
        self.max_iterations = max_iterations;
    }
}

impl RewriteConfig {
    fn from_core(enabled: bool, max_iterations: usize) -> Self {
        Self {
            enabled,
            max_iterations,
        }
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
    preserve_group_as_argument_of_command: Option<bool>,
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
    preserve_group_as_argument_of_command: bool,
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
            preserve_group_as_argument_of_command: input
                .preserve_group_as_argument_of_command
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
    pub fn preserve_group_as_argument_of_command(&self) -> bool {
        self.preserve_group_as_argument_of_command
    }

    #[wasm_bindgen(setter)]
    pub fn set_preserve_group_as_argument_of_command(&mut self, value: bool) {
        self.preserve_group_as_argument_of_command = value;
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

fn flatten_groups_input_to_core(input: FlattenGroupsConfigInput) -> CoreFlattenGroupsConfig {
    CoreFlattenGroupsConfig {
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
        preserve_group_as_argument_of_command: input
            .preserve_group_as_argument_of_command
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
            preserve_group_as_argument_of_command: config.preserve_group_as_argument_of_command,
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
            preserve_group_as_argument_of_command: self.preserve_group_as_argument_of_command,
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

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct EngineOptions {
    packages: Option<Vec<String>>,
    items: Option<Vec<ContextItemInput>>,
    remove_commands: Option<Vec<String>>,
    remove_environments: Option<Vec<String>>,
    remove_delimiter_controls: Option<Vec<String>>,
    disable_rules: Option<Vec<String>>,
    profile: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ParserOptions {
    packages: Option<Vec<String>>,
    items: Option<Vec<ContextItemInput>>,
    remove_commands: Option<Vec<String>>,
    remove_environments: Option<Vec<String>>,
    remove_delimiter_controls: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct NormalizeOptions {
    strict: Option<bool>,
    recover: Option<bool>,
    max_group_depth: Option<usize>,
    flatten_groups: Option<FlattenGroupsConfigInput>,
    rewrite_enabled: Option<bool>,
    lower_attributes_enabled: Option<bool>,
    max_iterations: Option<usize>,
}

#[wasm_bindgen]
impl TransformConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<TransformConfig, JsValue> {
        let mut config = Self::from_profile(CoreProfile::Authoring);
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
                preserve_group_as_argument_of_command: flatten_groups
                    .preserve_group_as_argument_of_command
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
        Self::from_profile(CoreProfile::Authoring)
    }

    pub fn corpus() -> TransformConfig {
        Self::from_profile(CoreProfile::Corpus)
    }

    pub fn corpus_drop() -> TransformConfig {
        Self::from_profile(CoreProfile::CorpusDrop)
    }

    pub fn equiv() -> TransformConfig {
        Self::from_profile(CoreProfile::Equiv)
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
        RewriteConfig {
            enabled: self.rewrite.enabled,
            max_iterations: self.rewrite.max_iterations,
        }
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
    fn from_profile(profile: CoreProfile) -> Self {
        let config = profile.default_transform_config();
        Self {
            lower_attributes: LowerAttributesConfig {
                enabled: config.lower_attributes_enabled,
            },
            rewrite: RewriteConfig::from_core(config.rewrite_enabled, config.max_iterations),
            flatten_groups: FlattenGroupsConfig::from_core(config.flatten_groups),
        }
    }
}

fn profile_from_name(name: &str) -> Result<texform::Profile, JsValue> {
    match name {
        "authoring" => Ok(texform::Profile::Authoring),
        "corpus" => Ok(texform::Profile::Corpus),
        "corpus-drop" => Ok(texform::Profile::CorpusDrop),
        "equiv" => Ok(texform::Profile::Equiv),
        other => Err(JsValue::from_str(&format!(
            "unknown transform profile: {}",
            other
        ))),
    }
}

#[wasm_bindgen]
pub struct Parser {
    inner: texform::Parser,
}

#[wasm_bindgen]
impl Parser {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<Parser, JsValue> {
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<ParserOptions>(value).map_err(|error| {
                    JsValue::from_str(&format!("invalid parser options: {}", error))
                })?
            }
            _ => ParserOptions::default(),
        };
        let mut builder = match input.packages {
            Some(pkgs) => {
                let refs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
                if refs.is_empty() {
                    texform::Parser::builder().empty_knowledge()
                } else {
                    texform::Parser::builder().packages(refs.as_slice())
                }
            }
            _ => texform::Parser::builder(),
        };
        for item in input.items.unwrap_or_default() {
            builder = builder.insert_item(parse_context_item_input(item)?);
        }
        for name in input.remove_commands.unwrap_or_default() {
            builder = builder.remove_command(name);
        }
        for name in input.remove_environments.unwrap_or_default() {
            builder = builder.remove_environment(name);
        }
        for name in input.remove_delimiter_controls.unwrap_or_default() {
            builder = builder.remove_delimiter_control(name);
        }

        let inner = builder.build().map_err(parse_context_build_error_to_js)?;
        Ok(Parser { inner })
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
    }

    pub fn parse(&self, src: &str, config: Option<ParseConfigInput>) -> Result<JsValue, JsValue> {
        let output = match config {
            Some(config) => self.inner.parse_with(src, &config.into_config()),
            None => self.inner.parse(src),
        };
        parse_output_to_result(output)
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

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }
}

#[wasm_bindgen]
pub struct Engine {
    inner: texform::Engine,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(args: Option<JsValue>) -> Result<Engine, JsValue> {
        let input = match args {
            Some(value) if !value.is_null() && !value.is_undefined() => {
                serde_wasm_bindgen::from_value::<EngineOptions>(value).map_err(|error| {
                    JsValue::from_str(&format!("invalid engine options: {}", error))
                })?
            }
            _ => EngineOptions::default(),
        };
        let profile = input
            .profile
            .as_deref()
            .ok_or_else(|| JsValue::from_str("profile is required"))?;
        let mut builder = texform::Engine::builder().profile(profile_from_name(profile)?);
        if let Some(packages) = input.packages {
            let refs = packages.iter().map(String::as_str).collect::<Vec<_>>();
            builder = if refs.is_empty() {
                builder.empty_knowledge()
            } else {
                builder.packages(refs.as_slice())
            };
        }
        for item in input.items.unwrap_or_default() {
            builder = builder.item(parse_context_item_input(item)?);
        }
        for name in input.remove_commands.unwrap_or_default() {
            builder = builder.remove_command(name);
        }
        for name in input.remove_environments.unwrap_or_default() {
            builder = builder.remove_environment(name);
        }
        for name in input.remove_delimiter_controls.unwrap_or_default() {
            builder = builder.remove_delimiter_control(name);
        }
        for name in input.disable_rules.unwrap_or_default() {
            builder = builder
                .disable_rule_by_name(&name)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }
        Ok(Self {
            inner: builder
                .build()
                .map_err(|error| JsValue::from_str(&error.to_string()))?,
        })
    }

    pub fn parse(&self, src: &str, config: Option<ParseConfigInput>) -> Result<JsValue, JsValue> {
        let output = match config {
            Some(config) => self.inner.parse_with(src, &config.into_config()),
            None => self.inner.parse(src),
        };
        parse_output_to_result(output)
    }

    pub fn normalize(&self, src: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
        let Some(value) = options else {
            let result = self
                .inner
                .normalize(src)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            return Ok(normalize_result_to_js(result.normalized, &result.report));
        };
        let mut config = texform::NormalizeConfig {
            parse: ParseConfig::STRICT_NO_RECOVER,
            transform: *self.inner.default_transform_config(),
        };
        if !value.is_null() && !value.is_undefined() {
            let input =
                serde_wasm_bindgen::from_value::<NormalizeOptions>(value).map_err(|error| {
                    JsValue::from_str(&format!("invalid normalize options: {}", error))
                })?;
            if let Some(strict) = input.strict {
                config.parse.strict = strict;
            }
            if let Some(recover) = input.recover {
                config.parse.recover = recover;
            }
            if let Some(max_group_depth) = input.max_group_depth {
                config.parse.max_group_depth = max_group_depth;
            }
            if let Some(flatten_groups) = input.flatten_groups {
                config.transform.flatten_groups = flatten_groups_input_to_core(flatten_groups);
            }
            if let Some(rewrite_enabled) = input.rewrite_enabled {
                config.transform.rewrite_enabled = rewrite_enabled;
            }
            if let Some(lower_attributes_enabled) = input.lower_attributes_enabled {
                config.transform.lower_attributes_enabled = lower_attributes_enabled;
            }
            if let Some(max_iterations) = input.max_iterations {
                config.transform.max_iterations = max_iterations;
            }
        }
        let result = self
            .inner
            .normalize_with(src, &config)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(normalize_result_to_js(result.normalized, &result.report))
    }

    pub fn is_delimiter_control(&self, name: &str) -> bool {
        self.inner.is_delimiter_control(name)
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

    pub fn knows_character_name(&self, name: &str) -> bool {
        self.inner.knows_character_name(name)
    }
}

fn normalize_result_to_js(normalized: String, report: &texform::TransformReport) -> JsValue {
    let value = js_sys::Object::new();
    js_sys::Reflect::set(&value, &"normalized".into(), &normalized.into()).unwrap();
    js_sys::Reflect::set(&value, &"report".into(), &transform_report_to_js(report)).unwrap();
    value.into()
}

#[wasm_bindgen]
pub fn serialize(node: JsValue, options: Option<JsValue>) -> Result<String, JsValue> {
    let node = serde_wasm_bindgen::from_value::<SyntaxNode>(node)
        .map_err(|error| JsValue::from_str(&format!("invalid syntax node: {}", error)))?;
    let options = match options {
        Some(value) if !value.is_null() && !value.is_undefined() => {
            serde_wasm_bindgen::from_value::<SerializeOptions>(value).map_err(|error| {
                JsValue::from_str(&format!("invalid serialize options: {}", error))
            })?
        }
        _ => SerializeOptions::default(),
    };
    texform::serialize_with(&node, &options).map_err(|error| JsValue::from_str(&error.to_string()))
}

impl Parser {
    #[cfg(test)]
    fn from_options(input: ParserOptions) -> Result<Parser, JsValue> {
        let mut builder = match input.packages {
            Some(pkgs) => {
                let refs: Vec<&str> = pkgs.iter().map(String::as_str).collect();
                if refs.is_empty() {
                    texform::Parser::builder().empty_knowledge()
                } else {
                    texform::Parser::builder().packages(refs.as_slice())
                }
            }
            _ => texform::Parser::builder(),
        };
        for item in input.items.unwrap_or_default() {
            builder = builder.insert_item(parse_context_item_input(item)?);
        }

        let inner = builder.build().map_err(parse_context_build_error_to_js)?;
        Ok(Parser { inner })
    }

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

impl Engine {
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
pub fn validate_argspec(spec: &str) -> JsValue {
    let value = js_sys::Object::new();

    match parse_arg_specs(spec, "validate_argspec") {
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

fn transform_report_to_js(report: &texform::TransformReport) -> JsValue {
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
    js_sys::Reflect::set(&value, &"flatten_groups".into(), &flatten.into()).unwrap();

    value.into()
}

fn set_number(value: &js_sys::Object, key: &str, number: usize) {
    js_sys::Reflect::set(value, &key.into(), &JsValue::from_f64(number as f64)).unwrap();
}

fn format_parse_context_build_error(error: ParserBuildError) -> String {
    error.to_string()
}

fn parse_context_build_error_to_js(error: ParserBuildError) -> JsValue {
    JsValue::from_str(&format_parse_context_build_error(error))
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
    js_sys::Reflect::set(&err, &"partialResult".into(), &partial_result).unwrap();

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

    #[test]
    fn package_load_build_errors_use_facade_error_text() {
        let error = texform::Parser::builder()
            .packages(&["missing"])
            .build()
            .expect_err("missing package should fail");
        let error = format_parse_context_build_error(error);

        assert_eq!(error, "unknown package: missing");
    }

    #[test]
    fn invalid_context_item_build_errors_include_item_name() {
        let error = texform::Parser::builder()
            .empty_knowledge()
            .item(CommandItem::new(
                "foo",
                CommandKind::Prefix,
                AllowedMode::Math,
                "s:T",
            ))
            .build()
            .expect_err("invalid item should fail");
        let error = format_parse_context_build_error(error);

        assert!(error.contains("foo"));
        assert!(error.contains("invalid argspec"));
    }

    #[test]
    fn empty_package_list_is_not_treated_like_default_packages() {
        let default_ctx =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");
        let empty_packages_ctx = Parser::from_options(ParserOptions {
            packages: Some(vec![]),
            items: None,
            remove_commands: None,
            remove_environments: None,
            remove_delimiter_controls: None,
        })
        .expect("empty package list parser should build");
        let explicit_braket_ctx = Parser::from_options(ParserOptions {
            packages: Some(vec!["braket".into()]),
            items: None,
            remove_commands: None,
            remove_environments: None,
            remove_delimiter_controls: None,
        })
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
    fn parser_none_config_uses_facade_default() {
        let parser =
            Parser::from_options(ParserOptions::default()).expect("default parser should build");

        let output = parser.inner.parse(r"\unknowncmd");
        assert!(output.result.is_some());
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn lookup_command_is_mode_specific() {
        let ctx = Parser::from_options(ParserOptions {
            packages: Some(vec!["base".into(), "textmacros".into()]),
            items: None,
            remove_commands: None,
            remove_environments: None,
            remove_delimiter_controls: None,
        })
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
