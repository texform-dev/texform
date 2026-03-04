//! Parser module - core combinator architecture
//!
//! - Content & arguments: mode content factories + argument_parser/arguments_parser
//! - Commands & environments: custom heads with combinator arguments/body
//! - Mode entry: mode_group_parsers + math_block_parser/text_block_parser

use chumsky::prelude::*;

use crate::column_parser::parse_column_template;
use crate::knowledge::{
    self, ArgForm, ArgSpec, CommandKind, CommandMeta, DelimiterToken, EnvMeta, KnowledgeBase,
    ValueKind,
};
use crate::lexer::Token;
use crate::parser_utils::{
    ParserError,
    ParserInput,
    ParserInputExt,
    Spanned,
    TokenStream,
    // Base parsers
    active_char,
    braced_group_parser,
    build_token_stream,
    collect_optional_bracketed_tokens,
    // Value combinators
    column_spec_value,
    control_seq,
    delimited_group_parser,
    delimiter,
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
    tokens_to_string,
    validate_keyval,
};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind,
    SyntaxNode,
};

type ContentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>>;
type NodeParser<'a> = Boxed<'a, 'a, TokenStream<'a>, SyntaxNode, ParserError<'a>>;
type ArgumentParser<'a> = Boxed<'a, 'a, TokenStream<'a>, ArgumentSlot, ParserError<'a>>;
type TailParseOutput = ((String, Vec<ArgumentSlot>), Vec<SyntaxNode>);

// ============================================================================
// Public Interface
// ============================================================================

/// Parse entry point - Math mode. Accepts source string directly.
/// Returns a `Spanned<SyntaxNode>` where the span covers the full input range.
pub fn parse(src: &str, strict: bool) -> Result<Spanned<SyntaxNode>, Vec<Rich<'_, Token>>> {
    let token_stream = build_token_stream(src);
    math_block_parser(knowledge::kb(), strict)
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
    kb: &'parse KnowledgeBase,
    expected_kind: CommandKind,
    current_mode: ContentMode,
    strict: bool,
) -> Result<(String, &'parse CommandMeta), Rich<'src, Token>> {
    let cmd_start = input.cursor();
    let token = input.next();
    let name = match token {
        Some(Token::ControlSeq(name)) => name,
        Some(_) => return Err(input.err_since(&cmd_start, "not a command")),
        None => return Err(input.err_since(&cmd_start, "not a command")),
    };

    let cmd_span = input.span_from_cursor(&cmd_start);

    let meta = match kb.lookup_command(&name) {
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

    if !meta.allowed_mode.allows(current_mode) {
        return Err(Rich::custom(
            cmd_span,
            format!("Command \\{} is not allowed in {} mode", name, current_mode),
        ));
    }

    Ok((name, meta))
}

// ============================================================================
// Content and Argument Parsers
// ============================================================================

/// Guard used to stop math content before infix/declarative commands.
fn math_infix_or_decl_guard<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str())
                .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                .unwrap_or(false) => ()
    }
}

/// Guard used to stop content parsing before declarative commands.
fn declarative_guard<'a>(
    kb: &'a KnowledgeBase,
) -> impl Parser<'a, TokenStream<'a>, (), ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str())
                .map(|m| m.kind == CommandKind::Declarative)
                .unwrap_or(false) => ()
    }
}

/// Parse one math item node (with script handling) without outer spacing policy.
///
/// Callers decide whether to wrap it with padding or stop-guards.
fn math_item_node_parser<'a, P>(
    kb: &'a KnowledgeBase,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let atom = math_atom_parser(kb, group_content, math_content, text_content, strict);
    scripted_atom_parser(atom)
}

/// Parse a single math item in argument contexts.
///
/// This parser does not consume trailing whitespace so the following argument
/// slot can still enforce `no_leading_space`.
fn math_item_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let item = math_item_node_parser(kb, math_content.clone(), math_content, text_content, strict);

    math_infix_or_decl_guard(kb)
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .ignore_then(item)
}

/// Parse a single text item (respecting stop guards).
fn text_item_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    let normal_item =
        text_atom_parser(kb, text_content.clone(), math_content, text_content, strict);

    declarative_guard(kb)
        .or(control_seq("end"))
        .not()
        .ignore_then(normal_item)
}

