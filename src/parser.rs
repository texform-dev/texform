//! Refactored Parser module - Layered architecture
//!
//! ## Architecture Design
//!
//! **Layer 1: Base Parsers** - No recursive dependencies, pure functional
//! - delimiter() - Delimiters
//! - escaped_symbol() - Escaped symbols
//! - active_char() - Active characters
//! - math_char() - Math characters
//! - text_chunk() - Text chunks
//!
//! **Layer 2: Parameterized Group Parsers** - Accept content_parser as parameter
//! - explicit_group_parser(mode, content_parser) - {...}
//! - delimited_group_parser(content_parser) - \left...\right
//!
//! **Layer 3: Parameterized Command Parsers** - Accept mode and strict
//! - prefix_command_parser(mode, strict) - \frac{}{} etc
//! - unknown_command_parser(mode, strict) - Unknown commands
//!
//! **Layer 4: Assembly Layer** - Combine within recursive closures
//! - parse_math_block(strict) - Math mode entry point
//! - parse_text_block(strict) - Text mode entry point

use chumsky::{input::InputRef, prelude::*};

use crate::knowledge::{self, ArgSpec, CommandKind};
use crate::lexer::Token;
use crate::syntax_node::{Argument, ArgumentKind, ContentMode, Delimiter, GroupKind, SyntaxNode};

// ============================================================================
// Public Interface
// ============================================================================

/// Filter tokens, removing whitespace and comments
pub fn filter_tokens(tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .cloned()
        .collect()
}

/// Parse entry point - Math mode
pub fn parse(tokens: &[Token], strict: bool) -> Result<SyntaxNode, Vec<Rich<'_, Token>>> {
    parse_math_block(strict)
        .then_ignore(end())
        .parse(tokens)
        .into_result()
}

// ============================================================================
// Layer 1: Base Parsers (no recursive dependencies)
// ============================================================================

/// Delimiter parser
///
/// Supports:
/// - '.' => Delimiter::None
/// - '(', ')', '[', ']', '|' etc => Delimiter::Char
/// - \langle, \rangle etc => Delimiter::Control
fn delimiter<'a>() -> impl Parser<'a, &'a [Token], Delimiter, extra::Err<Rich<'a, Token>>> + Clone {
    choice((
        select! { Token::Char('.') => Delimiter::None },
        select! {
            Token::Char(c) if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\')
                => Delimiter::Char(c)
        },
        select! {
            Token::ControlSeq(name) if matches!(
                name.as_str(),
                "langle" | "rangle" | "{" | "}" | "lfloor" | "rfloor" | "lceil" | "rceil" |
                "lvert" | "rvert" | "lVert" | "rVert" | "lgroup" | "rgroup" | "lmoustache" | "rmoustache"
            ) => Delimiter::Control(Box::leak(name.into_boxed_str()))
        },
    ))
}

/// Escaped symbol parser - Math mode
fn escaped_symbol<'a>()
-> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            let c = match name.as_str() {
                "%" => '%', "$" => '$', "&" => '&', "#" => '#',
                "_" => '_', "{" => '{', "}" => '}',
                _ => unreachable!(),
            };
            SyntaxNode::Char(c)
        }
    }
}

/// Active character parser - ~
fn active_char<'a>() -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone
{
    just(&Token::ActiveChar).to(SyntaxNode::ActiveSpace)
}

/// Math character parser
fn math_char<'a>() -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone
{
    select! {
        Token::Char(c) => SyntaxNode::Char(c),
        Token::Star => SyntaxNode::Char('*'),
        Token::Alignment => SyntaxNode::Char('&'),
    }
}

/// Text chunk parser - merge consecutive characters
fn text_chunk<'a>() -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone
{
    select! { Token::Char(c) => c }
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|chars| SyntaxNode::Text(chars.into_iter().collect()))
}

// ============================================================================
// Layer 2: Parameterized Group Parsers (accept content_parser)
// ============================================================================

