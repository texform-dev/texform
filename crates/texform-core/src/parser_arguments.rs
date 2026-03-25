use chumsky::prelude::*;

use crate::column_parser::parse_column_template;
use crate::knowledge::{ArgForm, ArgSpec, DelimiterToken, KnowledgeBase, ValueKind};
use crate::lexer::Token;
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentSlot, ArgumentValue, ContentMode, Delimiter, GroupKind,
    SyntaxNode,
};

use super::{
    ArgumentParser, ContentParser, NodeParser, ParserError, ParserInput, ParserInputExt,
    TokenStream, braced_group_parser, build_token_stream, delimiter, insignificant_whitespace,
    math_block_parser, math_item_parser, maybe_braced, maybe_braced_or_empty, optional_bracketed,
    optional_bracketed_or_empty, text_block_parser, text_item_parser,
};

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
    nullable: bool,
) -> Result<ArgumentValue, Rich<'src, Token>> {
    match kind {
        ValueKind::Content { mode } => {
            let node = parse_tokens_as_content(input, kb, mode, tokens, strict)?;
            Ok(ArgumentValue::Content(node))
        }
        ValueKind::CSName => {
            let value = parse_tokens_as_cs_name(input, &tokens)?;
            Ok(ArgumentValue::CSName(value))
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
            if nullable && src.trim().is_empty() {
                return Ok(ArgumentValue::Delimiter(Delimiter::None));
            }
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

fn parse_tokens_as_cs_name<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    tokens: &[Token],
) -> Result<String, Rich<'src, Token>> {
    if tokens
        .iter()
        .any(|token| matches!(token, Token::ControlSeq(_)))
    {
        let cursor = input.cursor();
        return Err(input.err_peek_or_point(&cursor, "escape sequence should not appear in CSName"));
    }

    Ok(tokens_to_string(tokens))
}

pub(super) fn argument_parser<'a>(
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
                    let parser = if spec.nullable {
                        maybe_braced_or_empty(delimiter(kb), Delimiter::None).boxed()
                    } else {
                        maybe_braced(delimiter(kb)).boxed()
                    }
                    .map(move |value| {
                        Some(Argument::from_value(
                            ArgumentKind::Mandatory,
                            ArgumentValue::Delimiter(value),
                        ))
                    });
                    input.parse(parser)
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let parser = if spec.nullable {
                        optional_bracketed_or_empty(delimiter(kb), Delimiter::None).boxed()
                    } else {
                        optional_bracketed(delimiter(kb)).boxed()
                    }
                    .map(move |opt| {
                        opt.map(|value| {
                            Argument::from_value(
                                ArgumentKind::Optional,
                                ArgumentValue::Delimiter(value),
                            )
                        })
                    });
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
            ValueKind::CSName => {
                if spec.required {
                    let value = if matches!(input.peek(), Some(Token::LBrace)) {
                        let tokens = collect_delimited_tokens(
                            input,
                            &DelimiterToken::Char('{'),
                            &DelimiterToken::Char('}'),
                        )?;
                        parse_tokens_as_cs_name(input, &tokens)?
                    } else {
                        let cursor = input.cursor();
                        let token = input.next().ok_or_else(|| {
                            input.err_peek_or_point(&cursor, "missing required CSName argument")
                        })?;
                        parse_tokens_as_cs_name(input, std::slice::from_ref(&token))?
                    };

                    Ok(Some(Argument::from_value(
                        ArgumentKind::Mandatory,
                        ArgumentValue::CSName(value),
                    )))
                } else if !matches!(input.peek(), Some(Token::LBracket)) {
                    Ok(None)
                } else {
                    let Some(tokens) = collect_optional_bracketed_tokens(input, false)? else {
                        return Ok(None);
                    };
                    let value = parse_tokens_as_cs_name(input, &tokens)?;
                    Ok(Some(Argument::from_value(
                        ArgumentKind::Optional,
                        ArgumentValue::CSName(value),
                    )))
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
                if spec.required {
                    let cursor = input.cursor();
                    return Err(
                        input.err_peek_or_point(&cursor, "missing required braced group argument")
                    );
                }
                return Ok(None);
            }

            let tokens = collect_delimited_tokens(
                input,
                &DelimiterToken::Char('{'),
                &DelimiterToken::Char('}'),
            )?;
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict, spec.nullable)?;
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
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict, spec.nullable)?;
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
            let value = parse_delimited_value(input, kb, spec.kind, tokens, strict, spec.nullable)?;
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

