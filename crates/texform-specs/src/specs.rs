//! Shared spec types.
//!
//! This crate hosts:
//! - `PackageSpecs`: parsed YAML package specs (owned, merge-ready)
//! - Knowledge metadata types (`CommandMeta`, `EnvMeta`, ...)
//!
//! For rapid prototyping, configuration errors fail fast (panic).

use std::borrow::Cow;

use serde::Deserialize;
pub use texform_interface::syntax_node::ContentMode;

/// Command type in knowledge base (determines AST node type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Prefix command → creates Command node
    /// Arguments follow the command
    Prefix,

    /// Infix command → creates InfixCommand node
    /// Left and right operands collected from context
    Infix,

    /// Declarative command → creates DeclarativeCommand node
    /// Scope collected from context (command to end of group)
    Declarative,
}

impl CommandKind {
    /// Return a human-readable label for error messages.
    pub const fn label(&self) -> &'static str {
        match self {
            CommandKind::Prefix => "prefix",
            CommandKind::Infix => "infix",
            CommandKind::Declarative => "declarative",
        }
    }
}

/// Allowed invocation mode for commands/environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedMode {
    /// Can only be invoked in math mode.
    Math,
    /// Can only be invoked in text mode.
    Text,
    /// Can be invoked in both math and text mode.
    Both,
}

impl AllowedMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            AllowedMode::Math => "math",
            AllowedMode::Text => "text",
            AllowedMode::Both => "both",
        }
    }

    pub const fn allows(self, mode: ContentMode) -> bool {
        match self {
            AllowedMode::Both => true,
            AllowedMode::Math => matches!(mode, ContentMode::Math),
            AllowedMode::Text => matches!(mode, ContentMode::Text),
        }
    }
}

impl std::fmt::Display for AllowedMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

/// Delimiter token used by delimited/paired argument forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelimiterToken {
    /// Single-character delimiter, e.g. `(`, `)`, `|`.
    Char(char),
    /// Control-sequence delimiter name without the leading backslash, e.g. `langle`.
    ControlSeq(Cow<'static, str>),
}

/// Argument parsing form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgForm {
    /// Standard xparse-like argument form (`m`/`o`).
    Standard,
    /// Optional star form (`s`).
    Star,
    /// Braced group form (`g` or `m{}`).
    ///
    /// The name follows xparse's `g` token ("group").
    /// Semantically this always reads a `{...}` group, but requiredness is
    /// controlled by the surrounding [`ArgSpec`]:
    /// - `g` => optional group slot
    /// - `m{}` => required group slot
    ///
    /// MathJax currently uses this as optional braced content in all known
    /// cases, but we intentionally keep value-kind binding open (except `Star`)
    /// to reserve room for future non-content uses.
    Group,
    /// Single delimited form (`r`/`d`).
    Delimited {
        open: DelimiterToken,
        close: DelimiterToken,
    },
    /// Multi-pair candidate form (`r`/`d` with `<l,r>` pair list syntax).
    Paired {
        pairs: Cow<'static, [(DelimiterToken, DelimiterToken)]>,
    },
}

/// Argument value kind (parsing strategy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// Content argument parsed recursively (math/text mode)
    Content { mode: ContentMode },
    /// Single delimiter token (including '.' for empty)
    Delimiter,
    /// Control-sequence name string without any escape/control sequences
    CSName,
    /// Dimension / length value (e.g., 1em, -2pt)
    Dimension,
    /// Integer value (e.g., 2, -10)
    Integer,
    /// Key=Value list (validated format, stored as raw string)
    KeyVal,
    /// Array column template argument
    Column,
    /// Star presence flag.
    Star,
}

impl ValueKind {
    pub const fn is_content(&self) -> bool {
        matches!(self, ValueKind::Content { .. })
    }

    pub const fn is_delimiter(&self) -> bool {
        matches!(self, ValueKind::Delimiter)
    }

    pub const fn is_cs_name(&self) -> bool {
        matches!(self, ValueKind::CSName)
    }

    pub const fn is_dimension(&self) -> bool {
        matches!(self, ValueKind::Dimension)
    }

    pub const fn is_integer(&self) -> bool {
        matches!(self, ValueKind::Integer)
    }

    pub const fn is_keyval(&self) -> bool {
        matches!(self, ValueKind::KeyVal)
    }

    pub const fn is_column(&self) -> bool {
        matches!(self, ValueKind::Column)
    }