fn token_matches_delimiter(token: &Token, delimiter: &DelimiterToken) -> bool {
    match delimiter {
        DelimiterToken::Char('{') => matches!(token, Token::LBrace),
        DelimiterToken::Char('}') => matches!(token, Token::RBrace),
        DelimiterToken::Char('[') => matches!(token, Token::LBracket),
        DelimiterToken::Char(']') => matches!(token, Token::RBracket),
        DelimiterToken::Char(c) => matches!(token, Token::Char(tc) if *tc == *c),
        DelimiterToken::ControlSeq(name) => {
            matches!(token, Token::ControlSeq(token_name) if token_name == name.as_ref())
        }
    }
}

fn syntax_delimiter(delimiter: &'static DelimiterToken) -> Delimiter {
    match delimiter {
        DelimiterToken::Char(c) => Delimiter::Char(*c),
        DelimiterToken::ControlSeq(name) => Delimiter::Control(name.as_ref()),
    }
}

fn collect_delimited_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    open: &DelimiterToken,
    close: &DelimiterToken,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let start = input.cursor();
    let next = match input.peek() {
        Some(token) => token,
        None => return Err(input.err_since(&start, "expected delimited argument")),
    };
    if !token_matches_delimiter(&next, open) {
        return Err(input.err_since(&start, "missing opening delimiter"));
    }
    input.next();

    let allow_nested = open != close;
    let mut depth = 0usize;
    let mut tokens = Vec::new();

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed delimited argument")),
        };

        if allow_nested && token_matches_delimiter(&token, open) {
            depth += 1;
            tokens.push(token);
            continue;
        }

        if token_matches_delimiter(&token, close) {
            if allow_nested && depth > 0 {
                depth -= 1;
                tokens.push(token);
                continue;
            }
            break;
        }

        tokens.push(token);
    }

    Ok(tokens)
}

fn parse_tokens_as_content<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    kb: &'parse KnowledgeBase,
    mode: ContentMode,
    tokens: Vec<Token>,
    strict: bool,
) -> Result<SyntaxNode, Rich<'src, Token>> {
    let src = tokens_to_string(&tokens);
    let token_stream = build_token_stream(src.as_str());
    let parser = match mode {
        ContentMode::Math => math_block_parser(kb, strict),
        ContentMode::Text => text_block_parser(kb, strict),
    };

    let node = parser
        .then_ignore(end())
        .parse(token_stream)
        .into_result()
        .map_err(|_| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, "failed to parse delimited argument content")
        })?;

    Ok(normalize_argument_value(mode, node))
}

fn parse_delimited_value<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    kb: &'parse KnowledgeBase,
    kind: ValueKind,
    tokens: Vec<Token>,
    strict: bool,
) -> Result<ArgumentValue, Rich<'src, Token>> {
    match kind {
        ValueKind::Content { mode } => {
            let node = parse_tokens_as_content(input, kb, mode, tokens, strict)?;
            Ok(ArgumentValue::Content(node))
        }
        ValueKind::Dimension => {
            let src = tokens_to_string(&tokens);
            let value = insignificant_whitespace()
                .ignore_then(dimension())
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid dimension argument")
                })?;
            Ok(ArgumentValue::Dimension(value))
        }
        ValueKind::Integer => {
            let src = tokens_to_string(&tokens);
            let value = insignificant_whitespace()
                .ignore_then(integer())
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid integer argument")
                })?;
            Ok(ArgumentValue::Integer(value))
        }
        ValueKind::KeyVal => {
            let raw = tokens_to_string(&tokens);
            validate_keyval(raw.as_str()).map_err(|msg| {
                let cursor = input.cursor();
                input.err_peek_or_point(&cursor, msg)
            })?;
            Ok(ArgumentValue::KeyVal(raw.trim().to_string()))
        }
        ValueKind::Column => {
            let raw = tokens_to_string(&tokens);
            let normalized = raw.trim().to_string();
            parse_column_template(normalized.as_str()).map_err(|msg| {
                let cursor = input.cursor();
                input.err_peek_or_point(&cursor, msg.to_string())
            })?;
            Ok(ArgumentValue::Column(normalized))
        }
        ValueKind::Delimiter => {
            let src = tokens_to_string(&tokens);
            let value = insignificant_whitespace()
                .ignore_then(delimiter(kb))
                .then_ignore(insignificant_whitespace())
                .then_ignore(end())
                .parse(build_token_stream(src.as_str()))
                .into_result()
                .map_err(|_| {
                    let cursor = input.cursor();
                    input.err_peek_or_point(&cursor, "invalid delimiter argument")
                })?;
            Ok(ArgumentValue::Delimiter(value))
        }
        ValueKind::Star => {
            let cursor = input.cursor();
            Err(input.err_peek_or_point(
                &cursor,
                "invalid spec: star kind is not supported by delimited/paired forms",
            ))
        }
    }
}