pub(super) fn arguments_parser<'a>(
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

pub(crate) fn collect_optional_bracketed_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    match_brackets: bool,
) -> Result<Option<Vec<Token>>, Rich<'src, Token>> {
    if !matches!(input.peek(), Some(Token::LBracket)) {
        return Ok(None);
    }

    let start = input.cursor();
    input.next();

    let mut tokens = Vec::new();
    let mut brace_depth = 0usize;
    let mut bracket_depth = 0usize;

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed bracket argument")),
        };

        match token {
            Token::LBracket => {
                if match_brackets && brace_depth == 0 {
                    bracket_depth += 1;
                }
                tokens.push(Token::LBracket);
            }
            Token::RBracket => {
                if brace_depth == 0 {
                    if match_brackets && bracket_depth > 0 {
                        bracket_depth -= 1;
                        tokens.push(Token::RBracket);
                        continue;
                    }
                    break;
                }
                tokens.push(Token::RBracket);
            }
            Token::LBrace => {
                brace_depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if brace_depth == 0 {
                    return Err(input.err_since(&start, "unbalanced brace in bracket argument"));
                }
                brace_depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(Some(tokens))
}

fn collect_braced_tokens<'src, 'parse>(
    input: &mut ParserInput<'src, 'parse>,
    allow_nested: bool,
) -> Result<Vec<Token>, Rich<'src, Token>> {
    let start = input.cursor();
    match input.next() {
        Some(Token::LBrace) => {}
        _ => return Err(input.err_since(&start, "expected '{'")),
    }

    let mut tokens = Vec::new();
    let mut depth = 0usize;

    loop {
        let token = match input.next() {
            Some(token) => token,
            None => return Err(input.err_since(&start, "unclosed brace argument")),
        };

        match token {
            Token::LBrace => {
                if !allow_nested {
                    return Err(input.err_since(&start, "nested braces not allowed"));
                }
                depth += 1;
                tokens.push(Token::LBrace);
            }
            Token::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(Token::RBrace);
            }
            other => tokens.push(other),
        }
    }

    Ok(tokens)
}

fn tokens_to_string(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::ControlSeq(name) => {
                out.push('\\');
                out.push_str(name);
            }
            Token::Char(c) => out.push(*c),
            Token::Star => out.push('*'),
            Token::Alignment => out.push('&'),
            Token::MathShift => out.push('$'),
            Token::Parameter => out.push('#'),
            Token::Superscript => out.push('^'),
            Token::Subscript => out.push('_'),
            Token::Prime(count) => {
                for _ in 0..*count {
                    out.push('\'');
                }
            }
            Token::ActiveChar => out.push('~'),
            Token::LBracket => out.push('['),
            Token::RBracket => out.push(']'),
            Token::LBrace => out.push('{'),
            Token::RBrace => out.push('}'),
            Token::Whitespaces => out.push(' '),
            Token::Comment => {}
        }
    }
    out
}