    pub const fn is_star(&self) -> bool {
        matches!(self, ValueKind::Star)
    }

    pub const fn content_mode(&self) -> Option<ContentMode> {
        match self {
            ValueKind::Content { mode } => Some(*mode),
            _ => None,
        }
    }
}

/// Argument specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgSpec {
    /// Whether the argument is required (true) or optional (false).
    pub required: bool,

    /// xparse-like `!` prefix semantics.
    pub no_leading_space: bool,

    /// Argument value kind (parsing strategy)
    pub kind: ValueKind,

    /// How the parser should read this argument.
    pub form: ArgForm,
}

impl ArgSpec {
    pub const fn new(required: bool, kind: ValueKind) -> Self {
        ArgSpec {
            required,
            no_leading_space: false,
            kind,
            form: ArgForm::Standard,
        }
    }

    pub fn with_form(
        required: bool,
        no_leading_space: bool,
        kind: ValueKind,
        form: ArgForm,
    ) -> Self {
        ArgSpec {
            required,
            no_leading_space,
            kind,
            form,
        }
    }

    /// Create a mandatory content argument spec (`m`).
    pub const fn mandatory(mode: ContentMode) -> Self {
        ArgSpec {
            required: true,
            no_leading_space: false,
            kind: ValueKind::Content { mode },
            form: ArgForm::Standard,
        }
    }

    /// Create an optional content argument spec (`o`).
    pub const fn optional(mode: ContentMode) -> Self {
        ArgSpec {
            required: false,
            no_leading_space: false,
            kind: ValueKind::Content { mode },
            form: ArgForm::Standard,
        }
    }

    pub const fn is_required(&self) -> bool {
        self.required
    }

    pub const fn is_optional(&self) -> bool {
        !self.required
    }
}

/// Command metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMeta {
    /// Command name (without backslash)
    pub name: &'static str,

    /// Command type (determines which AST node type to create)
    pub kind: CommandKind,

    /// Allowed invocation mode.
    pub allowed_mode: AllowedMode,

    /// Argument specifications
    /// - For Prefix: all arguments
    /// - For Infix: command's own args (usually empty), left/right collected separately
    /// - For Declarative: command's own args, scope collected separately
    pub args: &'static [ArgSpec],

    /// Metadata tags (kebab-case)
    pub tags: &'static [&'static str],

    /// Original xparse-style spec string from package definition.
    pub spec_string: &'static str,

    /// Package name that provided this command metadata.
    pub package: &'static str,
}

