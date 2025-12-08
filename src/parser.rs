//! Parser module - content-first combinator architecture
//!
//! - Base parsers: delimiters, escapes, characters, whitespace
//! - Group builders: implicit/braced/bracket/delimited/env_body
//! - Content & arguments: mode content factories + argument_parser/arguments_parser
//! - Commands & environments: custom heads with combinator arguments/body
//! - Mode entry: mode_group_parsers + math_block_parser/text_block_parser

use chumsky::{input::InputRef, prelude::*};

use crate::knowledge::{self, ArgSpec, CommandKind, CommandMeta, EnvMeta};
use crate::lexer::Token;
use crate::syntax_node::{Argument, ArgumentKind, ContentMode, Delimiter, GroupKind, SyntaxNode};

type ParserError<'a> = extra::Err<Rich<'a, Token>>;
type TokenInput<'a> = &'a [Token];
type ContentParser<'a> = Boxed<'a, 'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>>;
type NodeParser<'a> = Boxed<'a, 'a, TokenInput<'a>, SyntaxNode, ParserError<'a>>;
type ArgumentParser<'a> = Boxed<'a, 'a, TokenInput<'a>, Argument, ParserError<'a>>;
type ParserInput<'src, 'parse> = InputRef<'src, 'parse, TokenInput<'src>, ParserError<'src>>;
type TailParseOutput = ((String, bool, Vec<Argument>), Vec<SyntaxNode>);

// ============================================================================
// Public Interface
// ============================================================================

/// Filter tokens, removing only comments (whitespace must be preserved for text mode) TODO: Move comment filtering into lexer and drop this helper
pub fn filter_tokens(tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .filter(|t| !matches!(t, Token::Comment(_)))
        .cloned()
        .collect()
}

/// Parse entry point - Math mode.
pub fn parse(tokens: &[Token], strict: bool) -> Result<SyntaxNode, Vec<Rich<'_, Token>>> {
    math_block_parser(strict)
        .then_ignore(end())
        .parse(tokens)
        .into_result()
}

// ============================================================================
// Layer 1: Base Parsers (no recursive dependencies)
// ============================================================================

/// Parse a math delimiter token into a typed `Delimiter`.
///
/// Supports:
/// - '.' => Delimiter::None
/// - '(', ')', '[', ']', '|' etc => Delimiter::Char
/// - \langle, \rangle etc => Delimiter::Control
/// TODO: Integrate MathJax delimiter map and distinguish left/right.
fn delimiter<'a>() -> impl Parser<'a, TokenInput<'a>, Delimiter, ParserError<'a>> + Clone {
    select! {
        Token::Char('.') => Delimiter::None, // LaTeX use \left. to represent no delimiter
        Token::Char(c) if matches!(c, '(' | ')' | '[' | ']' | '|' | '<' | '>' | '/' | '\\')
            => Delimiter::Char(c),
        Token::ControlSeq(name) if knowledge::is_delimiter_control(name.as_str()) => {
            Delimiter::Control(Box::leak(name.into_boxed_str()))
        }
    }
    .labelled("delimiter")
}

/// Parse escaped symbol control sequences into raw `Char` nodes.
/// TODO: Confirm the full list of escapable symbols.
fn escaped_symbol<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
            SyntaxNode::Char(name.chars().next().unwrap())
        }
    }
    .labelled("escaped symbol")
}

/// Parse the active character `~` into `ActiveSpace`.
fn active_char<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    just(&Token::ActiveChar).to(SyntaxNode::ActiveSpace)
}

/// Parse plain math characters (including `*` and `&` tokens).
fn math_char<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => SyntaxNode::Char(c),
        Token::Star => SyntaxNode::Char('*'),
        Token::Alignment => SyntaxNode::Char('&'),
    }
    .labelled("math character")
}

/// Parse and coalesce consecutive text characters/whitespace into a single `Text` node.
fn text_chunk<'a>() -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::Char(c) => c,
        Token::Whitespaces => ' ',
    }
    .repeated()
    .at_least(1)
    .collect::<Vec<_>>()
    .labelled("text")
    .map(|chars| {
        let mut buf = String::new();
        let mut last_was_space = false;
        for ch in chars {
            if ch == ' ' {
                if !last_was_space {
                    buf.push(' ');
                    last_was_space = true;
                }
            } else {
                buf.push(ch);
                last_was_space = false;
            }
        }
        SyntaxNode::Text(buf)
    })
}