/// Explicit group parser - {...}
///
/// Parameters:
/// - mode: Content mode of the group
/// - content_parser: Parser for group content
fn explicit_group_parser<'a, P>(
    mode: ContentMode,
    content_parser: P,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone
where
    P: Parser<'a, &'a [Token], Vec<SyntaxNode>, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    just(&Token::LBrace)
        .ignore_then(content_parser)
        .then_ignore(just(&Token::RBrace))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Delimited group parser - \left...\right
///
/// Parameters:
/// - content_parser: Parser for group content (Math mode)
fn delimited_group_parser<'a, P>(
    content_parser: P,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone
where
    P: Parser<'a, &'a [Token], Vec<SyntaxNode>, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    select! { Token::ControlSeq(name) if name == "left" => () }
        .ignore_then(delimiter())
        .then(content_parser)
        .then_ignore(select! { Token::ControlSeq(name) if name == "right" => () })
        .then(delimiter())
        .map(|((left, children), right)| SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Delimited { left, right },
            children,
        })
}

type ParserInput<'src, 'parse> =
    InputRef<'src, 'parse, &'src [Token], extra::Err<Rich<'src, Token>>>;

fn read_env_name<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    context: &str,
) -> Result<String, Rich<'src, Token>> {
    match input.next() {
        Some(Token::LBrace) => {}
        Some(_) => {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Expected '{{' after {}", context),
            ));
        }
        None => {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Unexpected end of input after {}", context),
            ));
        }
    }

    let mut name = String::new();
    loop {
        let token = match input.next() {
            Some(tok) => tok.clone(),
            None => {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unclosed {} name", context),
                ));
            }
        };

        match token {
            Token::RBrace => break,
            Token::Char(c) => name.push(c),
            Token::Star => name.push('*'),
            _ => {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Invalid token inside {}", context),
                ));
            }
        }
    }

    if name.is_empty() {
        return Err(Rich::custom(
            SimpleSpan::new((), 0..0),
            format!("Empty {} name", context),
        ));
    }

    Ok(name)
}

fn parse_argument_value_from_tokens<'src>(
    tokens: Vec<Token>,
    mode: ContentMode,
    strict: bool,
    context: &str,
) -> Result<SyntaxNode, Rich<'src, Token>> {
    match mode {
        ContentMode::Text => match parse_text_block(strict).parse(&tokens).into_result() {
            Ok(SyntaxNode::Group { children, .. }) => Ok(fold_items(ContentMode::Text, children)),
            Ok(other) => Ok(other),
            Err(_) => Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Failed to parse {}", context),
            )),
        },
        ContentMode::Math => match parse_math_block(strict).parse(&tokens).into_result() {
            Ok(SyntaxNode::Group { children, .. }) => Ok(SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Implicit,
                children,
            }),
            Ok(other) => Ok(other),
            Err(_) => Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Failed to parse {}", context),
            )),
        },
    }
}

fn empty_group(mode: ContentMode) -> SyntaxNode {
    SyntaxNode::Group {
        mode,
        kind: GroupKind::Implicit,
        children: vec![],
    }
}

fn collect_braced_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    context: &str,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    match input.next() {
        Some(Token::LBrace) => {}
        Some(tok) => {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Expected '{{' for {}, got {:?}", context, tok),
            ));
        }
        None => {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Expected '{{' for {}", context),
            ));
        }
    }

    let mut content_tokens = Vec::new();
    let mut depth = 1;
    while depth > 0 {
        match input.next() {
            Some(Token::LBrace) => {
                content_tokens.push(Token::LBrace);
                depth += 1;
            }
            Some(Token::RBrace) => {
                depth -= 1;
                if depth > 0 {
                    content_tokens.push(Token::RBrace);
                }
            }
            Some(tok) => content_tokens.push(tok.clone()),
            None => {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unclosed brace in {}", context),
                ));
            }
        }
    }

    Ok(content_tokens)
}