/// Parse one argument slot according to `ArgSpec`.
fn argument_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    spec: &'static ArgSpec,
    strict: bool,
) -> ArgumentParser<'a> {
    custom(move |input| match &spec.form {
        ArgForm::Standard => match spec.kind {
            ValueKind::Content { mode } => {
                let content = match mode {
                    ContentMode::Math => math_content.clone(),
                    ContentMode::Text => text_content.clone(),
                };

                if spec.required {
                    let braced = braced_group_parser(mode, content.clone());
                    let single_item: NodeParser<'a> = match mode {
                        ContentMode::Math => {
                            math_item_parser(kb, math_content.clone(), text_content.clone(), strict)
                                .boxed()
                        }
                        ContentMode::Text => {
                            text_item_parser(kb, math_content.clone(), text_content.clone(), strict)
                                .boxed()
                        }
                    };
                    let parser = choice((braced, single_item))
                        .labelled("mandatory argument")
                        .map(move |node| {
                            Some(Argument::mandatory(normalize_argument_value(mode, node)))
                        });
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let Some(tokens) = collect_optional_bracketed_tokens(input, false)? else {
                        return Ok(None);
                    };
                    let node = parse_tokens_as_content(input, kb, mode, tokens, strict)?;
                    Ok(Some(Argument::from_value(
                        ArgumentKind::Optional,
                        ArgumentValue::Content(node),
                    )))
                }
            }
            ValueKind::Delimiter => {
                if spec.required {
                    let parser = maybe_braced(delimiter(kb))
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Delimiter(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = optional_bracketed(delimiter(kb))
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Delimiter(value),
                                )
                            })
                        })
                        .boxed();
                    input.parse(parser)
                }
            }
            ValueKind::Dimension => {
                if spec.required {
                    let parser = maybe_braced(dimension())
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Dimension(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = optional_bracketed(dimension())
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Dimension(value),
                                )
                            })
                        })
                        .boxed();
                    input.parse(parser)
                }
            }
            ValueKind::Integer => {
                if spec.required {
                    let parser = maybe_braced(integer())
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Integer(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = optional_bracketed(integer())
                        .map(move |opt| {
                            opt.map(|value| {
                                Argument::from_value(
                                    ArgumentKind::Optional,
                                    ArgumentValue::Integer(value),
                                )
                            })
                        })
                        .boxed();
                    input.parse(parser)
                }
            }
            ValueKind::KeyVal => {
                if spec.required {
                    let parser = keyval_value(true)
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::KeyVal(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = keyval_value(false)
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Optional,
                                ArgumentValue::KeyVal(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                }
            }
            ValueKind::Column => {
                if spec.required {
                    let parser = column_spec_value(true)
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Mandatory,
                                ArgumentValue::Column(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = column_spec_value(false)
                        .map(move |value| {
                            Some(Argument::from_value(
                                ArgumentKind::Optional,
                                ArgumentValue::Column(value),
                            ))
                        })
                        .boxed();
                    input.parse(parser)
                }
            }
            ValueKind::Star => {
                let cursor = input.cursor();
                Err(input.err_peek_or_point(&cursor, "invalid spec: star kind requires star form"))
            }
        },
        ArgForm::Star => {
            let present = matches!(input.peek(), Some(Token::Star));
            if present {
                input.next();
            }
            Ok(Some(Argument::star(present)))
        }
        ArgForm::Group => {
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Ok(None);
            }

            let tokens = collect_delimited_tokens(
                input,
                &DelimiterToken::Char('{'),
                &DelimiterToken::Char('}'),
            )?;
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict)?;
            Ok(Some(Argument::from_value(ArgumentKind::Group, value)))
        }
        ArgForm::Delimited { open, close } => {
            let has_open =
                matches!(input.peek(), Some(token) if token_matches_delimiter(&token, open));
            if !has_open {
                if spec.required {
                    let cursor = input.cursor();
                    return Err(
                        input.err_peek_or_point(&cursor, "missing required delimited argument")
                    );
                }
                return Ok(None);
            }

            let tokens = collect_delimited_tokens(input, open, close)?;
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict)?;
            Ok(Some(Argument::from_value(
                ArgumentKind::Delimited {
                    open: syntax_delimiter(open),
                    close: syntax_delimiter(close),
                },
                value,
            )))
        }
        ArgForm::Paired { pairs } => {
            let matched = input.peek().and_then(|next| {
                pairs
                    .iter()
                    .find(|(open, _)| token_matches_delimiter(&next, open))
            });

            let Some((open, close)) = matched else {
                if spec.required {
                    let cursor = input.cursor();
                    return Err(
                        input.err_peek_or_point(&cursor, "missing required paired argument")
                    );
                }
                return Ok(None);
            };

            let tokens = collect_delimited_tokens(input, open, close)?;
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict)?;
            Ok(Some(Argument::from_value(
                ArgumentKind::Paired {
                    open: syntax_delimiter(open),
                    close: syntax_delimiter(close),
                },
                value,
            )))
        }
    })
    .boxed()
}