/// Consume insignificant whitespace tokens and produce no output.
fn insignificant_whitespace<'a>() -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone {
    select! { Token::Whitespaces => () }.repeated().ignored()
}

/// Match an exact control sequence.
fn control_seq<'a>(
    target: &'static str,
) -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name) if name == target => (),
    }
    .labelled(target)
}

/// Build an implicit group from a content parser.
fn implicit_group_parser<'a>(
    mode: ContentMode,
    content: ContentParser<'a>,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    content.map(move |children| SyntaxNode::Group {
        mode,
        kind: GroupKind::Implicit,
        children,
    })
}

/// Parse an explicit `{...}` group with the given content parser.
fn braced_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    just(&Token::LBrace)
        .ignore_then(content)
        .then_ignore(just(&Token::RBrace))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Parse a bracketed `[...]` group with the given content parser.
fn bracket_group_parser<'a, P>(
    mode: ContentMode,
    content: P,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    just(&Token::LBracket)
        .ignore_then(content)
        .then_ignore(just(&Token::RBracket))
        .map(move |children| SyntaxNode::Group {
            mode,
            kind: GroupKind::Explicit,
            children,
        })
}

/// Parse `\\left ... \\right` delimited math group.
fn delimited_group_parser<'a>(
    math_content: ContentParser<'a>,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    control_seq("left")
        .ignore_then(delimiter())
        .then(math_content)
        .then_ignore(control_seq("right"))
        .then(delimiter())
        .map(|((left, children), right)| SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Delimited { left, right },
            children,
        })
}

/// Pick the correct implicit group parser for an environment body.
fn env_body_parser<'a>(
    mode: ContentMode,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    match mode {
        ContentMode::Math => implicit_group_parser(ContentMode::Math, math_content),
        ContentMode::Text => implicit_group_parser(ContentMode::Text, text_content),
    }
}

/// Parse a control sequence and ensure it matches the expected command kind.
fn command_head_parser<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    expected_kind: CommandKind,
    strict: bool,
) -> Result<(String, &'static CommandMeta, bool), Rich<'src, Token>> {
    let name = match input.peek() {
        Some(Token::ControlSeq(name)) => name.clone(),
        _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
    };

    if let Some(reason) = knowledge::is_blacklisted(&name) {
        input.next();
        return Err(Rich::custom(
            SimpleSpan::new((), 0..0),
            format!("Banned command: \\{} ({})", name, reason),
        ));
    }

    let kind_label = |kind: CommandKind| match kind {
        CommandKind::Prefix => "prefix",
        CommandKind::Infix => "infix",
        CommandKind::Declarative => "declarative",
    };

    let meta = match knowledge::lookup_command(&name) {
        Some(meta) if meta.kind == expected_kind => meta,
        Some(_) => {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("not {}", kind_label(expected_kind)),
            ));
        }
        None => {
            if strict {
                input.next();
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Unknown command: \\{}", name),
                ));
            } else {
                return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown"));
            }
        }
    };

    input.next();

    let starred = if matches!(input.peek(), Some(Token::Star)) {
        if meta.has_star_variant {
            input.next();
            true
        } else {
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!("Command \\{} has no starred variant", name),
            ));
        }
    } else {
        false
    };

    Ok((name, meta, starred))
}

// ============================================================================
// Content and Argument Parsers
// ============================================================================

/// Guard used to stop math content before infix/declarative commands.
fn math_infix_or_decl_guard<'a>() -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                .unwrap_or(false) => ()
    }
}

/// Guard used to stop text content before declarative commands.
fn text_declarative_guard<'a>() -> impl Parser<'a, TokenInput<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Declarative)
                .unwrap_or(false) => ()
    }
}

/// Parse a single math item (respecting script rules and stop guards).
fn math_item_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let ws = insignificant_whitespace();
    let atom = math_atom_parser(math_content.clone(), math_content, text_content, strict);
    let scripted = scripted_atom_parser(atom);
    let normal_item = scripted.padded_by(ws.clone());

    math_infix_or_decl_guard()
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(normal_item)
}

/// Parse a single text item (respecting stop guards).
fn text_item_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let normal_item = text_atom_parser(text_content.clone(), math_content, text_content, strict);

    text_declarative_guard()
        .or(control_seq("end"))
        .not()
        .ignore_then(normal_item)
}