fn collect_bracket_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    context: &str,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let mut tokens = Vec::new();
    loop {
        match input.next() {
            Some(Token::RBracket) => break,
            Some(tok) => tokens.push(tok.clone()),
            None => {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unclosed bracket in {}", context),
                ));
            }
        }
    }
    Ok(tokens)
}

fn parse_argument_by_spec<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    spec: ArgSpec,
    strict: bool,
    context: &str,
) -> Result<Argument, Rich<'src, Token>> {
    match spec.kind {
        ArgumentKind::Mandatory => {
            let tokens = collect_braced_tokens(input, context)?;
            let value = parse_argument_value_from_tokens(tokens, spec.mode, strict, context)?;
            Ok(Argument::mandatory(value))
        }
        ArgumentKind::Optional => {
            if matches!(input.peek(), Some(Token::LBracket)) {
                input.next();
                let tokens = collect_bracket_tokens(input, context)?;
                let value = parse_argument_value_from_tokens(tokens, spec.mode, strict, context)?;
                Ok(Argument::optional(value))
            } else {
                Ok(Argument::optional(empty_group(spec.mode)))
            }
        }
    }
}

// ============================================================================
// Layer 3: Parameterized Command Parsers
// ============================================================================

/// Prefix command parser
///
/// Implements full command argument parsing logic using custom parser
/// All logic is inlined to avoid complex type signatures
fn prefix_command_parser<'a>(
    _mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    custom(move |input| {
        // 1. Peek command name
        let name = match input.peek() {
            Some(Token::ControlSeq(name)) => name.clone(),
            _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
        };

        // 2. Check blacklist
        if let Some(reason) = knowledge::is_blacklisted(&name) {
            input.next();
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Banned command: \\{} ({})", name, reason),
            ));
        }

        // 3. Lookup metadata - must be Prefix type
        let meta = match knowledge::lookup_command(&name) {
            Some(m) if m.kind == CommandKind::Prefix => m,
            Some(_) => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not prefix")),
            None if strict => {
                input.next();
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unknown command: \\{}", name),
                ));
            }
            None => return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown")),
        };

        // 4. Consume command token
        input.next();

        // 5. Parse star variant
        let starred = if matches!(input.peek(), Some(Token::Star)) {
            input.next();
            true
        } else {
            false
        };

        // 6. Parse arguments
        let mut args = Vec::new();
        for &arg_spec in meta.args {
            let arg = parse_argument_by_spec(input, arg_spec, strict, "command argument")?;
            args.push(arg);
        }

        Ok(SyntaxNode::Command {
            name,
            starred,
            args,
        })
    })
}

/// Unknown command parser
fn unknown_command_parser<'a>(
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    select! { Token::ControlSeq(name) => name }.try_map(move |name, span| {
        if knowledge::is_blacklisted(&name).is_some() {
            return Err(Rich::custom(span, "blacklisted"));
        }
        if knowledge::lookup_command(&name).is_some() {
            return Err(Rich::custom(span, "known command"));
        }
        if !strict {
            Ok(SyntaxNode::UnknownCommand {
                name,
                starred: false,
            })
        } else {
            Err(Rich::custom(span, "strict mode"))
        }
    })
}

fn parse_env_body<'src>(
    tokens: &[Token],
    mode: ContentMode,
    strict: bool,
) -> Result<SyntaxNode, Rich<'src, Token>> {
    let parsed = match mode {
        ContentMode::Math => parse_math_block(strict).parse(tokens).into_result(),
        ContentMode::Text => parse_text_block(strict).parse(tokens).into_result(),
    };

    match parsed {
        Ok(group @ SyntaxNode::Group { .. }) => Ok(group),
        Ok(other) => Ok(SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: vec![other],
        }),
        Err(_) => Err(Rich::custom(
            SimpleSpan::new((), 0..0),
            "Failed to parse environment body",
        )),
    }
}