/// Environment metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvMeta {
    /// Environment name (without \begin/\end)
    pub name: &'static str,

    /// Allowed invocation mode.
    pub allowed_mode: AllowedMode,

    /// Argument specifications
    pub args: &'static [ArgSpec],

    /// Content mode for environment body
    pub body_mode: ContentMode,

    /// Metadata tags (kebab-case)
    pub tags: &'static [&'static str],

    /// Original xparse-style spec string from package definition.
    pub spec_string: &'static str,

    /// Package name that provided this environment metadata.
    pub package: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub kind: CommandKind,
    pub allowed_mode: AllowedMode,
    pub args: Vec<ArgSpec>,
    pub tags: Vec<String>,
    pub spec_string: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvironmentSpec {
    pub name: String,
    pub allowed_mode: AllowedMode,
    pub args: Vec<ArgSpec>,
    pub body_mode: ContentMode,
    pub tags: Vec<String>,
    pub spec_string: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterSpec {
    pub name: String,
    pub allowed_mode: AllowedMode,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PackageSpecs {
    pub characters: Vec<CharacterSpec>,
    pub commands: Vec<CommandSpec>,
    pub environments: Vec<EnvironmentSpec>,
    pub delimiter_controls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgSpecParseError {
    pub context: String,
    pub char_index: usize,
    pub message: String,
}

impl ArgSpecParseError {
    fn new(context: &str, char_index: usize, message: impl Into<String>) -> Self {
        Self {
            context: context.to_string(),
            char_index,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ArgSpecParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid argspec ({}) at char {}: {}",
            self.context, self.char_index, self.message
        )
    }
}

impl std::error::Error for ArgSpecParseError {}

pub fn load_package_specs_from_str(yaml: &str, context: &str) -> PackageSpecs {
    let parsed: PackageSpecsYaml = serde_yaml::from_str(yaml)
        .unwrap_or_else(|e| panic!("failed to parse package specs ({context}): {e}"));
    parsed.into_specs()
}

pub fn parse_arg_specs(spec: &str, context: &str) -> Result<Vec<ArgSpec>, ArgSpecParseError> {
    ArgSpecParser::new(spec, context).parse()
}

#[derive(Debug, Default, Deserialize)]
struct PackageSpecsYaml {
    #[serde(default)]
    characters: Vec<CharacterSpecYaml>,
    #[serde(default)]
    commands: Vec<CommandSpecYaml>,
    #[serde(default)]
    environments: Vec<EnvironmentSpecYaml>,
    #[serde(default)]
    delimiter_controls: Vec<String>,
}

impl PackageSpecsYaml {
    fn into_specs(self) -> PackageSpecs {
        PackageSpecs {
            characters: self.characters.into_iter().map(|c| c.into()).collect(),
            commands: self.commands.into_iter().map(|c| c.into()).collect(),
            environments: self.environments.into_iter().map(|e| e.into()).collect(),
            delimiter_controls: self.delimiter_controls,
        }
    }
}

#[derive(Debug, Deserialize)]
struct CharacterSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
}

impl From<CharacterSpecYaml> for CharacterSpec {
    fn from(value: CharacterSpecYaml) -> Self {
        CharacterSpec {
            name: value.name,
            allowed_mode: value.allowed_mode.into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommandSpecYaml {
    name: String,
    kind: CommandKindYaml,
    #[serde(default)]
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    spec: String,
    #[serde(default)]
    tags: Vec<String>,
}

impl From<CommandSpecYaml> for CommandSpec {
    fn from(value: CommandSpecYaml) -> Self {
        let context = format!("command {}", value.name.as_str());
        let args = parse_arg_specs(&value.spec, context.as_str())
            .unwrap_or_else(|error| panic!("{error}"));

        CommandSpec {
            name: value.name,
            kind: value.kind.into(),
            allowed_mode: value.allowed_mode.into(),
            args,
            tags: value.tags,
            spec_string: value.spec,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CommandKindYaml {
    Prefix,
    Infix,
    Declarative,
}

impl From<CommandKindYaml> for CommandKind {
    fn from(value: CommandKindYaml) -> Self {
        match value {
            CommandKindYaml::Prefix => CommandKind::Prefix,
            CommandKindYaml::Infix => CommandKind::Infix,
            CommandKindYaml::Declarative => CommandKind::Declarative,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum AllowedModeYaml {
    Math,
    Text,
    #[default]
    Both,
}

impl From<AllowedModeYaml> for AllowedMode {
    fn from(value: AllowedModeYaml) -> Self {
        match value {
            AllowedModeYaml::Math => AllowedMode::Math,
            AllowedModeYaml::Text => AllowedMode::Text,
            AllowedModeYaml::Both => AllowedMode::Both,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EnvironmentSpecYaml {
    name: String,
    allowed_mode: AllowedModeYaml,
    #[serde(default)]
    spec: String,
    body_mode: ContentModeYaml,
    #[serde(default)]
    tags: Vec<String>,
}

impl From<EnvironmentSpecYaml> for EnvironmentSpec {
    fn from(value: EnvironmentSpecYaml) -> Self {
        let context = format!("environment {}", value.name.as_str());
        let args = parse_arg_specs(&value.spec, context.as_str())
            .unwrap_or_else(|error| panic!("{error}"));

        EnvironmentSpec {
            name: value.name,
            allowed_mode: value.allowed_mode.into(),
            args,
            body_mode: value.body_mode.into(),
            tags: value.tags,
            spec_string: value.spec,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ContentModeYaml {
    Math,
    Text,
}

impl From<ContentModeYaml> for ContentMode {
    fn from(value: ContentModeYaml) -> Self {
        match value {
            ContentModeYaml::Math => ContentMode::Math,
            ContentModeYaml::Text => ContentMode::Text,
        }
    }
}

struct ArgSpecParser<'a> {
    chars: Vec<char>,
    cursor: usize,
    context: &'a str,
}

impl<'a> ArgSpecParser<'a> {
    fn new(spec: &str, context: &'a str) -> Self {
        ArgSpecParser {
            chars: spec.chars().collect(),
            cursor: 0,
            context,
        }
    }

    fn parse(mut self) -> Result<Vec<ArgSpec>, ArgSpecParseError> {
        let mut specs = Vec::new();

        loop {
            self.skip_whitespace();
            if self.eof() {
                break;
            }
            specs.push(self.parse_one()?);
        }

        Ok(specs)
    }

    fn parse_one(&mut self) -> Result<ArgSpec, ArgSpecParseError> {
        let no_leading_space = self.consume_if('!');
        let kind_token = self
            .next_char()
            .ok_or_else(|| self.err("expected argument token"))?;

        let (required, form, has_ignored_default) = match kind_token {
            'm' => (true, self.parse_mandatory_form()?, false),
            'o' => (false, ArgForm::Standard, false),
            'O' => (false, ArgForm::Standard, true),
            's' => (false, ArgForm::Star, false),
            'g' => (false, ArgForm::Group, false),
            'G' => (false, ArgForm::Group, true),
            'r' => {
                if self.peek_char() == Some('<') {
                    let pairs = self.parse_pair_list()?;
                    (true, ArgForm::Paired { pairs }, false)
                } else {
                    let open = self.parse_delimiter_token()?;
                    let close = self.parse_delimiter_token()?;
                    (true, ArgForm::Delimited { open, close }, false)
                }
            }
            'R' => {
                if self.peek_char() == Some('<') {
                    let pairs = self.parse_pair_list()?;
                    (true, ArgForm::Paired { pairs }, true)
                } else {
                    let open = self.parse_delimiter_token()?;
                    let close = self.parse_delimiter_token()?;
                    (true, ArgForm::Delimited { open, close }, true)
                }
            }
            'd' => {
                if self.peek_char() == Some('<') {
                    let pairs = self.parse_pair_list()?;
                    (false, ArgForm::Paired { pairs }, false)
                } else {
                    let open = self.parse_delimiter_token()?;
                    let close = self.parse_delimiter_token()?;
                    (false, ArgForm::Delimited { open, close }, false)
                }
            }
            'D' => {
                if self.peek_char() == Some('<') {
                    let pairs = self.parse_pair_list()?;
                    (false, ArgForm::Paired { pairs }, true)
                } else {
                    let open = self.parse_delimiter_token()?;
                    let close = self.parse_delimiter_token()?;
                    (false, ArgForm::Delimited { open, close }, true)
                }
            }
            other => {
                return Err(self.err(format!("unsupported argument token `{other}`")));
            }
        };

        if has_ignored_default {
            self.parse_ignored_default_block(kind_token)?;
        }

        let kind = if matches!(&form, ArgForm::Star) {
            if self.peek_char() == Some(':') {
                return Err(self.err("`s` does not accept value type annotation"));
            }
            ValueKind::Star
        } else {
            self.parse_value_kind_annotation()?
        };

        let spec = ArgSpec {
            required,
            no_leading_space,
            kind,
            form,
        };
        self.validate_spec(spec)
    }

    fn parse_mandatory_form(&mut self) -> Result<ArgForm, ArgSpecParseError> {
        if !self.consume_if('{') {
            return Ok(ArgForm::Standard);
        }

        if !self.consume_if('}') {
            return Err(self.err("`m` only supports required braced group syntax `m{}`"));
        }

        Ok(ArgForm::Group)
    }

    fn parse_ignored_default_block(&mut self, token: char) -> Result<(), ArgSpecParseError> {
        if !self.consume_if('{') {
            return Err(self.err(format!("`{token}` requires a default block like `{{...}}`")));
        }

        let mut brace_depth = 1usize;
        while let Some(ch) = self.next_char() {
            match ch {
                '\\' => {
                    if self.peek_char().is_some() {
                        self.cursor += 1;
                    }
                }
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth -= 1;
                    if brace_depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }

        Err(self.err(format!("unterminated default block for `{token}`")))
    }

    fn parse_value_kind_annotation(&mut self) -> Result<ValueKind, ArgSpecParseError> {
        if !self.consume_if(':') {
            return Ok(ValueKind::Content {
                mode: ContentMode::Math,
            });
        }

        let annotation = self
            .next_char()
            .ok_or_else(|| self.err("missing value kind annotation after `:`"))?;
        match annotation {
            'T' => Ok(ValueKind::Content {
                mode: ContentMode::Text,
            }),
            'D' => Ok(ValueKind::Delimiter),
            'N' => Ok(ValueKind::CSName),
            'L' => Ok(ValueKind::Dimension),
            'I' => Ok(ValueKind::Integer),
            'K' => Ok(ValueKind::KeyVal),
            'C' => Ok(ValueKind::Column),
            other => Err(self.err(format!("unsupported value kind annotation `:{other}`"))),
        }
    }

    fn parse_delimiter_token(&mut self) -> Result<DelimiterToken, ArgSpecParseError> {
        match self.next_char() {
            Some('\\') => Ok(DelimiterToken::ControlSeq(Cow::Owned(
                self.parse_control_sequence_name()?,
            ))),
            Some(c) if c.is_whitespace() => Err(self.err("delimiter token cannot be whitespace")),
            Some(c) => Ok(DelimiterToken::Char(c)),
            None => Err(self.err("missing delimiter token")),
        }
    }

    fn parse_pair_list(
        &mut self,
    ) -> Result<Cow<'static, [(DelimiterToken, DelimiterToken)]>, ArgSpecParseError> {
        let mut pairs = Vec::new();

        while self.consume_if('<') {
            let open = self.parse_pair_delimiter_token()?;
            self.expect_char(',')?;
            let close = self.parse_pair_delimiter_token()?;
            self.expect_char('>')?;
            pairs.push((open, close));
        }

        if pairs.is_empty() {
            return Err(self.err("paired form requires at least one `<open,close>` block"));
        }

        Ok(Cow::Owned(pairs))
    }

    fn parse_pair_delimiter_token(&mut self) -> Result<DelimiterToken, ArgSpecParseError> {
        match self.next_char() {
            Some('\\') => Ok(DelimiterToken::ControlSeq(Cow::Owned(
                self.parse_control_sequence_name()?,
            ))),
            Some(c) if c.is_whitespace() => Err(self.err("pair delimiter cannot be whitespace")),
            Some('<') | Some('>') | Some(',') => {
                Err(self.err("`<`, `>`, `,` are reserved in pair syntax"))
            }
            Some(c) => Ok(DelimiterToken::Char(c)),
            None => Err(self.err("missing pair delimiter token")),
        }
    }

    fn parse_control_sequence_name(&mut self) -> Result<String, ArgSpecParseError> {
        let first = self
            .next_char()
            .ok_or_else(|| self.err("expected control sequence name after `\\`"))?;

        let mut name = String::new();
        name.push(first);

        if first.is_ascii_alphabetic() {
            while let Some(c) = self.peek_char() {
                if c.is_ascii_alphabetic() {
                    name.push(c);
                    self.cursor += 1;
                } else {
                    break;
                }
            }
        }

        Ok(name)
    }

    fn validate_spec(&self, spec: ArgSpec) -> Result<ArgSpec, ArgSpecParseError> {
        if spec.no_leading_space && spec.required {
            return Err(self.err("`!` prefix is only valid for optional argument forms"));
        }

        match &spec.form {
            ArgForm::Standard => {
                if spec.kind.is_star() {
                    return Err(self.err("star value kind requires `s` form"));
                }
            }
            ArgForm::Star => {
                if spec.required {
                    return Err(self.err("star form must be optional"));
                }
                if !spec.kind.is_star() {
                    return Err(self.err("star form must use star value kind"));
                }
            }
            ArgForm::Group => {
                if spec.kind.is_star() {
                    return Err(self.err("group form cannot use star value kind"));
                }
            }
            ArgForm::Delimited { .. } | ArgForm::Paired { .. } => {
                if spec.kind.is_star() {
                    return Err(self.err("delimited/paired form cannot use star value kind"));
                }
                if spec.kind.is_delimiter() {
                    return Err(self.err("delimiter kind cannot use delimited/paired form"));
                }
            }
        }

        Ok(spec)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_char(), Some(c) if c.is_whitespace()) {
            self.cursor += 1;
        }
    }

    fn expect_char(&mut self, target: char) -> Result<(), ArgSpecParseError> {
        let got = self
            .next_char()
            .ok_or_else(|| self.err(format!("expected `{target}`")))?;
        if got != target {
            return Err(self.err(format!("expected `{target}`, found `{got}`")));
        }
        Ok(())
    }

    fn consume_if(&mut self, target: char) -> bool {
        if self.peek_char() == Some(target) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.cursor += 1;
        Some(ch)
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn eof(&self) -> bool {
        self.cursor >= self.chars.len()
    }

    fn err(&self, msg: impl Into<String>) -> ArgSpecParseError {
        ArgSpecParseError::new(self.context, self.cursor, msg)
    }
}