fn validate_keyval(raw: &str) -> Result<(), &'static str> {
    let mut key = String::new();
    let mut value = String::new();
    let mut in_value = false;
    let mut brace_depth = 0usize;

    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                let target = if in_value { &mut value } else { &mut key };
                target.push('\\');
                if let Some(next) = chars.peek().copied() {
                    if next.is_ascii_alphabetic() {
                        while let Some(c) = chars.peek().copied() {
                            if c.is_ascii_alphabetic() {
                                target.push(c);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    } else {
                        target.push(next);
                        chars.next();
                    }
                }
            }
            '{' => {
                brace_depth += 1;
                if in_value {
                    value.push('{');
                } else {
                    key.push('{');
                }
            }
            '}' => {
                if brace_depth == 0 {
                    return Err("unbalanced brace in keyval");
                }
                brace_depth -= 1;
                if in_value {
                    value.push('}');
                } else {
                    key.push('}');
                }
            }
            '=' if brace_depth == 0 && !in_value => {
                if key.trim().is_empty() {
                    return Err("keyval missing key");
                }
                in_value = true;
            }
            ',' if brace_depth == 0 && in_value => {
                if value.trim().is_empty() {
                    return Err("keyval missing value");
                }
                key.clear();
                value.clear();
                in_value = false;
            }
            ',' if brace_depth == 0 && !in_value => {
                return Err("keyval missing value");
            }
            '=' if brace_depth == 0 && in_value => {
                value.push('=');
            }
            other => {
                if in_value {
                    value.push(other);
                } else {
                    key.push(other);
                }
            }
        }
    }

    if brace_depth != 0 {
        return Err("unbalanced brace in keyval");
    }

    if !in_value {
        return Err("keyval missing value");
    }

    if value.trim().is_empty() {
        return Err("keyval missing value");
    }

    Ok(())
}

fn normalize_keyval_string(raw: &str) -> String {
    raw.trim().to_string()
}

fn integer<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };

    sign.then(digit.repeated().at_least(1).collect::<Vec<char>>())
        .map(|(sign, digits)| {
            let mut out = String::with_capacity(digits.len() + 1);
            if let Some(s) = sign {
                out.push(s);
            }
            for d in digits {
                out.push(d);
            }
            out
        })
        .labelled("integer")
}

fn dimension<'a>() -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    let sign = select! { Token::Char(c @ ('+' | '-')) => c }.or_not();
    let digit = select! { Token::Char(c) if c.is_ascii_digit() => c };
    let sep = select! { Token::Char(c @ ('.' | ',')) => c };
    let ws = insignificant_whitespace();
    let alpha = select! { Token::Char(c) if c.is_ascii_alphabetic() => c };
    let unit = alpha.repeated().at_least(1).collect::<Vec<char>>();

    let int_digits = digit.repeated().collect::<Vec<char>>();
    let frac = sep.then(digit.repeated().collect::<Vec<char>>());

    sign.then(int_digits)
        .then(frac.or_not())
        .then_ignore(ws)
        .then(unit)
        .try_map(|(((sign, int_digits), frac), unit_chars), span| {
            let has_int = !int_digits.is_empty();
            let has_frac = frac.as_ref().is_some_and(|(_, ds)| !ds.is_empty());
            if !has_int && !has_frac {
                return Err(Rich::custom(span, "invalid dimension"));
            }
            let unit: String = unit_chars.into_iter().collect();
            if !is_valid_dimension_unit(&unit) {
                return Err(Rich::custom(span, "unsupported dimension unit"));
            }
            let mut value = String::new();
            if let Some(s) = sign {
                value.push(s);
            }
            for d in &int_digits {
                value.push(*d);
            }
            if let Some((_, frac_digits)) = frac {
                value.push('.');
                for d in &frac_digits {
                    value.push(*d);
                }
            }
            Ok(format!("{}{}", value, unit))
        })
        .labelled("dimension")
}

fn keyval_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            let start = input.cursor();
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected keyval argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            return Ok(String::new());
        };

        validate_keyval(&raw).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg)
        })?;

        Ok(normalize_keyval_string(&raw))
    })
}

fn column_spec_value<'a>(
    required: bool,
) -> impl Parser<'a, TokenStream<'a>, String, ParserError<'a>> + Clone {
    custom(move |input| {
        let raw = if required {
            let start = input.cursor();
            if !matches!(input.peek(), Some(Token::LBrace)) {
                return Err(input.err_since(&start, "expected column argument"));
            }
            let tokens = collect_braced_tokens(input, true)?;
            tokens_to_string(&tokens)
        } else if let Some(tokens) = collect_optional_bracketed_tokens(input, false)? {
            tokens_to_string(&tokens)
        } else {
            String::new()
        };
        let normalized = raw.trim().to_string();

        parse_column_template(&normalized).map_err(|msg| {
            let cursor = input.cursor();
            input.err_peek_or_point(&cursor, msg.to_string())
        })?;

        Ok(normalized)
    })
}

