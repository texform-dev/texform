//! Parser module: Token stream → SyntaxTree (Stage 1)
//!
//! This is the first stage of the two-stage parsing pipeline.
//! Uses chumsky parser combinators to build an immutable syntax tree.
//!
//! ## Implementation Strategy
//!
//! Following the design doc (3-parser.md), this parser:
//! 1. Uses knowledge base (KB) for command metadata lookup
//! 2. Supports strict/non-strict mode for unknown commands
//! 3. Implements starred variants, blacklist checking
//! 4. Provides generic argument parsing based on ArgSpec
//! 5. Supports both Math and Text modes with proper mode switching

use chumsky::prelude::*;

use crate::knowledge::{self, CommandKind};
use crate::lexer::Token;
use crate::syntax_node::{Argument, ArgumentKind, ContentMode, GroupKind, SyntaxNode};

/// Filter tokens to remove whitespace and comments
///
/// This should be called before parsing.
/// Note: In Text mode, whitespace is collected by text_chunk parser.
pub fn filter_tokens(tokens: &[Token]) -> Vec<Token> {
    tokens
        .iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::Comment(_)))
        .cloned()
        .collect()
}

/// Parse token stream into syntax tree
///
/// # Arguments
/// * `tokens` - Token stream from lexer (should be pre-filtered with `filter_tokens`)
/// * `strict` - If true, unknown commands cause errors; if false, produce UnknownCommand nodes
///
/// # Returns
/// * `Ok(SyntaxNode::Group)` - Root group in Math mode with Implicit kind
/// * `Err(errors)` - Parse errors with span information
pub fn parse(tokens: &[Token], strict: bool) -> Result<SyntaxNode, Vec<Rich<'_, Token>>> {
    parse_math_block(strict)
        .then_ignore(end())
        .parse(tokens)
        .into_result()
}

// ============================================================================
// Math Mode Parsing
// ============================================================================