/// Parse a full argument list driven by metadata specs. This is the only custom loop in the argument layer.
fn arguments_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    specs: &'static [ArgSpec],
    strict: bool,
    context: &'static str,
) -> impl Parser<'a, TokenStream<'a>, Vec<ArgumentSlot>, ParserError<'a>> + Clone {
    custom(move |input| {
        let mut args = Vec::with_capacity(specs.len());

        for spec in specs {
            if !spec.no_leading_space {
                let _ = input.parse(insignificant_whitespace());
            }
            let parser =
                argument_parser(kb, math_content.clone(), text_content.clone(), spec, strict)
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
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Prefix, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "command argument",
        ))?;

        Ok(SyntaxNode::Command { name, args })
    })
}

fn unknown_command_parser<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    select! {
        Token::ControlSeq(name)
            if kb.lookup_command(name.as_str()).is_none() => name
    }
    .try_map(move |name, span| {
        if matches!(name.as_str(), "begin" | "end") {
            return Err(Rich::custom(
                span,
                format!("Reserved environment delimiter: \\{}", name),
            ));
        }

        if strict {
            Err(Rich::custom(span, format!("Unknown command: \\{}", name)))
        } else {
            Ok(SyntaxNode::UnknownCommand { name })
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

    let is_star_variant = just(Token::Star).or_not().map(|s| s.is_some());

    base_name
        .then(is_star_variant)
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .labelled("environment name")
}

/// Parse `\begin{name}` plus its arguments, returning metadata.
fn parse_env_header<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, (String, bool, Vec<ArgumentSlot>, &'a EnvMeta), ParserError<'a>>
+ Clone {
    custom(move |input| {
        input.parse(control_seq("begin"))?;

        let name_start = input.cursor();
        let (base_name, is_star_variant) = input.parse(env_name_parser())?;
        let name_span = input.span_from_cursor(&name_start);

        let meta = match kb.lookup_env(base_name.as_str()) {
            Some(m) => m,
            None => {
                return Err(Rich::custom(
                    name_span,
                    format!("Unknown environment: {}", base_name),
                ));
            }
        };

        if !meta.allowed_mode.allows(current_mode) {
            return Err(Rich::custom(
                name_span,
                format!(
                    "Environment {} is not allowed in {} mode",
                    base_name, current_mode
                ),
            ));
        }

        if is_star_variant && !meta.has_star_variant {
            return Err(Rich::custom(
                name_span,
                format!("Environment {} has no starred variant", base_name),
            ));
        }

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "environment argument",
        ))?;

        Ok((base_name, is_star_variant, args, meta))
    })
}

/// Parse a full environment including body and closing tag.
fn environment_parser<'a>(
    kb: &'a KnowledgeBase,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone {
    custom(move |input| {
        let (name, is_star_variant, args, meta) = input.parse(parse_env_header(
            kb,
            math_content.clone(),
            text_content.clone(),
            current_mode,
            strict,
        ))?;

        let body_content = match meta.body_mode {
            ContentMode::Math => math_content.clone(),
            ContentMode::Text => text_content.clone(),
        };
        let body = input.parse(env_body_parser(meta.body_mode, body_content))?;

        let expected_end = if is_star_variant {
            format!("{name}*")
        } else {
            name.clone()
        };

        let end_start = input.cursor();
        input.parse(control_seq("end")).map_err(|_| {
            Rich::custom(
                input.span_from_cursor(&end_start),
                format!(
                    "Environment {} missing closing \\end{{{}}}",
                    expected_end, expected_end
                ),
            )
        })?;

        let (end_name, end_starred) = input.parse(env_name_parser()).map_err(|_| {
            Rich::custom(
                input.span_from_cursor(&end_start),
                format!(
                    "Environment {} missing closing \\end{{{}}}",
                    expected_end, expected_end
                ),
            )
        })?;
        let end_span = input.span_from_cursor(&end_start);

        if end_name != name || end_starred != is_star_variant {
            let found = if end_starred {
                format!("{end_name}*")
            } else {
                end_name.clone()
            };
            return Err(Rich::custom(
                end_span,
                format!(
                    "Environment name mismatch: expected \\end{{{}}}, found \\end{{{}}}",
                    expected_end, found
                ),
            ));
        }

        Ok(SyntaxNode::Environment {
            name,
            is_star_variant,
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
    kb: &'a KnowledgeBase,
    group_content: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone + 'a,
{
    let explicit_group = braced_group_parser(ContentMode::Math, group_content.clone());
    let delimited_group = delimited_group_parser(kb, math_content.clone());
    let environment = environment_parser(
        kb,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Math,
        strict,
    );
    let prefix_command =
        prefix_command_parser(kb, math_content, text_content, ContentMode::Math, strict);
    let unknown_command = unknown_command_parser(kb, strict);

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
///
/// This parser allows leading whitespace before script atoms but does not
/// consume trailing whitespace after the parsed item.
fn scripted_atom_parser<'a, P>(
    atom: P,
) -> impl Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let ws = insignificant_whitespace();
    let atom_for_scripts = ws.ignore_then(atom.clone());
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
    kb: &'a KnowledgeBase,
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
    let environment = environment_parser(
        kb,
        math_content.clone(),
        text_content.clone(),
        ContentMode::Text,
        strict,
    );
    let prefix_command =
        prefix_command_parser(kb, math_content, text_content, ContentMode::Text, strict);
    let unknown_command = unknown_command_parser(kb, strict);

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
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let infix_cmd = custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Infix, ContentMode::Math, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "infix command argument",
        ))?;

        Ok((name, args))
    });

    let stop_declarative = declarative_guard(kb);

    let guarded_item = stop_declarative
        .not()
        .then(normal_item)
        .map(|(_, item)| item);
    let right_items = guarded_item.repeated().at_least(1).collect::<Vec<_>>();

    infix_cmd.then(right_items)
}

