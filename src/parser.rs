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