fn environment_parser<'a>(
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    custom(move |input| {
        match input.peek() {
            Some(Token::ControlSeq(name)) if name == "begin" => {
                input.next();
            }
            _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not environment")),
        }

        let raw_name = read_env_name(input, "\\begin")?;
        let raw_name_full = raw_name.clone();

        let (base_name, mut starred) = if raw_name.ends_with('*') {
            let stripped = &raw_name[..raw_name.len() - 1];
            if stripped.is_empty() || stripped.contains('*') {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    "Invalid '*' placement in environment name",
                ));
            }
            (stripped.to_string(), true)
        } else {
            if raw_name.contains('*') {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    "Invalid '*' placement in environment name",
                ));
            }
            (raw_name.clone(), false)
        };

        let meta = match knowledge::lookup_env(&base_name) {
            Some(m) => m,
            None => {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unknown environment: {}", base_name),
                ));
            }
        };

        if starred && !meta.has_star_variant {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Environment {} has no starred variant", base_name),
            ));
        }

        if !meta.has_star_variant {
            starred = false;
        }

        let mut args = Vec::new();
        for &arg_spec in meta.args {
            let arg = parse_argument_by_spec(input, arg_spec, strict, "environment argument")?;
            args.push(arg);
        }

        let mut body_tokens = Vec::new();
        let mut depth = 0;
        loop {
            let token = match input.next() {
                Some(tok) => tok.clone(),
                None => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        "Unclosed environment body",
                    ));
                }
            };

            if let Token::ControlSeq(name) = &token {
                if name == "begin" {
                    depth += 1;
                } else if name == "end" {
                    let close_name = read_env_name(input, "\\end")?;
                    if depth == 0 {
                        if close_name != raw_name_full {
                            return Err(Rich::custom(
                                SimpleSpan::new((), 0..0),
                                "Environment name mismatch",
                            ));
                        }
                        break;
                    } else {
                        depth -= 1;
                        body_tokens.push(Token::ControlSeq("end".to_string()));
                        body_tokens.push(Token::LBrace);
                        for ch in close_name.chars() {
                            if ch == '*' {
                                body_tokens.push(Token::Star);
                            } else {
                                body_tokens.push(Token::Char(ch));
                            }
                        }
                        body_tokens.push(Token::RBrace);
                        continue;
                    }
                }
            }

            body_tokens.push(token);
        }

        let body = parse_env_body(&body_tokens, meta.body_mode, strict)?;

        Ok(SyntaxNode::Environment {
            name: base_name,
            starred,
            args,
            body: Box::new(body),
        })
    })
}

// ============================================================================
// Layer 4: Mode Parsers (Math and Text)
// ============================================================================

/// Math mode parser
fn parse_math_block<'a>(
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|group_content| {
        // === Build Math mode parsers ===

        // Atom layer: base elements
        let explicit_group = explicit_group_parser(ContentMode::Math, group_content.clone());

        // Delimited group: \left delim ... \right delim (inlined for better type inference)
        let delimited_group =
            select! { Token::ControlSeq(name) if matches!(name.as_str(), "left") => () }
                .ignore_then(delimiter())
                .then(group_content.clone())
                .then_ignore(
                    select! { Token::ControlSeq(name) if matches!(name.as_str(), "right") => () },
                )
                .then(delimiter())
                .map(|((left, children), right)| SyntaxNode::Group {
                    mode: ContentMode::Math,
                    kind: GroupKind::Delimited { left, right },
                    children,
                });

        let environment = environment_parser(strict);
        let prefix_command = prefix_command_parser(ContentMode::Math, strict);
        let unknown_command = unknown_command_parser(strict);

        let atom = choice((
            delimited_group,
            explicit_group,
            environment,
            escaped_symbol(),
            prefix_command,
            unknown_command,
            active_char(),
            math_char(),
        ));

        // Scripted layer: superscripts and subscripts
        let scripted = atom
            .clone()
            .then(
                choice((
                    just(&Token::Superscript)
                        .ignore_then(atom.clone())
                        .map(|n| (true, n)),
                    just(&Token::Subscript)
                        .ignore_then(atom.clone())
                        .map(|n| (false, n)),
                ))
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
            )
            .map(|(base, scripts)| {
                let mut subscript = None;
                let mut superscript = None;
                for (is_sup, node) in scripts {
                    if is_sup {
                        superscript = Some(Box::new(node));
                    } else {
                        subscript = Some(Box::new(node));
                    }
                }
                SyntaxNode::Scripted {
                    base: Box::new(base),
                    subscript,
                    superscript,
                }
            });

        let normal_item = choice((scripted, atom));

        // Group content: Leading + InfixTail + DeclarativeTail
        build_math_group_content(normal_item)
    })
    .map(|children| SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Implicit,
        children,
    })
}

