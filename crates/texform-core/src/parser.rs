//! Parser module - core combinator architecture
//!
//! - Content & arguments: mode content factories + argument_parser/arguments_parser
//! - Commands & environments: custom heads with combinator arguments/body
//! - Mode entry: mode_group_parsers + math_block_parser/text_block_parser

use chumsky::prelude::*;

use crate::knowledge::{self, ArgSpec, CommandKind, CommandMeta, EnvMeta, ValueKind};
use crate::lexer::Token;
use crate::parser_utils::{
    // Base parsers
    active_char,
    braced_group_parser,
    bracket_group_parser,
    build_token_stream,
    control_seq,
    delimited_group_parser,
    delimiter,
    // Value combinators
    dimension,
    escaped_symbol,
    // Helpers
    fold_items,
    implicit_group_parser,
    // Token-level parsers
    insignificant_whitespace,
    integer,
    keyval_value,
    math_char,
    maybe_braced,
    normalize_argument_value,
    optional_bracketed,
    parse_scripted_components,
    text_chunk,
    ParserError,
    ParserInput,
    ParserInputExt,
    Spanned,
    TokenStream,
};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

type ContentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>>;
type NodeParser<'a> = Boxed<'a, 'a, TokenStream<'a>, SyntaxNode, ParserError<'a>>;
type ArgumentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, Argument, ParserError<'a>>;
type TailParseOutput = ((String, bool, Vec<Argument>), Vec<SyntaxNode>);

// ============================================================================
// Public Interface
// ============================================================================

/// Parse entry point - Math mode. Accepts source string directly.
/// Returns a `Spanned<SyntaxNode>` where the span covers the full input range.
pub fn parse(src: &str, strict: bool) -> Result<Spanned<SyntaxNode>, Vec<Rich<'_, Token>>> {
    let token_stream = build_token_stream(src);
    math_block_parser(strict)
        .map_with(|node, e| (node, e.span()))
        .then_ignore(end())
        .parse(token_stream)
        .into_result()
}

// ============================================================================
// Environment Body Parser
// ============================================================================

/// Pick the correct implicit group parser for an environment body.
fn env_body_parser<'a>(
    mode: ContentMode,
    content: ContentParser<'a>,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    implicit_group_parser(mode, content)
}

