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
    // KEY ARCHITECTURE CHANGE:
    // Instead of recursive(|normal_item| ...) returning normal_item definition,
    // we use recursive(|group_content_parser| ...) returning group_content_math.
    // This ensures the top-level uses Infix/Declarative logic, not simple .repeated().
    recursive(|group_content_parser| {
        // group_content_parser: Parser<Vec<SyntaxNode>>
        // This is a forward reference to the group_content_math we'll define below.

        // ====================================================================
        // Stage 4: GroupContent = Leading + [InfixTail] + [DeclarativeTail]
        // ====================================================================

        // First, we need to define normal_item (which uses group_content_parser for explicit_group)
        // Then we define group_content_math using normal_item

        // We'll build normal_item inline and use it in group_content_math

        // ====================================================================
        // Define Atom Parsers First (needed for normal_item)
        // ====================================================================

        // Explicit group: {...}
        // Uses group_content_parser for recursive parsing
        let explicit_group = just(&Token::LBrace)
            .ignore_then(group_content_parser.clone())
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
        let prefix_command = custom(move |input| {
            // 1. Peek command name (don't consume yet!)
            let name = match input.peek() {
                Some(Token::ControlSeq(name)) => name.clone(),
                _ => {
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        "not a command",
                    ))
                }
            };

            // 2. Check blacklist
            if let Some(reason) = knowledge::is_blacklisted(&name) {
                // For blacklisted commands, consume the token before erroring
                input.next();
                return Err(Rich::custom(
                    SimpleSpan::new((), 0..0),
                    format!("Banned command: \\{} ({})", name, reason),
                ));
            }

            // 3. Lookup in KB - check if it's a Prefix command
            let meta = match knowledge::lookup_command(&name) {
                Some(m) if m.kind == CommandKind::Prefix => m,
                Some(_) => {
                    // Not a prefix command (e.g., Infix or Declarative)
                    // Don't consume token, just fail softly
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        "not prefix",
                    ))
                }
                None if strict => {
                    // Unknown in strict mode - consume and error
                    input.next();
                    return Err(Rich::custom(
                        SimpleSpan::new((), 0..0),
                        format!("Unknown command: \\{}", name),
                    ))
                }
                None => {
                    // Non-strict mode: will be handled by unknown_command parser
                    return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown"));
                }
            };

            // 4. Now we know it's a valid Prefix command - consume the token
            input.next();

            // 5. Parse optional star
            let starred = if matches!(input.peek(), Some(Token::Star)) {
                input.next();
                true
            } else {
                false
            };

            // 6. Parse arguments based on meta.args
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

            // 7. Return command node
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
            if let Some(meta) = knowledge::lookup_command(&name) {
                // Known command - don't match it here
                return Err(Rich::custom(span, format!("known {} command", match meta.kind {
                    CommandKind::Prefix => "prefix",
                    CommandKind::Infix => "infix",
                    CommandKind::Declarative => "declarative",
                })));
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
        let normal_item = choice((scripted, atom));

        // ====================================================================
        // Now define group_content_math using normal_item
        // ====================================================================

        // group_content_math parses: Leading items + optional InfixTail + optional DeclarativeTail
        let group_content_math = {
            // Leading items: parse normal_item repeatedly, stopping before infix/declarative commands
            //
            // Solution: Use .not() combinator for zero-consumption lookahead
            // 1. Create a parser that matches infix/declarative commands
            // 2. Use .not() to invert it (succeeds when NOT infix/declarative)
            // 3. When .not() fails (i.e., we find infix/declarative), repeated() stops gracefully
            // 4. .not() doesn't consume tokens, so infix_tail can read them

            // Pattern that matches infix/declarative commands
            let stop_infix_or_decl = select! {
                Token::ControlSeq(name)
                    if knowledge::lookup_command(name.as_str())
                        .map(|m| matches!(m.kind, CommandKind::Infix | CommandKind::Declarative))
                        .unwrap_or(false) => ()
            };

            // Guard: .not() inverts the match (zero-consumption lookahead)
            // When we encounter infix/declarative, .not() fails and repeated() stops
            let guarded_item = stop_infix_or_decl.not().then(normal_item.clone()).map(|(_, item)| item);

            let leading = guarded_item.repeated().collect::<Vec<_>>();

            // InfixTail: infix_command + right_items
            // Returns (infix_info, right_items) where infix_info = (name, starred, args)
            let infix_tail = {
                // Parse infix command (returns marker with metadata)
                let infix_cmd = custom(move |input| {
                    // 1. Read command name
                    let name = match input.peek() {
                        Some(Token::ControlSeq(n)) => n.clone(),
                        _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
                    };
                    input.next(); // consume

                    // 2. Check blacklist
                    if let Some(reason) = knowledge::is_blacklisted(&name) {
                        return Err(Rich::custom(
                            SimpleSpan::new((), 0..0),
                            format!("Banned command: \\{} ({})", name, reason),
                        ));
                    }

                    // 3. Lookup in KB - must be Infix
                    let meta = match knowledge::lookup_command(&name) {
                        Some(m) if m.kind == CommandKind::Infix => m,
                        Some(_) => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not infix")),
                        None => return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown")),
                    };

                    // 4. Parse optional star
                    let starred = if meta.has_star_variant && matches!(input.peek(), Some(Token::Star)) {
                        input.next();
                        true
                    } else {
                        false
                    };

                    // 5. Parse args (usually empty for infix)
                    // TODO: Implement arg parsing for infix commands with args
                    let args = Vec::new();

                    Ok((name, starred, args))
                });

                // Right operand: at least one normal_item, stopping before declarative commands
                // Use .not() combinator for zero-consumption lookahead

                // Pattern that matches declarative commands
                let stop_declarative = select! {
                    Token::ControlSeq(name)
                        if knowledge::lookup_command(name.as_str())
                            .map(|m| m.kind == CommandKind::Declarative)
                            .unwrap_or(false) => ()
                };

                // Guard: .not() inverts the match (zero-consumption lookahead)
                // When we encounter declarative, .not() fails and repeated() stops
                let guarded_item_right = stop_declarative.not().then(normal_item.clone()).map(|(_, item)| item);

                let right_items = guarded_item_right.repeated().at_least(1).collect::<Vec<_>>();

                infix_cmd.then(right_items)
            };

            // DeclarativeTail: declarative_command + scope_items
            let declarative_tail = {
                // Parse declarative command
                let decl_cmd = custom(move |input| {
                    // 1. Read command name
                    let name = match input.peek() {
                        Some(Token::ControlSeq(n)) => n.clone(),
                        _ => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not a command")),
                    };
                    input.next(); // consume

                    // 2. Check blacklist
                    if let Some(reason) = knowledge::is_blacklisted(&name) {
                        return Err(Rich::custom(
                            SimpleSpan::new((), 0..0),
                            format!("Banned command: \\{} ({})", name, reason),
                        ));
                    }

                    // 3. Lookup in KB - must be Declarative
                    let meta = match knowledge::lookup_command(&name) {
                        Some(m) if m.kind == CommandKind::Declarative => m,
                        Some(_) => return Err(Rich::custom(SimpleSpan::new((), 0..0), "not declarative")),
                        None => return Err(Rich::custom(SimpleSpan::new((), 0..0), "unknown")),
                    };

                    // 4. Parse optional star
                    let starred = if meta.has_star_variant && matches!(input.peek(), Some(Token::Star)) {
                        input.next();
                        true
                    } else {
                        false
                    };

                    // 5. Parse args
                    // TODO: Implement arg parsing for declarative commands
                    let args = Vec::new();

                    Ok((name, starred, args))
                });

                // Scope: remaining items (can be empty)
                let scope_items = normal_item.clone().repeated().collect::<Vec<_>>();

                decl_cmd.then(scope_items)
            };

            // Combine: Leading + optional InfixTail + optional DeclarativeTail
            leading
                .then(infix_tail.or_not())
                .then(declarative_tail.or_not())
                .map(|((leading, infix_tail), declarative_tail)| {
                    // Build the final group based on what we found
                    //
                    // Logic:
                    // 1. If InfixTail exists, fold Leading as left and RightItems+DeclarativeTail as right
                    // 2. If only DeclarativeTail exists, append it to Leading
                    // 3. Otherwise return Leading as is

                    if let Some((infix_info, right_items)) = infix_tail {
                        // Have InfixTail: construct Infix node
                        let (infix_name, infix_starred, infix_args) = infix_info;

                        // Left operand: fold Leading items
                        let left = fold_items(ContentMode::Math, leading);

                        // Right operand: right_items + optional DeclarativeTail
                        let mut right_content = right_items;
                        if let Some((decl_tail_info, scope_items)) = declarative_tail {
                            let (decl_name, decl_starred, decl_args) = decl_tail_info;
                            let scope = fold_items(ContentMode::Math, scope_items);

                            let decl_node = SyntaxNode::Declarative {
                                name: decl_name,
                                starred: decl_starred,
                                args: decl_args,
                                scope: Box::new(scope),
                            };

                            right_content.push(decl_node);
                        }

                        let right = fold_items(ContentMode::Math, right_content);

                        // Return Infix node as single-element vector
                        return vec![SyntaxNode::Infix {
                            name: infix_name,
                            starred: infix_starred,
                            args: infix_args,
                            left: Box::new(left),
                            right: Box::new(right),
                        }];
                    }

                    // No InfixTail: check DeclarativeTail
                    let mut items = leading;

                    if let Some((decl_tail_info, scope_items)) = declarative_tail {
                        let (decl_name, decl_starred, decl_args) = decl_tail_info;
                        let scope = fold_items(ContentMode::Math, scope_items);

                        let decl_node = SyntaxNode::Declarative {
                            name: decl_name,
                            starred: decl_starred,
                            args: decl_args,
                            scope: Box::new(scope),
                        };

                        items.push(decl_node);
                    }

                    // Return items
                    items
                })
        };

        // ====================================================================
        // Return group_content_math (not normal_item!)
        // ====================================================================
        //
        // KEY: This is the architecture fix!
        // We return group_content_math which contains the full Infix/Declarative logic,
        // not a simple normal_item that would be .repeated() outside the closure.
        //
        // group_content_math: Parser<Vec<SyntaxNode>>
        group_content_math
    })
    // Wrap the Vec<SyntaxNode> into a top-level Group node
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

    // ========================================================================
    // Stage 4 Tests (Infix and Declarative commands)
    // ========================================================================

    #[test]
    fn test_infix_over_simple() {
        // "a \over b"
        let tokens = vec![
            Token::Char('a'),
            Token::ControlSeq("over".to_string()),
            Token::Char('b'),
        ];

        let result = match parse(&tokens, false) {
            Ok(r) => r,
            Err(errors) => {
                eprintln!("Parse errors:");
                for err in &errors {
                    eprintln!("  {:?}", err);
                }
                panic!("Parse failed with {} errors", errors.len());
            }
        };

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Infix {
                        name,
                        starred,
                        left,
                        right,
                        args,
                    } => {
                        assert_eq!(name, "over");
                        assert!(!starred);
                        assert!(args.is_empty());
                        assert_eq!(**left, SyntaxNode::Char('a'));
                        assert_eq!(**right, SyntaxNode::Char('b'));
                    }
                    _ => panic!("Expected Infix node, got {:?}", children[0]),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_infix_choose() {
        // "n \choose k"
        let tokens = vec![
            Token::Char('n'),
            Token::ControlSeq("choose".to_string()),
            Token::Char('k'),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Infix { name, left, right, .. } => {
                        assert_eq!(name, "choose");
                        assert_eq!(**left, SyntaxNode::Char('n'));
                        assert_eq!(**right, SyntaxNode::Char('k'));
                    }
                    _ => panic!("Expected Infix node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_infix_multiple_items() {
        // "a+b \over c+d"
        let tokens = vec![
            Token::Char('a'),
            Token::Char('+'),
            Token::Char('b'),
            Token::ControlSeq("over".to_string()),
            Token::Char('c'),
            Token::Char('+'),
            Token::Char('d'),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Infix { left, right, .. } => {
                        // Left should be folded into implicit group
                        match &**left {
                            SyntaxNode::Group { children, kind, .. } => {
                                assert_eq!(*kind, GroupKind::Implicit);
                                assert_eq!(children.len(), 3);
                                assert_eq!(children[0], SyntaxNode::Char('a'));
                                assert_eq!(children[1], SyntaxNode::Char('+'));
                                assert_eq!(children[2], SyntaxNode::Char('b'));
                            }
                            _ => panic!("Expected implicit group for left operand"),
                        }

                        // Right should be folded into implicit group
                        match &**right {
                            SyntaxNode::Group { children, kind, .. } => {
                                assert_eq!(*kind, GroupKind::Implicit);
                                assert_eq!(children.len(), 3);
                                assert_eq!(children[0], SyntaxNode::Char('c'));
                                assert_eq!(children[1], SyntaxNode::Char('+'));
                                assert_eq!(children[2], SyntaxNode::Char('d'));
                            }
                            _ => panic!("Expected implicit group for right operand"),
                        }
                    }
                    _ => panic!("Expected Infix node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_declarative_bfseries() {
        // "\bfseries text" -> Declarative with scope containing "text"
        let tokens = vec![
            Token::ControlSeq("bfseries".to_string()),
            Token::Char('t'),
            Token::Char('e'),
            Token::Char('x'),
            Token::Char('t'),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Declarative {
                        name,
                        starred,
                        args,
                        scope,
                    } => {
                        assert_eq!(name, "bfseries");
                        assert!(!starred);
                        assert!(args.is_empty());

                        // Scope should contain 4 chars
                        match &**scope {
                            SyntaxNode::Group { children, kind, .. } => {
                                assert_eq!(*kind, GroupKind::Implicit);
                                assert_eq!(children.len(), 4);
                            }
                            _ => panic!("Expected Group for scope"),
                        }
                    }
                    _ => panic!("Expected Declarative node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_declarative_with_leading() {
        // "a \bfseries b c"
        let tokens = vec![
            Token::Char('a'),
            Token::ControlSeq("bfseries".to_string()),
            Token::Char('b'),
            Token::Char('c'),
        ];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 2);

                // First item: 'a'
                assert_eq!(children[0], SyntaxNode::Char('a'));

                // Second item: Declarative
                match &children[1] {
                    SyntaxNode::Declarative { name, scope, .. } => {
                        assert_eq!(name, "bfseries");

                        match &**scope {
                            SyntaxNode::Group { children, .. } => {
                                assert_eq!(children.len(), 2);
                                assert_eq!(children[0], SyntaxNode::Char('b'));
                                assert_eq!(children[1], SyntaxNode::Char('c'));
                            }
                            _ => panic!("Expected Group for scope"),
                        }
                    }
                    _ => panic!("Expected Declarative node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }

    #[test]
    fn test_declarative_empty_scope() {
        // "\bfseries" with nothing after it
        let tokens = vec![Token::ControlSeq("bfseries".to_string())];

        let result = parse(&tokens, false).unwrap();

        match result {
            SyntaxNode::Group { children, .. } => {
                assert_eq!(children.len(), 1);

                match &children[0] {
                    SyntaxNode::Declarative { scope, .. } => {
                        // Scope should be empty implicit group
                        match &**scope {
                            SyntaxNode::Group { children, kind, .. } => {
                                assert_eq!(*kind, GroupKind::Implicit);
                                assert!(children.is_empty());
                            }
                            _ => panic!("Expected empty Group for scope"),
                        }
                    }
                    _ => panic!("Expected Declarative node"),
                }
            }
            _ => panic!("Expected root Group"),
        }
    }
}