/// Parse math mode block (returns Group node)
///
/// This is the entry point for math mode parsing.
/// Returns a Group node with mode=Math and kind=Implicit.
fn parse_math_block<'a>(
    strict: bool,
) -> impl Parser<'a, &'a [Token], SyntaxNode, extra::Err<Rich<'a, Token>>> + Clone {
    recursive(|normal_item| {
        // Helper: parse group content (list of normal items)
        let group_content = normal_item.clone().repeated().collect::<Vec<_>>();

        // ====================================================================
        // Atom Parsers
        // ====================================================================

        // Explicit group: {...}
        let explicit_group = just(&Token::LBrace)
            .ignore_then(group_content.clone())
            .then_ignore(just(&Token::RBrace))
            .map(|children| SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: GroupKind::Explicit,
                children,
            });

        // Escaped symbols: \%, \$, \&, \#, \_, \{, \}
        let escaped_symbol = select! {
            Token::ControlSeq(name) if matches!(name.as_str(), "%" | "$" | "&" | "#" | "_" | "{" | "}") => {
                let c = match name.as_str() {
                    "%" => '%',
                    "$" => '$',
                    "&" => '&',
                    "#" => '#',
                    "_" => '_',
                    "{" => '{',
                    "}" => '}',
                    _ => unreachable!(),
                };
                SyntaxNode::Char(c)
            }
        };

        // Active character: ~
        let active_char = just(&Token::ActiveChar).to(SyntaxNode::ActiveSpace);

        // Math characters
        let math_char = select! {
            Token::Char(c) => SyntaxNode::Char(c),
            Token::Star => SyntaxNode::Char('*'),
            Token::Alignment => SyntaxNode::Char('&'),
        };

        // ====================================================================
        // Command Parsing (Stage 3) - Generic Implementation using custom()
        // ====================================================================

        // Generic prefix command parser using KB and custom parser
        // This allows us to dynamically parse arguments based on CommandMeta.args
        let prefix_command = custom(move |input| {
            // 1. Read command name
            let name = match input.next() {
                Some(Token::ControlSeq(name)) => name.clone(),
                Some(tok) => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        format!("Expected command, got {:?}", tok),
                    ))
                }
                None => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        "Unexpected end of input",
                    ))
                }
            };

            // 2. Check blacklist
            if let Some(reason) = knowledge::is_blacklisted(&name) {
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Banned command: \\{} ({})", name, reason),
                ));
            }

            // 3. Lookup in KB
            let meta = match knowledge::lookup_command(&name) {
                Some(m) if m.kind == CommandKind::Prefix => m,
                Some(_) => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        format!("Not a prefix command: \\{}", name),
                    ))
                }
                None if strict => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        format!("Unknown command: \\{}", name),
                    ))
                }
                None => {
                    // Non-strict mode: will be handled by unknown_command parser
                    return Err(Rich::custom(SimpleSpan::new((), 0..0), "__UNKNOWN__"));
                }
            };

            // 4. Parse optional star
            let starred = if matches!(input.peek(), Some(Token::Star)) {
                input.next();
                true
            } else {
                false
            };

            // 5. Parse arguments based on meta.args
            let mut args = Vec::new();
            for arg_spec in meta.args {
                match arg_spec.kind {
                    ArgumentKind::Mandatory => {
                        // Expect {
                        match input.next() {
                            Some(Token::LBrace) => {}
                            Some(tok) => {
                                return Err(Rich::custom(
                                    SimpleSpan::new((), 0..0),
                                    format!("Expected '{{' for mandatory arg, got {:?}", tok),
                                ))
                            }
                            None => {
                                return Err(Rich::custom(
                                    SimpleSpan::new((), 0..0),
                                    "Expected '{'",
                                ))
                            }
                        }

                        // Parse content (recursively using group_content)
                        // For now, collect tokens until }
                        // TODO: Actually parse using group_content based on arg_spec.mode
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
                                        "Unclosed brace",
                                    ))
                                }
                            }
                        }

                        // Parse content recursively based on mode
                        let value = if arg_spec.mode == ContentMode::Text {
                            // Text mode: collect chars as text
                            // TODO: Implement proper text mode parsing in Stage 5
                            let text: String = content_tokens
                                .iter()
                                .filter_map(|t| {
                                    if let Token::Char(c) = t {
                                        Some(*c)
                                    } else {
                                        None
                                    }
                                })
                                .collect();
                            SyntaxNode::Text(text)
                        } else {
                            // Math mode: recursively parse content
                            // Parse the collected tokens using the same parser
                            match parse_math_block(strict)
                                .parse(&content_tokens)
                                .into_result()
                            {
                                Ok(SyntaxNode::Group { children, .. }) => {
                                    // Return the implicit group with parsed children
                                    SyntaxNode::Group {
                                        mode: ContentMode::Math,
                                        kind: GroupKind::Implicit,
                                        children,
                                    }
                                }
                                Ok(other) => other, // Shouldn't happen
                                Err(_) => {
                                    // Parsing failed, return error
                                    return Err(Rich::custom(
                                        SimpleSpan::new((), 0..0),
                                        "Failed to parse argument content",
                                    ));
                                }
                            }
                        };

                        args.push(Argument::mandatory(value));
                    }
                    ArgumentKind::Optional => {
                        // Check for [
                        if matches!(input.peek(), Some(Token::LBracket)) {
                            input.next(); // consume [

                            // Collect until ]
                            let mut content_tokens = Vec::new();
                            loop {
                                match input.next() {
                                    Some(Token::RBracket) => break,
                                    Some(tok) => content_tokens.push(tok.clone()),
                                    None => {
                                        return Err(Rich::custom(
                                            SimpleSpan::new((), 0..0),
                                            "Unclosed bracket",
                                        ))
                                    }
                                }
                            }

                            // Parse content recursively based on mode
                            let value = if arg_spec.mode == ContentMode::Text {
                                // Text mode: simple char collection
                                // TODO: Implement proper text mode parsing in Stage 5
                                let text: String = content_tokens
                                    .iter()
                                    .filter_map(|t| {
                                        if let Token::Char(c) = t {
                                            Some(*c)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                SyntaxNode::Text(text)
                            } else {
                                // Math mode: recursively parse
                                match parse_math_block(strict)
                                    .parse(&content_tokens)
                                    .into_result()
                                {
                                    Ok(SyntaxNode::Group { children, .. }) => SyntaxNode::Group {
                                        mode: arg_spec.mode,
                                        kind: GroupKind::Implicit,
                                        children,
                                    },
                                    Ok(other) => other,
                                    Err(_) => {
                                        return Err(Rich::custom(
                                            SimpleSpan::new((), 0..0),
                                            "Failed to parse optional argument",
                                        ))
                                    }
                                }
                            };

                            args.push(Argument::optional(value));
                        } else {
                            // Optional arg not provided: empty group
                            args.push(Argument::optional(SyntaxNode::Group {
                                mode: arg_spec.mode,
                                kind: GroupKind::Implicit,
                                children: vec![],
                            }));
                        }
                    }
                }
            }

            // 6. Return command node
            Ok(SyntaxNode::Command {
                name,
                starred,
                args,
            })
        });

        // Unknown command (non-strict mode)
        let unknown_command = select! {
            Token::ControlSeq(name) => name,
        }
        .try_map(move |name, span| {
            // Check if it's blacklisted
            if knowledge::is_blacklisted(&name).is_some() {
                return Err(Rich::custom(span, "blacklisted"));
            }

            // Check if it's in KB
            if knowledge::lookup_command(&name).is_some() {
                return Err(Rich::custom(span, "known command"));
            }

            // Unknown command in non-strict mode
            if !strict {
                Ok(SyntaxNode::UnknownCommand {
                    name,
                    starred: false,
                })
            } else {
                Err(Rich::custom(span, "strict mode"))
            }
        });

        // ====================================================================
        // Atom: priority order
        // ====================================================================

        // Try parsers in order (highest to lowest priority)
        let atom = choice((
            explicit_group,
            escaped_symbol,
            prefix_command,
            unknown_command,
            active_char,
            math_char,
        ));

        // ====================================================================
        // Scripted Expression
        // ====================================================================

        let scripted = atom
            .clone()
            .then(
                choice((
                    just(&Token::Superscript)
                        .ignore_then(atom.clone())
                        .map(|node| (true, node)),
                    just(&Token::Subscript)
                        .ignore_then(atom.clone())
                        .map(|node| (false, node)),
                ))
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>(),
            )
            .map(|(base, scripts)| {
                // Normalize scripts: collect last occurrence of each type
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

        // Normal item: scripted or atom
        choice((scripted, atom))
    })
    .repeated()
    .collect()
    .map(|children| SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Implicit,
        children,
    })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Fold a list of items into a single node