fn is_valid_dimension_unit(unit: &str) -> bool {
    matches!(
        unit,
        "em" | "ex" | "pt" | "pc" | "px" | "in" | "cm" | "mm" | "mu"
    )
}

pub(crate) fn normalize_argument_value(mode: ContentMode, node: SyntaxNode) -> SyntaxNode {
    match node {
        SyntaxNode::Group { children, .. } => fold_items(mode, children),
        other => other,
    }
}

pub(crate) fn fold_items(mode: ContentMode, items: Vec<SyntaxNode>) -> SyntaxNode {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_integer(src: &str) -> Result<String, ()> {
        let stream = build_token_stream(src);
        integer().parse(stream).into_result().map_err(|_| ())
    }

    fn parse_dimension(src: &str) -> Result<String, ()> {
        let stream = build_token_stream(src);
        dimension().parse(stream).into_result().map_err(|_| ())
    }

    fn parse_optional_tokens(src: &str, match_brackets: bool) -> Result<Option<Vec<Token>>, ()> {
        let stream = build_token_stream(src);
        custom(move |input| collect_optional_bracketed_tokens(input, match_brackets))
            .parse(stream)
            .into_result()
            .map_err(|_| ())
    }

    fn parse_delimited_tokens(src: &str) -> Result<Vec<Token>, ()> {
        let stream = build_token_stream(src);
        custom(|input| {
            collect_delimited_tokens(
                input,
                &DelimiterToken::Char('{'),
                &DelimiterToken::Char('}'),
            )
        })
        .parse(stream)
        .into_result()
        .map_err(|_| ())
    }

    #[test]
    fn integer_combinator() {
        assert_eq!(parse_integer("123").unwrap(), "123");
        assert_eq!(parse_integer("+42").unwrap(), "+42");
        assert_eq!(parse_integer("-0").unwrap(), "-0");
        assert!(parse_integer("abc").is_err());
        assert!(parse_integer("+").is_err());
    }

    #[test]
    fn dimension_combinator() {
        assert_eq!(parse_dimension("1em").unwrap(), "1em");
        assert_eq!(parse_dimension("1.5em").unwrap(), "1.5em");
        assert_eq!(parse_dimension("1,5em").unwrap(), "1.5em");
        assert_eq!(parse_dimension(".5pt").unwrap(), ".5pt");
        assert_eq!(parse_dimension("1.em").unwrap(), "1.em");
        assert!(parse_dimension("abc").is_err());
    }

    #[test]
    fn validate_keyval_reports_shape_errors() {
        assert!(validate_keyval("key=val").is_ok());
        assert!(validate_keyval("key={a,b},other=c").is_ok());
        assert!(validate_keyval("key=\\{,other=c").is_ok());
        assert!(validate_keyval("key=").is_err());
        assert!(validate_keyval("=value").is_err());
        assert!(validate_keyval("key={a").is_err());
    }

    #[test]
    fn optional_bracket_tokens_stop_at_first_top_level_closer() {
        let tokens = parse_optional_tokens("[a[b]", false).unwrap().unwrap();
        assert_eq!(tokens_to_string(&tokens), "a[b");
    }

    #[test]
    fn delimited_tokens_collect_nested_content() {
        let tokens = parse_delimited_tokens("{a{b}c}").unwrap();
        assert_eq!(tokens_to_string(&tokens), "a{b}c");
    }

    #[test]
    fn normalize_argument_value_unwraps_single_explicit_group() {
        let node = SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Explicit,
            children: vec![SyntaxNode::Char('x')],
        };

        assert_eq!(
            normalize_argument_value(ContentMode::Math, node),
            SyntaxNode::Char('x')
        );
    }
}