/// Build Math mode group content parser
fn build_math_group_content<'a, P>(
    normal_item: P,
) -> impl Parser<'a, &'a [Token], Vec<SyntaxNode>, extra::Err<Rich<'a, Token>>> + Clone
where
    P: Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    // Define stop patterns
    let stop_infix_or_decl = select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                .unwrap_or(false) => ()
    };
    let stop_delimited = select! {
        Token::ControlSeq(name) if name.as_str() == "right" => ()
    };
    let stop_environment = select! {
        Token::ControlSeq(name) if name.as_str() == "end" => ()
    };

    // Leading items - stop before infix/declarative tokens and before \right
    let guarded_item = stop_infix_or_decl
        .or(stop_delimited)
        .or(stop_environment)
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    // Infix tail
    let infix_tail = build_infix_tail(normal_item.clone());

    // Declarative tail
    let declarative_tail = build_declarative_tail(normal_item);

    // Combine
    leading
        .then(infix_tail.or_not())
        .then(declarative_tail.or_not())
        .map(|((leading, infix_tail), declarative_tail)| {
            if let Some((infix_info, right_items)) = infix_tail {
                // Has Infix command
                let (name, starred, args) = infix_info;
                let left = fold_items(ContentMode::Math, leading);

                let mut right_content = right_items;
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_starred, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    right_content.push(SyntaxNode::Declarative {
                        name: decl_name,
                        starred: decl_starred,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }

                let right = fold_items(ContentMode::Math, right_content);

                vec![SyntaxNode::Infix {
                    name,
                    starred,
                    args,
                    left: Box::new(left),
                    right: Box::new(right),
                }]
            } else {
                // No Infix command
                let mut items = leading;
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_starred, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    items.push(SyntaxNode::Declarative {
                        name: decl_name,
                        starred: decl_starred,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }
                items
            }
        })
}

/// Build Infix tail parser
fn build_infix_tail<'a, P>(
    normal_item: P,
) -> impl Parser<
    'a,
    &'a [Token],
    ((String, bool, Vec<Argument>), Vec<SyntaxNode>),
    extra::Err<Rich<'a, Token>>,
> + Clone
where
    P: Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    let infix_cmd = custom(move |input| {
        let name = match input.peek() {
            Some(Token::ControlSeq(n)) => n.clone(),
            _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
        };
        input.next();

        if let Some(reason) = knowledge::is_blacklisted(&name) {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Banned: \\{} ({})", name, reason),
            ));
        }

        let meta = match knowledge::lookup_command(&name) {
            Some(m) if m.kind == CommandKind::Infix => m,
            Some(_) => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not infix")),
            None => return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown")),
        };

        let starred = if meta.has_star_variant && matches!(input.peek(), Some(Token::Star)) {
            input.next();
            true
        } else {
            false
        };

        Ok((name, starred, Vec::new()))
    });

    let stop_declarative = select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Declarative)
                .unwrap_or(false) => ()
    };

    let guarded_item = stop_declarative
        .not()
        .then(normal_item)
        .map(|(_, item)| item);
    let right_items = guarded_item.repeated().at_least(1).collect::<Vec<_>>();

    infix_cmd.then(right_items)
}