/// Parse one argument according to `ArgSpec`, producing an `Argument`.
fn argument_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    spec: ArgSpec,
    strict: bool,
) -> ArgumentParser<'a> {
    let content = match spec.mode {
        ContentMode::Math => math_content.clone(),
        ContentMode::Text => text_content.clone(),
    };

    match spec.kind {
        ArgumentKind::Mandatory => {
            let braced = braced_group_parser(spec.mode, content.clone());
            let single_item: NodeParser<'a> = match spec.mode {
                ContentMode::Math => math_item_parser(math_content, text_content, strict).boxed(),
                ContentMode::Text => text_item_parser(math_content, text_content, strict).boxed(),
            };
            choice((braced, single_item))
                .labelled("mandatory argument")
                .map(move |node| Argument::mandatory(normalize_argument_value(spec.mode, node)))
                .boxed()
        }
        ArgumentKind::Optional => bracket_group_parser(spec.mode, content)
            .labelled("optional argument")
            .or_not()
            .map(move |opt| match opt {
                Some(node) => Argument::optional(normalize_argument_value(spec.mode, node)),
                None => Argument::optional(SyntaxNode::empty_group(spec.mode)),
            })
            .boxed(),
    }
}

/// Parse a full argument list driven by metadata specs. This is the only custom loop in the argument layer.
fn arguments_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    specs: &'static [ArgSpec],
    strict: bool,
    context: &'static str,
) -> impl Parser<'a, TokenInput<'a>, Vec<Argument>, ParserError<'a>> + Clone {
    custom(move |input| {
        let mut args = Vec::with_capacity(specs.len());

        for &spec in specs {
            let _ = input.parse(insignificant_whitespace());
            let parser = argument_parser(math_content.clone(), text_content.clone(), spec, strict)
                .labelled(context);
            let arg = input.parse(parser)?;
            args.push(arg);
        }

        Ok(args)
    })
}

// ============================================================================
// Command and Environment Parsers
// ============================================================================

fn prefix_command_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, meta, starred) = match command_head_parser(input, CommandKind::Prefix, strict) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        let args = input.parse(arguments_parser(
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "command argument",
        ))?;

        Ok(SyntaxNode::Command {
            name,
            starred,
            args,
        })
    })
}

fn unknown_command_parser<'a>(
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! { Token::ControlSeq(name) => name }
        .try_map(move |name, span| {
            if let Some(reason) = knowledge::is_blacklisted(&name) {
                return Err(Rich::custom(
                    span,
                    format!("Banned command: \\{} ({})", name, reason),
                ));
            }
            if knowledge::lookup_command(name.as_str()).is_some() {
                return Err(Rich::custom(span, "Unexpected known command"));
            }

            if strict {
                Err(Rich::custom(span, format!("Unknown command: \\{}", name)))
            } else {
                Ok(SyntaxNode::UnknownCommand {
                    name,
                    starred: false,
                })
            }
        })
        .labelled("unknown command")
}

/// Parse `{name}` or `{name*}` inside environment delimiters.
fn env_name_parser<'a>() -> impl Parser<'a, TokenInput<'a>, (String, bool), ParserError<'a>> + Clone
{
    let base_name = select! {
        Token::Char(c) => c,
    }
    .repeated()
    .at_least(1)
    .collect::<String>();

    let starred = just(Token::Star).or_not().map(|s| s.is_some());

    base_name
        .then(starred)
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .labelled("environment name")
}

/// Parse `\begin{name}` plus its arguments, returning metadata.
fn parse_env_header<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, (String, bool, Vec<Argument>, &'static EnvMeta), ParserError<'a>>
+ Clone {
    custom(move |input| {
        input.parse(control_seq("begin"))?;

        let (base_name, starred) = input.parse(env_name_parser())?;

        let meta = match knowledge::lookup_env(base_name.as_str()) {
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

        let args = input.parse(arguments_parser(
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "environment argument",
        ))?;

        Ok((base_name, starred, args, meta))
    })
}

/// Parse a full environment including body and closing tag.
fn environment_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, starred, args, meta) = input.parse(parse_env_header(
            math_content.clone(),
            text_content.clone(),
            strict,
        ))?;

        let body = input.parse(env_body_parser(
            meta.body_mode,
            math_content.clone(),
            text_content.clone(),
        ))?;

        let end_tag = just(Token::ControlSeq("end".into()))
            .ignore_then(env_name_parser())
            .labelled("environment end tag");

        let (end_name, end_starred) = input.parse(end_tag)?;

        if end_name != name || end_starred != starred {
            let expected = if starred {
                format!("{name}*")
            } else {
                name.clone()
            };
            let found = if end_starred {
                format!("{end_name}*")
            } else {
                end_name.clone()
            };
            return Err(Rich::custom(
                SimpleSpan::new((), 0..0),
                format!(
                    "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                    expected, found
                ),
            ));
        }

        Ok(SyntaxNode::Environment {
            name,
            starred,
            args,
            body: Box::new(body),
        })
    })
}