/// Parse the tail of a declarative command: command head plus scoped items.
fn declarative_tail_parser<'a, P>(
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    current_mode: ContentMode,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, TailParseOutput, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let decl_cmd = custom(move |input| {
        let (name, meta) =
            match command_head_parser(input, kb, CommandKind::Declarative, current_mode, strict) {
                Ok(data) => data,
                Err(err) => return Err(err),
            };

        let args = input.parse(arguments_parser(
            kb,
            math_content.clone(),
            text_content.clone(),
            meta.args,
            strict,
            "declarative command argument",
        ))?;

        Ok((name, args))
    });

    let scope_items = normal_item.repeated().collect::<Vec<_>>();
    decl_cmd.then(scope_items)
}

/// Build math-mode group content (leading items + optional infix/declarative tails).
fn math_group_content_parser<'a, P>(
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_infix_or_decl = math_infix_or_decl_guard(kb);
    let guarded_item = stop_infix_or_decl
        .or(control_seq("right"))
        .or(control_seq("end"))
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let infix_tail = infix_tail_parser(
        kb,
        normal_item.clone(),
        math_content.clone(),
        text_content.clone(),
        strict,
    );

    let declarative_tail = declarative_tail_parser(
        kb,
        normal_item,
        math_content,
        text_content,
        ContentMode::Math,
        strict,
    );

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

                let (name, args) = infix_info;
                let left = fold_items(ContentMode::Math, leading);
                let right = fold_items(ContentMode::Math, right_items);

                let infix_node = SyntaxNode::Infix {
                    name,
                    args,
                    left: Box::new(left),
                    right: Box::new(right),
                };

                let mut nodes = vec![infix_node];
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    nodes.push(SyntaxNode::Declarative {
                        name: decl_name,
                        args: decl_args,
                        scope: Box::new(scope),
                    });
                }
                Ok(nodes)
            } else {
                let mut items = leading;
                if let Some((decl_info, scope_items)) = declarative_tail {
                    let (decl_name, decl_args) = decl_info;
                    let scope = fold_items(ContentMode::Math, scope_items);
                    items.push(SyntaxNode::Declarative {
                        name: decl_name,
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
    kb: &'a KnowledgeBase,
    normal_item: P,
    math_content: ContentParser<'a>,
    text_content: ContentParser<'a>,
    strict: bool,
) -> impl Parser<'a, TokenStream<'a>, Vec<SyntaxNode>, ParserError<'a>> + Clone
where
    P: Parser<'a, TokenStream<'a>, SyntaxNode, ParserError<'a>> + Clone + 'a,
{
    let stop_declarative = declarative_guard(kb);

    let guarded_item = stop_declarative
        .or(control_seq("end"))
        .not()
        .then(normal_item.clone())
        .map(|(_, item)| item);
    let leading = guarded_item.repeated().collect::<Vec<_>>();

    let declarative_tail = declarative_tail_parser(
        kb,
        normal_item,
        math_content,
        text_content,
        ContentMode::Text,
        strict,
    );

    leading
        .then(declarative_tail.or_not())
        .map(|(mut leading, declarative_tail)| {
            if let Some((decl_info, scope_items)) = declarative_tail {
                let (decl_name, decl_args) = decl_info;
                let scope = fold_items(ContentMode::Text, scope_items);
                leading.push(SyntaxNode::Declarative {
                    name: decl_name,
                    args: decl_args,
                    scope: Box::new(scope),
                });
            }
            leading
        })
}

/// Construct paired math/text content parsers using mutually recursive declarations.
fn mode_content_parsers<'a>(
    kb: &'a KnowledgeBase,
    strict: bool,
) -> (ContentParser<'a>, ContentParser<'a>) {
    let mut math = Recursive::declare();
    let mut text = Recursive::declare();

    let math_for_math = math.clone();
    let text_for_math = text.clone();
    math.define(recursive(move |group_content| {
        let ws = insignificant_whitespace();
        let math_content = math_for_math.clone().boxed();
        let text_content = text_for_math.clone().boxed();
        let normal_item = math_item_node_parser(
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        )
        .padded_by(ws.clone());
        math_group_content_parser(kb, normal_item, math_content, text_content, strict).padded_by(ws)
    }));

    let math_for_text = math.clone();
    let text_for_text = text.clone();
    text.define(recursive(move |group_content| {
        let math_content = math_for_text.clone().boxed();
        let text_content = text_for_text.clone().boxed();
        let normal_item = text_atom_parser(
            kb,
            group_content,
            math_content.clone(),
            text_content.clone(),
            strict,
        );
        text_group_content_parser(kb, normal_item, math_content, text_content, strict)
    }));

    (math.boxed(), text.boxed())
}

/// Construct top-level math/text group parsers from content parsers.
fn mode_group_parsers<'a>(kb: &'a KnowledgeBase, strict: bool) -> (NodeParser<'a>, NodeParser<'a>) {
    let (math_content, text_content) = mode_content_parsers(kb, strict);
    let math_group = implicit_group_parser(ContentMode::Math, math_content).boxed();
    let text_group = implicit_group_parser(ContentMode::Text, text_content).boxed();
    (math_group, text_group)
}

/// Entry point parser for math mode.
pub(crate) fn math_block_parser<'a>(kb: &'a KnowledgeBase, strict: bool) -> NodeParser<'a> {
    let (math_parser, _) = mode_group_parsers(kb, strict);
    math_parser
}

/// Entry point parser for text mode.
#[allow(dead_code)] // Text entry point is unused; expose when direct text parsing is needed
fn text_block_parser<'a>(kb: &'a KnowledgeBase, strict: bool) -> NodeParser<'a> {
    let (_, text_parser) = mode_group_parsers(kb, strict);
    text_parser
}