///
/// - Empty list: empty implicit group
/// - Single item: return that item
/// - Multiple items: wrap in implicit group
#[allow(dead_code)]
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Stage 1-2 Tests (Basic parsing)
    // ========================================================================

    #[test]
    fn test_parse_simple_chars() {
        let tokens = vec![Token::Char('a'), Token::Char('b'), Token::Char('c')];
        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                assert_eq!(mode, ContentMode::Math);
                assert_eq!(kind, GroupKind::Implicit);
                assert_eq!(children.len(), 3);
                assert_eq!(children[0], SyntaxNode::Char('a'));
                assert_eq!(children[1], SyntaxNode::Char('b'));
                assert_eq!(children[2], SyntaxNode::Char('c'));
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_parse_empty() {
        let tokens = vec![];
        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                assert_eq!(mode, ContentMode::Math);
                assert_eq!(kind, GroupKind::Implicit);
                assert!(children.is_empty());
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_escaped_symbols() {
        let tokens = vec![
            Token::ControlSeq("%".to_string()),
            Token::ControlSeq("$".to_string()),
            Token::ControlSeq("&".to_string()),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 3);
                assert_eq!(children[0], SyntaxNode::Char('%'));
                assert_eq!(children[1], SyntaxNode::Char('$'));
                assert_eq!(children[2], SyntaxNode::Char('&'));
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_active_char() {
        let tokens = vec![Token::Char('a'), Token::ActiveChar, Token::Char('b')];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 3);
                assert_eq!(children[0], SyntaxNode::Char('a'));
                assert_eq!(children[1], SyntaxNode::ActiveSpace);
                assert_eq!(children[2], SyntaxNode::Char('b'));
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_explicit_group() {
        let tokens = vec![Token::LBrace, Token::Char('a'), Token::RBrace];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                assert_eq!(mode, ContentMode::Math);
                assert_eq!(kind, GroupKind::Implicit);
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Group {
                        mode: inner_mode,
                        kind: inner_kind,
                        children: inner_children,
                    } => {
                        assert_eq!(*inner_mode, ContentMode::Math);
                        assert_eq!(*inner_kind, GroupKind::Explicit);
                        assert_eq!(inner_children.len(), 1);
                        assert_eq!(inner_children[0], SyntaxNode::Char('a'));
                    }
                    _ => panic!("Expected inner Group"),
                }
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_nested_groups() {
        let tokens = vec![
            Token::Char('a'),
            Token::LBrace,
            Token::Char('b'),
            Token::LBrace,
            Token::Char('c'),
            Token::RBrace,
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 2);
                assert_eq!(children[0], SyntaxNode::Char('a'));

                match &children[1] {
                    SyntaxNode::Group {
                        kind: GroupKind::Explicit,
                        children: inner1,
                        ..
                    } => {
                        assert_eq!(inner1.len(), 2);
                        assert_eq!(inner1[0], SyntaxNode::Char('b'));

                        match &inner1[1] {
                            SyntaxNode::Group {
                                kind: GroupKind::Explicit,
                                children: inner2,
                                ..
                            } => {
                                assert_eq!(inner2.len(), 1);
                                assert_eq!(inner2[0], SyntaxNode::Char('c'));
                            }
                            _ => panic!("Expected second level Group"),
                        }
                    }
                    _ => panic!("Expected first level Group"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_simple_script() {
        let tokens = vec![Token::Char('x'), Token::Superscript, Token::Char('2')];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Scripted {
                        base,
                        subscript,
                        superscript,
                    } => {
                        assert_eq!(**base, SyntaxNode::Char('x'));
                        assert!(subscript.is_none());
                        assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('2'));
                    }
                    _ => panic!("Expected Scripted node"),
                }
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_script_normalization() {
        // "x^a_b" and "x_b^a" should produce equivalent AST
        let tokens1 = vec![
            Token::Char('x'),
            Token::Superscript,
            Token::Char('a'),
            Token::Subscript,
            Token::Char('b'),
        ];

        let tokens2 = vec![
            Token::Char('x'),
            Token::Subscript,
            Token::Char('b'),
            Token::Superscript,
            Token::Char('a'),
        ];

        let result1 = parse(&tokens1, false).unwrap();
        let result2 = parse(&tokens2, false).unwrap();

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_script_duplicate_last_wins() {
        // "x^a^b" -> superscript should be 'b' (last wins)
        let tokens = vec![
            Token::Char('x'),
            Token::Superscript,
            Token::Char('a'),
            Token::Superscript,
            Token::Char('b'),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Scripted { superscript, .. } => {
                        assert_eq!(**superscript.as_ref().unwrap(), SyntaxNode::Char('b'));
                    }
                    _ => panic!("Expected Scripted node"),
                }
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_script_with_group() {
        // "x^{ab}" -> Scripted with group as superscript
        let tokens = vec![
            Token::Char('x'),
            Token::Superscript,
            Token::LBrace,
            Token::Char('a'),
            Token::Char('b'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Scripted {
                        base,
                        superscript,
                        subscript,
                    } => {
                        assert_eq!(**base, SyntaxNode::Char('x'));
                        assert!(subscript.is_none());

                        match superscript.as_ref().unwrap().as_ref() {
                            SyntaxNode::Group { children, .. } => {
                                assert_eq!(children.len(), 2);
                                assert_eq!(children[0], SyntaxNode::Char('a'));
                                assert_eq!(children[1], SyntaxNode::Char('b'));
                            }
                            _ => panic!("Expected Group as superscript"),
                        }
                    }
                    _ => panic!("Expected Scripted node"),
                }
            }
            _ => panic!("Expected Group node"),
        }
    }

    #[test]
    fn test_filter_tokens() {
        let tokens = vec![
            Token::Char('a'),
            Token::Whitespace,
            Token::Char('b'),
            Token::Whitespace,
            Token::Char('c'),
        ];

        let filtered = filter_tokens(&tokens);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0], Token::Char('a'));
        assert_eq!(filtered[1], Token::Char('b'));
        assert_eq!(filtered[2], Token::Char('c'));
    }

    #[test]
    fn test_filter_comments() {
        let tokens = vec![
            Token::Char('a'),
            Token::Comment("% test\n".to_string()),
            Token::Char('b'),
        ];

        let filtered = filter_tokens(&tokens);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0], Token::Char('a'));
        assert_eq!(filtered[1], Token::Char('b'));
    }

    // ========================================================================
    // Stage 3 Tests (Command parsing)
    // ========================================================================

    #[test]
    fn test_frac_command() {
        // "\frac{a}{b}"
        let tokens = vec![
            Token::ControlSeq("frac".to_string()),
            Token::LBrace,
            Token::Char('a'),
            Token::RBrace,
            Token::LBrace,
            Token::Char('b'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Command { name, starred, args } => {
                        assert_eq!(name, "frac");
                        assert!(!starred);
                        assert_eq!(args.len(), 2);

                        assert_eq!(args[0].kind, ArgumentKind::Mandatory);
                        assert_eq!(args[1].kind, ArgumentKind::Mandatory);
                    }
                    _ => panic!("Expected Command node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_sqrt_without_optional() {
        // "\sqrt{x}"
        let tokens = vec![
            Token::ControlSeq("sqrt".to_string()),
            Token::LBrace,
            Token::Char('x'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Command { name, args, .. } => {
                        assert_eq!(name, "sqrt");
                        assert_eq!(args.len(), 2);

                        // Optional arg should be empty
                        assert_eq!(args[0].kind, ArgumentKind::Optional);
                        match &args[0].value {
                            SyntaxNode::Group { children, .. } => {
                                assert!(children.is_empty());
                            }
                            _ => panic!("Expected Group in optional arg"),
                        }

                        // Mandatory arg
                        assert_eq!(args[1].kind, ArgumentKind::Mandatory);
                    }
                    _ => panic!("Expected Command node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_sqrt_with_optional() {
        // "\sqrt[3]{8}"
        let tokens = vec![
            Token::ControlSeq("sqrt".to_string()),
            Token::LBracket,
            Token::Char('3'),
            Token::RBracket,
            Token::LBrace,
            Token::Char('8'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Command { name, args, .. } => {
                        assert_eq!(name, "sqrt");
                        assert_eq!(args.len(), 2);

                        // Optional arg
                        assert_eq!(args[0].kind, ArgumentKind::Optional);
                        match &args[0].value {
                            SyntaxNode::Group { children, .. } => {
                                assert_eq!(children.len(), 1);
                                assert_eq!(children[0], SyntaxNode::Char('3'));
                            }
                            _ => panic!("Expected Group in optional arg"),
                        }
                    }
                    _ => panic!("Expected Command node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_text_command() {
        // "\text{hello}"
        let tokens = vec![
            Token::ControlSeq("text".to_string()),
            Token::LBrace,
            Token::Char('h'),
            Token::Char('e'),
            Token::Char('l'),
            Token::Char('l'),
            Token::Char('o'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Command { name, args, .. } => {
                        assert_eq!(name, "text");
                        assert_eq!(args.len(), 1);
                        match &args[0].value {
                            SyntaxNode::Text(s) => {
                                assert_eq!(s, "hello");
                            }
                            _ => panic!("Expected Text node"),
                        }
                    }
                    _ => panic!("Expected Command node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_nested_commands() {
        // "\frac{a}{\sqrt{b}}"
        let tokens = vec![
            Token::ControlSeq("frac".to_string()),
            Token::LBrace,
            Token::Char('a'),
            Token::RBrace,
            Token::LBrace,
            Token::ControlSeq("sqrt".to_string()),
            Token::LBrace,
            Token::Char('b'),
            Token::RBrace,
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                match &children[0] {
                    SyntaxNode::Command { name, args, .. } => {
                        assert_eq!(name, "frac");

                        // Second argument should contain \sqrt command
                        match &args[1].value {
                            SyntaxNode::Group { children, .. } => {
                                assert_eq!(children.len(), 1);
                                match &children[0] {
                                    SyntaxNode::Command { name, .. } => {
                                        assert_eq!(name, "sqrt");
                                    }
                                    _ => panic!("Expected nested Command"),
                                }
                            }
                            _ => panic!("Expected Group in arg 1"),
                        }
                    }
                    _ => panic!("Expected Command node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_unknown_command_nonstrict() {
        // "\unknown{x}" in non-strict mode
        let tokens = vec![
            Token::ControlSeq("unknown".to_string()),
            Token::LBrace,
            Token::Char('x'),
            Token::RBrace,
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 2);

                // First: UnknownCommand node
                match &children[0] {
                    SyntaxNode::UnknownCommand { name, starred } => {
                        assert_eq!(name, "unknown");
                        assert!(!starred);
                    }
                    _ => panic!("Expected UnknownCommand node"),
                }

                // Second: explicit group {x}
                match &children[1] {
                    SyntaxNode::Group {
                        kind: GroupKind::Explicit,
                        children,
                        ..
                    } => {
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Char('x'));
                    }
                    _ => panic!("Expected Group"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_unknown_command_strict() {
        // "\unknown{x}" in strict mode should error
        let tokens = vec![
            Token::ControlSeq("unknown".to_string()),
            Token::LBrace,
            Token::Char('x'),
            Token::RBrace,
        ];

        let result = parse(&tokens, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_blacklisted_command() {
        // "\ifnum" is blacklisted
        let tokens = vec![Token::ControlSeq("ifnum".to_string())];

        let result = parse(&tokens, false);
        assert!(result.is_err());
    }
}