// ============================================================================
// Mode Parsers (Math and Text)
// ============================================================================

/// Parse a math atom (group/command/env/char) without scripts.
fn math_atom_parser<'a, P>(
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let explicit_group = braced_group_parser(ContentMode::Math, group_content.clone());
    let delimited_group = delimited_group_parser(math_content.clone());
    let environment = environment_parser(math_content.clone(), text_content.clone(), strict);
    let prefix_command = prefix_command_parser(math_content, text_content, strict);
    let unknown_command = unknown_command_parser(strict);

    choice((
        delimited_group,
        explicit_group,
        environment,
        escaped_symbol(),
        prefix_command,
        unknown_command,
        active_char(),
        math_char(),
    ))
}

/// Wrap a base atom with script parsing (`^`, `_`, primes).
fn scripted_atom_parser<'a, P>(
    atom: P,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let atom_for_scripts = atom.clone().padded_by(ws.clone());

    let script_marker = choice((
        just(&Token::Superscript)
            .padded_by(ws.clone())
            .ignore_then(atom_for_scripts.clone())
            .map(|n| (true, false, n)),
        just(&Token::Subscript)
            .padded_by(ws.clone())
            .ignore_then(atom_for_scripts.clone())
            .map(|n| (false, false, n)),
        just(&Token::Prime)
            .repeated()
            .at_least(1)
            .collect::<Vec<_>>()
            .map(|primes| {
                let prime_nodes: Vec<SyntaxNode> =
                    primes.iter().map(|_| SyntaxNode::Char('\'')).collect();
                let node = if prime_nodes.len() == 1 {
                    prime_nodes.into_iter().next().unwrap()
                } else {
                    SyntaxNode::Group {
                        mode: ContentMode::Math,
                        kind: GroupKind::Implicit,
                        children: prime_nodes,
                    }
                };
                (true, true, node)
            }),
    ));

    atom_for_scripts
        .then(script_marker.repeated().collect::<Vec<_>>())
        .map(|(base, scripts)| {
            if scripts.is_empty() {
                return base;
            }

            let mut subscript: Option<Box<SyntaxNode>> = None;
            let mut superscript: Option<Box<SyntaxNode>> = None;
            for (is_sup, is_prime, node) in scripts {
                if is_sup {
                    if is_prime {
                        superscript = Some(Box::new(match superscript {
                            None => node,
                            Some(existing) => SyntaxNode::Group {
                                mode: ContentMode::Math,
                                kind: GroupKind::Implicit,
                                children: vec![*existing, node],
                            },
                        }));
                    } else {
                        superscript = Some(Box::new(node));
                    }
                } else {
                    subscript = Some(Box::new(node));
                }
            }
            SyntaxNode::Scripted {
                base: Box::new(base),
                subscript,
                superscript,
            }
        })
}

/// Parse a text atom (text chunk, inline math, group, command, env).
fn text_atom_parser<'a, P>(
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let inline_math = just(&Token::MathShift)
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            math_content.clone(),
        ))
        .then_ignore(just(&Token::MathShift))
        .map(|node| match node {
            SyntaxNode::Group { mode, children, .. } => SyntaxNode::Group {
                mode,
                kind: GroupKind::InlineMath,
                children,
            },
            other => other,
        });

    let explicit_group = braced_group_parser(ContentMode::Text, group_content);
    let environment = environment_parser(math_content.clone(), text_content.clone(), strict);
    let prefix_command = prefix_command_parser(math_content, text_content, strict);
    let unknown_command = unknown_command_parser(strict);

    choice((
        text_chunk(),
        inline_math,
        explicit_group,
        environment,
        escaped_symbol(),
        prefix_command,
        unknown_command,
        active_char(),
    ))
}