/// Build Declarative tail parser
fn build_declarative_tail<'a, P>(
    normal_item: P,
) -> impl Parser<
    'a,
    &'a [Token],
    ((String, bool, Vec<Argument>), Vec<SyntaxNode>),
    extra::Err<Rich<'a, Token>>,
> + Clone
where
    P: Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    let decl_cmd = custom(move |input| {
        let name = match input.peek() {
            Some(Token::ControlSeq(n)) => n.clone(),
            _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
        };
        input.next();

        if let Some(reason) = knowledge::is_blacklisted(&name) {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Banned: \\{} ({})", name, reason),
            ));
        }

        let meta = match knowledge::lookup_command(&name) {
            Some(m) if m.kind == CommandKind::Declarative => m,
            Some(_) => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not declarative")),
            None => return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown")),
        };

        let starred = if meta.has_star_variant && matches!(input.peek(), Some(Token::Star)) {
            input.next();
            true
        } else {
            false
        };

        Ok((name, starred, Vec::new()))
    });

    let scope_items = normal_item.repeated().collect::<Vec<_>>();
    decl_cmd.then(scope_items)
}

/// Text mode parser
fn parse_text_block<'a>(
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|group_content| {
        // === Build Text mode parsers ===

        // Inline math: $...$
        let inline_math = just(&Token::MathShift)
            .ignore_then(parse_math_block(strict).map(|node| match node {
                SyntaxNode::Group { mode, children, .. } => SyntaxNode::Group {
                    mode,
                    kind: GroupKind::InlineMath,
                    children,
                },
                other => other,
            }))
            .then_ignore(just(&Token::MathShift));

        let explicit_group = explicit_group_parser(ContentMode::Text, group_content.clone());
        let prefix_command = prefix_command_parser(ContentMode::Text, strict);
        let environment = environment_parser(strict);
        let unknown_command = unknown_command_parser(strict);

        let normal_item = choice((
            text_chunk(),
            inline_math,
            explicit_group,
            environment,
            escaped_symbol(),
            prefix_command,
            unknown_command,
            active_char(),
        ));

        // Text mode only has Declarative tail, no Infix
        build_text_group_content(normal_item)
    })
    .map(|children| SyntaxNode::Group {
        mode: ContentMode::Text,
        kind: GroupKind::Implicit,
        children,
    })
}

/// Build Text mode group content parser
fn build_text_group_content<'a, P>(
    normal_item: P,
) -> impl Parser<'a, &'a [Token], Vec<SyntaxNode>, extra::Err<Rich<'a, Token>>> + Clone
where
    P: Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone + 'a,
{
    let stop_declarative = select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Declarative)
                .unwrap_or(false) => ()
    };
    let stop_environment = select! {
        Token::ControlSeq(name) if name.as_str() == "end" => ()
    };

    let guarded_item = stop_declarative
        .or(stop_environment)
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let declarative_tail = build_declarative_tail(normal_item);

    leading
        .then(declarative_tail.or_not())
        .map(|(mut leading, declarative_tail)| {
            if let Some((decl_info, scope_items)) = declarative_tail {
                let (decl_name, decl_starred, decl_args) = decl_info;
                let scope = fold_items(ContentMode::Text, scope_items);
                leading.push(SyntaxNode::Declarative {
                    name: decl_name,
                    starred: decl_starred,
                    args: decl_args,
                    scope: Box::new(scope),
                });
            }
            leading
        })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Fold node list into single node
fn fold_items(mode: ContentMode, items: Vec<SyntaxNode>) -> SyntaxNode {
    match items.len() {
        0 => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: vec![],
        },
        1 => items.into_iter().next().unwrap(),
        _ => SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: items,
        },
    }
}

// Tests in tests/parser.rs
