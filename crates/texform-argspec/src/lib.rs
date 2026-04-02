use std::borrow::Cow;
use std::ops::Deref;

pub use texform_interface::syntax_node::ContentMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelimiterToken {
    Char(char),
    ControlSeq(Cow<'static, str>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgForm {
    Standard,
    Star,
    Group,
    Delimited {
        open: DelimiterToken,
        close: DelimiterToken,
    },
    Paired {
        pairs: Cow<'static, [(DelimiterToken, DelimiterToken)]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Content { mode: ContentMode },
    Delimiter,
    CSName,
    Dimension,
    Integer,
    KeyVal,
    Column,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgSpec {
    pub required: bool,
    pub no_leading_space: bool,
    pub nullable: bool,
    pub kind: ValueKind,
    pub form: ArgForm,
}

impl ArgSpec {
    pub const fn new(required: bool, kind: ValueKind) -> Self {
        ArgSpec {
            required,
            no_leading_space: false,
            nullable: false,
            kind,
            form: ArgForm::Standard,
        }
    }

    pub const fn with_form(
        required: bool,
        no_leading_space: bool,
        kind: ValueKind,
        form: ArgForm,
    ) -> Self {
        ArgSpec {
            required,
            no_leading_space,
            nullable: false,
            kind,
            form,
        }
    }

    pub const fn mandatory(mode: ContentMode) -> Self {
        ArgSpec {
            required: true,
            no_leading_space: false,
            nullable: false,
            kind: ValueKind::Content { mode },
            form: ArgForm::Standard,
        }
    }

    pub const fn optional(mode: ContentMode) -> Self {
        ArgSpec {
            required: false,
            no_leading_space: false,
            nullable: false,
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

/// A parsed argspec: the structured argument list together with the source
/// string it was parsed from. Produced by the `argspec!` compile-time macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedArgSpec {
    pub args: &'static [ArgSpec],
    pub source: &'static str,
}

impl Deref for ParsedArgSpec {
    type Target = [ArgSpec];
    fn deref(&self) -> &[ArgSpec] {
        self.args
    }
}

/// Owned counterpart of [`ParsedArgSpec`] for runtime-loaded specs (e.g. YAML).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedArgSpec {
    pub args: Vec<ArgSpec>,
    pub source: String,
}

impl From<ParsedArgSpec> for OwnedArgSpec {
    fn from(value: ParsedArgSpec) -> Self {
        Self {
            args: value.args.to_vec(),
            source: value.source.to_string(),
        }
    }
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

pub fn parse_arg_specs(spec: &str, context: &str) -> Result<Vec<ArgSpec>, ArgSpecParseError> {
    ArgSpecParser::new(spec, context).parse()
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

        let (kind, nullable) = if matches!(&form, ArgForm::Star) {
            if self.peek_char() == Some(':') {
                return Err(self.err("`s` does not accept value type annotation"));
            }
            (ValueKind::Star, false)
        } else {
            self.parse_value_kind_annotation()?
        };

        let spec = ArgSpec {
            required,
            no_leading_space,
            nullable,
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

    fn parse_value_kind_annotation(&mut self) -> Result<(ValueKind, bool), ArgSpecParseError> {
        if !self.consume_if(':') {
            return Ok((
                ValueKind::Content {
                    mode: ContentMode::Math,
                },
                false,
            ));
        }

        let annotation = self
            .next_char()
            .ok_or_else(|| self.err("missing value kind annotation after `:`"))?;
        let kind = match annotation {
            'T' => ValueKind::Content {
                mode: ContentMode::Text,
            },
            'D' => ValueKind::Delimiter,
            'N' => ValueKind::CSName,
            'L' => ValueKind::Dimension,
            'I' => ValueKind::Integer,
            'K' => ValueKind::KeyVal,
            'C' => ValueKind::Column,
            other => {
                return Err(self.err(format!("unsupported value kind annotation `:{other}`")));
            }
        };
        let nullable = self.consume_if('?');
        Ok((kind, nullable))
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
        if spec.nullable && !spec.kind.is_delimiter() {
            return Err(self.err("`?` is currently only supported for delimiter annotations"));
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