/// Parse the tail after an infix command: the command head plus right operand items.
fn infix_tail_parser<'a, P>(
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let infix_cmd = custom(move |input| {
        let (name, meta, starred) = match command_head_parser(input, CommandKind::Infix, strict) {
            Ok(data) => data,
            Err(err) => return Err(err),
        };

        let args = input.parse(arguments_parser(
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "infix command argument",
        ))?;

        Ok((name, starred, args))
    });

    let stop_declarative = text_declarative_guard();

    let guarded_item = stop_declarative
        .not()
        .then(normal_item)
        .map(|(_, item)| item);
    let right_items = guarded_item.repeated().at_least(1).collect::<Vec<_>>();

    infix_cmd.then(right_items)
}

/// Parse the tail of a declarative command: command head plus scoped items.
fn declarative_tail_parser<'a, P>(
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let decl_cmd = custom(move |input| {
        let (name, meta, starred) =
            match command_head_parser(input, CommandKind::Declarative, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "declarative command argument",
        ))?;

        Ok((name, starred, args))
    });

    let scope_items = normal_item.repeated().collect::<Vec<_>>();
    decl_cmd.then(scope_items)
}

/// Build math-mode group content (leading items + optional infix/declarative tails).
fn math_group_content_parser<'a, P>(
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_infix_or_decl = math_infix_or_decl_guard();
    let guarded_item = stop_infix_or_decl
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let infix_tail = infix_tail_parser(
        normal_item.clone(),
        math_content.clone(),
        text_content.clone(),
        strict,
    );

    let declarative_tail = declarative_tail_parser(normal_item, math_content, text_content, strict);

    leading
        .then(infix_tail.or_not())
        .then(declarative_tail.or_not())
        .try_map(|((leading, infix_tail), declarative_tail), span| {
            if let Some((infix_info, right_items)) = infix_tail {
                if leading.is_empty() {
                    return Err(Rich::custom(
                        span,
                        "Infix command requires non-empty left operand",
                    ));
                }

                let (name, starred, args) = infix_info;
                let left = fold_items(ContentMode::Math, leading);
                let right = fold_items(ContentMode::Math, right_items);

                let infix_node = SyntaxNode::Infix {
                    name,
                    starred,
                    args,
                    left: Box::new(left),
                    right: Box::new(right),
                };

                let mut nodes = vec![infix_node];
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_starred, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    nodes.push(SyntaxNode::Declarative {
                        name: decl_name,
                        starred: decl_starred,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }
                Ok(nodes)
            } else {
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
                Ok(items)
            }
        })
}

/// Build text-mode group content (leading items + optional declarative tail).
fn text_group_content_parser<'a, P>(
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenInput<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenInput<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_declarative = text_declarative_guard();

    let guarded_item = stop_declarative
        .or(control_seq("end"))
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let declarative_tail = declarative_tail_parser(normal_item, math_content, text_content, strict);

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

/// Construct paired math/text content parsers using mutually recursive declarations.
fn mode_content_parsers<'a>(strict: bool) -> (ContentParser<'a>, ContentParser<'a>) {
    let mut math = Recursive::declare();
    let mut text = Recursive::declare();

    let math_for_math = math.clone();
    let text_for_math = text.clone();
    math.define(recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = math_for_math.clone().boxed();
        let text_content = text_for_math.clone().boxed();
        let atom = math_atom_parser(
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        let scripted = scripted_atom_parser(atom);
        let normal_item = scripted.padded_by(ws.clone());
        math_group_content_parser(normal_item, math_content, text_content, strict).padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let normal_item = text_atom_parser(
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        text_group_content_parser(normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

/// Construct top-level math/text group parsers from content parsers.
fn mode_group_parsers<'a>(strict: bool) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers(strict);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

/// Entry point parser for math mode.
fn math_block_parser<'a>(strict: bool) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(strict);
    math_parser
}

/// Entry point parser for text mode.
#[allow(dead_code)] // Text entry point is unused; expose when direct text parsing is needed
fn text_block_parser<'a>(strict: bool) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers(strict);
    text_parser
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Normalize argument value by collapsing text-mode groups to a single node.
fn normalize_argument_value(mode: ContentMode, node: SyntaxNode) -> SyntaxNode {
    match (mode, node) {
        (ContentMode::Text, SyntaxNode::Group { children, .. }) => {
            fold_items(ContentMode::Text, children)
        }
        (_, other) => other,
    }
}

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