/// Parse a control sequence and ensure it matches the expected command kind.
fn command_head_parser<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    expected_kind: CommandKind,
    strict: bool,
) -> Result<(String, &'static CommandMeta, bool), Rich<'src, Token>> {
    let cmd_start = input.cursor();
    let token = input.next();
    let name = match token {
        Some(Token::ControlSeq(name)) => name,
        Some(_) => return Err(input.err_since(&cmd_start, "not a command")),
        None => return Err(input.err_since(&cmd_start, "not a command")),
    };

    let cmd_span = input.span_from_cursor(&cmd_start);

    if knowledge::is_blocklisted(&name) {
        return Err(Rich::custom(
            cmd_span,
            format!("Banned command: \\{}", name),
        ));
    }

    let meta = match knowledge::lookup_command(&name) {
        Some(meta) if meta.kind == expected_kind => meta,
        Some(_) => {
            return Err(Rich::custom(
                cmd_span,
                format!("not {}", expected_kind.label()),
            ));
        }
        None => {
            if strict {
                return Err(Rich::custom(
                    cmd_span,
                    format!("Unknown command: \\{}", name),
                ));
            } else {
                return Err(Rich::custom(cmd_span, "unknown"));
            }
        }
    };

    let starred = if matches!(input.peek(), Some(Token::Star)) {
        let star_cursor = input.cursor();
        if meta.has_star_variant {
            input.next();
            true
        } else {
            return Err(input.err_peek_or_point(
                &star_cursor,
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
fn math_infix_or_decl_guard<'a>() -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if knowledge::lookup_command(name.as_str())
                .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                .unwrap_or(false) => ()
    }
}

/// Guard used to stop content parsing before declarative commands.
fn declarative_guard<'a>() -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let normal_item = text_atom_parser(text_content.clone(), math_content, text_content, strict);

    declarative_guard()
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
    match spec.kind {
        ValueKind::Content { mode } => {
            let content = match mode {
                ContentMode::Math => math_content.clone(),
                ContentMode::Text => text_content.clone(),
            };

            if spec.required {
                let braced = braced_group_parser(mode, content.clone());
                let single_item: NodeParser<'a> = match mode {
                    ContentMode::Math => {
                        math_item_parser(math_content, text_content, strict).boxed()
                    }
                    ContentMode::Text => {
                        text_item_parser(math_content, text_content, strict).boxed()
                    }
                };
                choice((braced, single_item))
                    .labelled("mandatory argument")
                    .map(move |node| Argument::mandatory(normalize_argument_value(mode, node)))
                    .boxed()
            } else {
                bracket_group_parser(mode, content)
                    .labelled("optional argument")
                    .or_not()
                    .map(move |opt| match opt {
                        Some(node) => Argument::optional(normalize_argument_value(mode, node)),
                        None => Argument::optional(SyntaxNode::empty_group(mode)),
                    })
                    .boxed()
            }
        }
        ValueKind::Delimiter => {
            let kind = ArgumentKind::from_required(spec.required);

            if spec.required {
                maybe_braced(delimiter())
                    .map(move |value| Argument::from_value(kind, ArgumentValue::Delimiter(value)))
                    .boxed()
            } else {
                optional_bracketed(delimiter())
                    .map(move |opt| {
                        let value = opt.unwrap_or(Delimiter::None);
                        Argument::from_value(kind, ArgumentValue::Delimiter(value))
                    })
                    .boxed()
            }
        }
        ValueKind::Dimension => {
            let kind = ArgumentKind::from_required(spec.required);
            if spec.required {
                maybe_braced(dimension())
                    .map(move |value| Argument::from_value(kind, ArgumentValue::Dimension(value)))
                    .boxed()
            } else {
                optional_bracketed(dimension())
                    .map(move |opt| {
                        let value = opt.unwrap_or_default();
                        Argument::from_value(kind, ArgumentValue::Dimension(value))
                    })
                    .boxed()
            }
        }
        ValueKind::Integer => {
            let kind = ArgumentKind::from_required(spec.required);
            if spec.required {
                maybe_braced(integer())
                    .map(move |value| Argument::from_value(kind, ArgumentValue::Integer(value)))
                    .boxed()
            } else {
                optional_bracketed(integer())
                    .map(move |opt| {
                        let value = opt.unwrap_or_default();
                        Argument::from_value(kind, ArgumentValue::Integer(value))
                    })
                    .boxed()
            }
        }
        ValueKind::KeyVal => {
            let kind = ArgumentKind::from_required(spec.required);
            keyval_value(spec.required)
                .map(move |value| Argument::from_value(kind, ArgumentValue::KeyVal(value)))
                .boxed()
        }
    }
}

/// Parse a full argument list driven by metadata specs. This is the only custom loop in the argument layer.
fn arguments_parser<'a>(
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    specs: &'static [ArgSpec],
    strict: bool,
    context: &'static str,
) -> impl Parser<'a, TokenStream<'a>, Vec<Argument>, ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! { Token::ControlSeq(name) => name }
        .try_map(move |name, span| {
            if knowledge::is_blocklisted(&name) {
                return Err(Rich::custom(span, format!("Banned command: \\{}", name)));
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
fn env_name_parser<'a>() -> impl Parser<'a, TokenStream<'a>, (String, bool), ParserError<'a>> + Clone
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
) -> impl Parser<'a, TokenStream<'a>, (String, bool, Vec<Argument>, &'static EnvMeta), ParserError<'a>>
       + Clone {
    custom(move |input| {
        input.parse(control_seq("begin"))?;

        let name_start = input.cursor();
        let (base_name, starred) = input.parse(env_name_parser())?;
        let name_span = input.span_from_cursor(&name_start);

        let meta = match knowledge::lookup_env(base_name.as_str()) {
            Some(m) => m,
            None => {
                return Err(Rich::custom(
                    name_span,
                    format!("Unknown environment: {}", base_name),
                ));
            }
        };

        if starred && !meta.has_star_variant {
            return Err(Rich::custom(
                name_span,
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, starred, args, meta) = input.parse(parse_env_header(
            math_content.clone(),
            text_content.clone(),
            strict,
        ))?;

        let body_content = match meta.body_mode {
            ContentMode::Math => math_content.clone(),
            ContentMode::Text => text_content.clone(),
        };
        let body = input.parse(env_body_parser(meta.body_mode, body_content))?;

        let end_tag = just(Token::ControlSeq("end".into()))
            .ignore_then(env_name_parser())
            .labelled("environment end tag");

        let end_start = input.cursor();
        let (end_name, end_starred) = input.parse(end_tag)?;
        let end_span = input.span_from_cursor(&end_start);

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
                end_span,
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
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
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let atom_for_scripts = atom.clone().padded_by(ws.clone());
    // Script parsing is delegated to `parse_scripted_components` so that
    // the core state machine can be tested in isolation and kept small.
    custom(move |input| {
        let components = parse_scripted_components(input, atom_for_scripts.clone())?;

        if components.subscript.is_none() && components.superscript.is_none() {
            return Ok(components.base);
        }

        Ok(SyntaxNode::Scripted {
            base: Box::new(components.base),
            subscript: components.subscript.map(Box::new),
            superscript: components.superscript.map(Box::new),
        })
    })
}

/// Parse a text atom (text chunk, inline math, group, command, env).
fn text_atom_parser<'a, P>(
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let inline_math = just(Token::MathShift)
        .ignore_then(implicit_group_parser(
            ContentMode::Math,
            math_content.clone(),
        ))
        .then_ignore(just(Token::MathShift))
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
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
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

    let stop_declarative = declarative_guard();

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
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
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
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
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
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_declarative = declarative_guard();

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
pub(crate) fn math_block_parser<'a>(strict: bool) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(strict);
    math_parser
}

/// Entry point parser for text mode.
#[allow(dead_code)] // Text entry point is unused; expose when direct text parsing is needed
fn text_block_parser<'a>(strict: bool) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers(strict);
    text_parser
}
