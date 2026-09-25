mod support;

use support::parser::{command_item, test_context, test_context_with_items};
use texform_core::{
    ast::{Argument, ArgumentKind, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId},
    parse::{AllowedMode, CommandKind, ParseContext},
    serialize::{
        AdjacentCharSpacing, CommandSpacing, EnvironmentNameSpacing, InfixGrouping,
        MathGroupInnerSpacing, ScriptOrder, ScriptSpacing, SerializationTokenKind,
        SerializeOptions, TokenizedLatex, serialize, serialize_tokenized, serialize_tokenized_with,
        serialize_with,
    },
};

fn assert_token_contract(result: &TokenizedLatex) {
    let mut cursor = 0;
    for token in &result.tokens {
        assert!(token.span.start < token.span.end);
        assert!(cursor <= token.span.start);
        assert!(
            result.latex[cursor..token.span.start]
                .chars()
                .all(char::is_whitespace)
        );
        assert_eq!(token.text, result.latex[token.span.clone()]);
        cursor = token.span.end;
    }
    assert!(result.latex[cursor..].chars().all(char::is_whitespace));
}

fn parse_to_ast(src: &str) -> texform_core::ast::Ast {
    let document = ParseContext::shared()
        .parse(src, &texform_core::parse::ParseConfig::STRICT)
        .try_into_document()
        .unwrap();
    Ast::from_syntax_root(&document.0.to_syntax())
}

fn parse_to_ast_with_context(ctx: &ParseContext, src: &str) -> texform_core::ast::Ast {
    let document = ctx
        .parse(src, &texform_core::parse::ParseConfig::STRICT)
        .try_into_document()
        .unwrap();
    Ast::from_syntax_root(&document.0.to_syntax())
}

#[test]
fn try_into_document_returns_diagnostics_present_when_partial_tree_has_errors() {
    let error = ParseContext::shared()
        .parse(
            r"\text{\frac{a}{b}}",
            &texform_core::parse::ParseConfig::default(),
        )
        .try_into_document()
        .expect_err("partial parses with diagnostics should not produce a document");

    assert!(error.document().is_some(), "expected partial document");
    assert!(
        !error.diagnostics().is_empty(),
        "expected parse diagnostics"
    );
}

#[test]
fn try_into_document_returns_no_document_when_strict_parse_fails() {
    let error = ParseContext::shared()
        .parse(r"\unknowncmd", &texform_core::parse::ParseConfig::STRICT)
        .try_into_document()
        .expect_err("strict parse failures should not produce a document");

    assert!(error.document().is_none(), "expected no document");
    assert!(
        !error.diagnostics().is_empty(),
        "expected parse diagnostics"
    );
}

#[test]
fn test_serialize_simple_math_chars() {
    let ast = parse_to_ast("ab");
    assert_eq!(serialize(&ast), "a b");
}

#[test]
fn tokenized_text_and_inline_math_use_semantic_modes() {
    let ast = parse_to_ast(r"\text{abc$x$}");
    let result = serialize_tokenized(&ast);
    assert_eq!(result.latex, serialize(&ast));
    assert_token_contract(&result);

    let tokens = result
        .tokens
        .iter()
        .map(|token| (token.text.as_str(), token.kind, token.mode))
        .collect::<Vec<_>>();
    assert_eq!(
        tokens,
        vec![
            (
                r"\text",
                SerializationTokenKind::ControlSequence,
                ContentMode::Math,
            ),
            ("{", SerializationTokenKind::Delimiter, ContentMode::Math),
            ("abc", SerializationTokenKind::Text, ContentMode::Text),
            ("$", SerializationTokenKind::Delimiter, ContentMode::Text),
            ("x", SerializationTokenKind::Character, ContentMode::Math),
            ("$", SerializationTokenKind::Delimiter, ContentMode::Text),
            ("}", SerializationTokenKind::Delimiter, ContentMode::Math),
        ]
    );
}

#[test]
fn tokenized_environment_and_scalar_wrappers_are_decomposed() {
    let ast = parse_to_ast(r"\begin{array}{lc}α&𝒜\end{array}");
    let result = serialize_tokenized(&ast);
    assert_eq!(result.latex, serialize(&ast));
    assert_token_contract(&result);

    let begin = &result.tokens[..5];
    assert_eq!(begin[0].text, r"\begin");
    assert_eq!(begin[0].kind, SerializationTokenKind::ControlSequence);
    assert_eq!(begin[1].text, "{");
    assert_eq!(begin[1].kind, SerializationTokenKind::Delimiter);
    assert_eq!(begin[2].text, "array");
    assert_eq!(begin[2].kind, SerializationTokenKind::Raw);
    assert_eq!(begin[3].text, "}");
    assert_eq!(begin[3].kind, SerializationTokenKind::Delimiter);
    assert_eq!(begin[4].text, "{");
    assert_eq!(begin[4].kind, SerializationTokenKind::Delimiter);
    assert!(
        result
            .tokens
            .iter()
            .any(|token| { token.text == "lc" && token.kind == SerializationTokenKind::Raw })
    );
    assert!(result.tokens.iter().any(|token| token.text == "𝒜"));
}

#[test]
fn escaped_text_chars_remain_single_character_tokens() {
    let result = serialize_tokenized(&parse_to_ast(r"\text{\%\$\{中}"));
    assert_token_contract(&result);
    let escaped = result
        .tokens
        .iter()
        .filter(|token| matches!(token.text.as_str(), r"\%" | r"\$" | r"\{"))
        .collect::<Vec<_>>();
    assert_eq!(escaped.len(), 3);
    assert!(
        escaped
            .iter()
            .all(|token| token.kind == SerializationTokenKind::Character)
    );
}

#[test]
fn escaped_ampersand_stays_literal_and_alignment_tabs_stay_bare() {
    // A literal `\&` must not lose its backslash in math mode, where a bare `&`
    // would become a column separator.
    let cases = [
        (r"a \& b", r"a \& b"),
        (r"{\displaystyle \&}", r"{ \displaystyle \& }"),
        (r"\mathrm{\&}", r"\mathrm { \& }"),
        (r"\text{a \& b}", r"\text {a \& b}"),
        (
            r"\begin{matrix}a \& b & c\\ d & \&\end{matrix}",
            r"\begin {matrix} a \& b & c \\ d & \& \end {matrix}",
        ),
        (
            r"\begin{align}x &= a \& b\end{align}",
            r"\begin {align} x & = a \& b \end {align}",
        ),
    ];
    for (source, expected) in cases {
        let serialized = serialize(&parse_to_ast(source));
        assert_eq!(serialized, expected, "source: {source}");
        assert_eq!(
            serialize(&parse_to_ast(&serialized)),
            serialized,
            "reparse of {serialized}"
        );
    }
}

#[test]
fn tokenized_options_never_change_canonical_text() {
    let ast = parse_to_ast(r"\sqrt[3]{x_i}");
    let options = SerializeOptions {
        script_spacing: ScriptSpacing::Compact,
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        command_spacing: CommandSpacing::Minimal,
        ..SerializeOptions::default()
    };

    let result = serialize_tokenized_with(&ast, &options);
    assert_eq!(result.latex, serialize_with(&ast, &options));
    assert_token_contract(&result);
    assert!(result.tokens.iter().any(|token| {
        token.text == "_"
            && token.kind == SerializationTokenKind::Character
            && token.mode == ContentMode::Math
    }));
}

#[test]
fn paired_argument_delimiters_use_wrapper_mode() {
    let mut ast = Ast::new();
    let text_group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    });
    let x = ast.new_node(Node::Char('x'));
    let paired = ast.new_node(Node::Command {
        name: "mark".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Paired {
                open: texform_core::ast::Delimiter::Char('|'),
                close: texform_core::ast::Delimiter::Char('|'),
            },
            no_leading_space: false,
            value: ArgumentValue::MathContent(x),
        })],
        known: true,
    });
    ast.append_child(text_group, paired);
    let wrapper = ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(text_group),
        })],
        known: true,
    });
    ast.append_child(ast.root(), wrapper);

    let result = serialize_tokenized(&ast);
    let delimiters = result
        .tokens
        .iter()
        .filter(|token| token.text == "|")
        .collect::<Vec<_>>();
    assert_eq!(delimiters.len(), 2);
    assert!(
        delimiters
            .iter()
            .all(|token| token.mode == ContentMode::Text)
    );
    assert!(
        result
            .tokens
            .iter()
            .any(|token| { token.text == "x" && token.mode == ContentMode::Math })
    );
}

#[test]
fn empty_error_snippet_has_no_zero_width_token() {
    for snippet in ["", r"\bad{"] {
        let mut ast = Ast::new();
        let error = ast.new_node(Node::Error {
            message: "unexpected".to_string(),
            snippet: snippet.to_string(),
        });
        ast.append_child(ast.root(), error);
        let result = serialize_tokenized(&ast);
        assert_token_contract(&result);
        if snippet.is_empty() {
            assert!(result.tokens.is_empty());
        } else {
            assert_eq!(result.tokens.len(), 1);
            assert_eq!(result.tokens[0].kind, SerializationTokenKind::Error);
        }
    }
}

#[test]
fn test_serialize_glues_adjacent_math_digits_only() {
    assert_eq!(serialize(&parse_to_ast("1093^2")), "1093 ^ { 2 }");
    assert_eq!(serialize(&parse_to_ast("abc")), "a b c");
}

#[test]
fn test_serialize_operatorname_argument_stays_compact() {
    let ctx = ParseContext::from_packages(&["ams", "base"]);

    assert_eq!(
        serialize(&parse_to_ast_with_context(
            &ctx,
            r"\operatorname{Effectiveness}"
        )),
        r"\operatorname {Effectiveness}"
    );
    assert_eq!(
        serialize(&parse_to_ast_with_context(
            &ctx,
            r"\operatorname{lambda-lift}"
        )),
        r"\operatorname {lambda-lift}"
    );
    assert_eq!(
        serialize(&parse_to_ast_with_context(&ctx, r"\operatorname{a+}")),
        r"\operatorname {a+}"
    );
}

#[test]
fn test_serialize_declare_math_operator_name_stays_compact() {
    let ctx = ParseContext::from_packages(&["ams", "base"]);
    let output = serialize(&parse_to_ast_with_context(
        &ctx,
        r"\DeclareMathOperator{diff}{Diff}",
    ));

    assert!(
        output.contains("{Diff}"),
        "operator name should stay compact: {output}"
    );
    assert!(
        !output.contains("{D i f f}"),
        "operator name should not be split: {output}"
    );
}

#[test]
fn test_serialize_regular_math_argument_still_uses_math_spacing() {
    let ast = parse_to_ast(r"\sqrt{abc}");

    assert_eq!(serialize(&ast), r"\sqrt { a b c }");
}

#[test]
fn test_serialize_tight_optional_argument_sticks_to_previous_slot() {
    assert_eq!(serialize(&parse_to_ast(r"\\[3pt]")), r"\\[3pt]");
    assert_eq!(serialize(&parse_to_ast(r"\\*[3pt]")), r"\\*[3pt]");

    let ctx = test_context();
    assert_eq!(
        serialize(&parse_to_ast_with_context(&ctx, r"\newline*[1cm]")),
        r"\newline*[1cm]"
    );
}

#[test]
fn test_serialize_custom_no_leading_space_optional_is_generic() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m !o",
    )]);

    assert_eq!(
        serialize(&parse_to_ast_with_context(&ctx, r"\probe a[b]")),
        r"\probe { a }[ b ]"
    );
}

#[test]
fn test_serialize_root_does_not_emit_extra_braces() {
    let mut ast = Ast::new();
    let root = ast.root();
    let left = ast.new_node(Node::Char('a'));
    let plus = ast.new_node(Node::Char('+'));
    let right = ast.new_node(Node::Char('b'));

    ast.append_child(root, left);
    ast.append_child(root, plus);
    ast.append_child(root, right);

    assert_eq!(serialize(&ast), "a + b");
}

#[test]
fn test_serialize_with_minimal_command_spacing() {
    let ast = parse_to_ast(r"\sqrt{a}");
    let options = SerializeOptions {
        command_spacing: CommandSpacing::Minimal,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"\sqrt{ a }");
}

#[test]
fn test_compact_math_group_inner_spacing_affects_command_wrapper_braces() {
    let ast = parse_to_ast(r"\sqrt{a}");
    let options = SerializeOptions {
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"\sqrt {a}");
}

#[test]
fn test_serialize_with_compact_adjacent_char_spacing() {
    let ast = parse_to_ast("a+b");
    let options = SerializeOptions {
        adjacent_char_spacing: AdjacentCharSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "a+b");
}

#[test]
fn test_serialize_manual_nodes_for_groups_and_literals() {
    let mut ast = Ast::new();
    let root = ast.root();

    let explicit = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let implicit = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    let x = ast.new_node(Node::Char('x'));
    let y = ast.new_node(Node::Char('y'));
    let unknown = ast.new_node(Node::Command {
        name: "mystery".to_string(),
        args: vec![],
        known: false,
    });
    let active_space = ast.new_node(Node::ActiveSpace);
    let text = ast.new_node(Node::Text("abc".to_string()));

    ast.append_child(explicit, x);
    ast.append_child(implicit, y);
    ast.append_child(root, explicit);
    ast.append_child(root, implicit);
    ast.append_child(root, unknown);
    ast.append_child(root, active_space);
    ast.append_child(root, text);

    assert_eq!(serialize(&ast), r"{ x } { y } \mystery ~ abc");
}

#[test]
fn test_serialize_command_argument_does_not_double_wrap_group_content() {
    let mut ast = Ast::new();
    let root = ast.root();

    let group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    let x = ast.new_node(Node::Char('x'));
    ast.append_child(group, x);

    let command = ast.new_node(Node::Command {
        name: "sqrt".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::MathContent(group),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\sqrt { x }");
}

#[test]
fn test_serialize_delimited_group_with_none_delimiter() {
    let ast = parse_to_ast(r"\left. x\right|");

    assert_eq!(serialize(&ast), r"\left . x \right |");
}

#[test]
fn test_serialize_delimited_group_with_square_brackets() {
    let ast = parse_to_ast(r"\left[a+b\right]");

    assert_eq!(serialize(&ast), r"\left [ a + b \right ]");
}

#[test]
fn test_serialize_scripted_nodes_use_sub_first_and_explicit_grouping() {
    let ast = parse_to_ast("x^2_i");

    assert_eq!(serialize(&ast), "x _ { i } ^ { 2 }");
}

#[test]
fn test_serialize_prime_superscript_uses_shorthand() {
    assert_eq!(serialize(&parse_to_ast("f'")), "f'");
    assert_eq!(serialize(&parse_to_ast("f''")), "f''");
}

#[test]
fn test_serialize_prime_superscript_respects_script_order() {
    assert_eq!(serialize(&parse_to_ast("f_n'")), "f _ { n }'");

    let ast = parse_to_ast("f_n'");
    let options = SerializeOptions {
        script_order: ScriptOrder::SupFirst,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "f' _ { n }");
}

#[test]
fn test_serialize_mixed_prime_superscript_keeps_script_group() {
    assert_eq!(serialize(&parse_to_ast("f'^2")), r"f ^ { \prime 2 }");
}

#[test]
fn prime_symbols_and_quote_superscripts_have_distinct_tokens() {
    let mut ast = Ast::new();
    let prime = ast.new_node(Node::Prime { count: 2 });
    ast.append_child(ast.root(), prime);
    let tokens = serialize_tokenized(&ast);
    assert_eq!(tokens.latex, serialize(&ast));
    assert_token_contract(&tokens);
    assert_eq!(tokens.tokens.len(), 2);
    assert!(tokens.tokens.iter().all(|token| {
        token.text == r"\prime"
            && token.kind == SerializationTokenKind::ControlSequence
            && token.mode == ContentMode::Math
    }));

    let quote = serialize_tokenized(&parse_to_ast("f''"));
    assert_eq!(quote.latex, "f''");
    assert!(
        quote
            .tokens
            .iter()
            .any(|token| { token.text == "''" && token.kind == SerializationTokenKind::Character })
    );
}

#[test]
fn bare_quote_and_nested_quote_preserve_their_empty_base() {
    for (source, expected) in [
        ("'", "{ }'"),
        ("''", "{ }''"),
        (r"f^{'}", "f ^ { { }' }"),
        (r"A^{'\alpha}", r"A ^ { { }' \alpha }"),
    ] {
        let ast = parse_to_ast(source);
        assert_eq!(serialize(&ast), expected, "{source}");
        assert_eq!(serialize_tokenized(&ast).latex, expected, "{source}");
    }

    let compact = SerializeOptions {
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        ..SerializeOptions::default()
    };
    assert_eq!(serialize_with(&parse_to_ast("'"), &compact), "{}'");
}

#[test]
fn grouped_prime_superscript_uses_prime_commands() {
    use texform_interface::syntax_node::{self as syntax, SyntaxNode};

    let syntax = SyntaxNode::Root {
        mode: ContentMode::Math,
        children: vec![SyntaxNode::Scripted {
            base: Box::new(SyntaxNode::Char('f')),
            subscript: None,
            superscript: Some(Box::new(SyntaxNode::Group {
                mode: ContentMode::Math,
                kind: syntax::GroupKind::Explicit,
                children: vec![SyntaxNode::prime(2)],
            })),
        }],
    };
    assert_eq!(
        serialize(&Ast::from_syntax_root(&syntax)),
        r"f ^ { \prime \prime }"
    );
}

#[test]
fn test_compact_math_group_inner_spacing_affects_script_wrapper_braces() {
    let ast = parse_to_ast("x^2_i");
    let options = SerializeOptions {
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "x _ {i} ^ {2}");
}

#[test]
fn test_serialize_with_sup_first_order() {
    let ast = parse_to_ast("x_i^2");
    let options = SerializeOptions {
        script_order: ScriptOrder::SupFirst,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "x ^ { 2 } _ { i }");
}

#[test]
fn test_implicit_and_explicit_groups_share_text_form() {
    let mut ast = Ast::new();
    let root = ast.root();
    let implicit = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    let explicit = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let x = ast.new_node(Node::Char('x'));
    let y = ast.new_node(Node::Char('y'));
    ast.append_child(implicit, x);
    ast.append_child(explicit, y);
    ast.append_child(root, implicit);
    ast.append_child(root, explicit);

    assert_eq!(serialize(&ast), "{ x } { y }");
}

#[test]
fn test_empty_group_uses_single_inner_padding_space() {
    let mut ast = Ast::new();
    let root = ast.root();
    let group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    ast.append_child(root, group);

    assert_eq!(serialize(&ast), "{ }");
}

#[test]
fn test_compact_math_group_inner_spacing_removes_brace_padding() {
    let ast = parse_to_ast("{} {a}");
    let options = SerializeOptions {
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "{} {a}");
}

#[test]
fn test_serialize_inline_math_keeps_text_mode_compact() {
    let ast = parse_to_ast(r"\text{ab$x$cd}");

    assert_eq!(serialize(&ast), r"\text {ab$x$cd}");
}

#[test]
fn test_serialize_text_argument_single_text_chunk_stays_compact() {
    let ast = parse_to_ast(r"\text{x}");

    assert_eq!(serialize(&ast), r"\text {x}");
}

#[test]
fn test_serialize_text_argument_preserves_edge_spaces() {
    let ast = parse_to_ast(r"\text{ or }");

    assert_eq!(serialize(&ast), r"\text { or }");
}

#[test]
fn test_serialize_mbox_argument_preserves_leading_space() {
    let ast = parse_to_ast(r"\mbox{ heads}");

    assert_eq!(serialize(&ast), r"\mbox { heads}");
}

#[test]
fn test_serialize_frac_and_text_argument() {
    let ast = parse_to_ast(r"\frac{a}{\text{abc}}");

    assert_eq!(serialize(&ast), r"\frac { a } { \text {abc} }");
}

#[test]
fn test_serialize_text_mode_single_char_argument_uses_text_content_variant() {
    let mut ast = Ast::new();
    let root = ast.root();
    let ch = ast.new_node(Node::Char('x'));
    let command = ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(ch),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\text {x}");
}

#[test]
fn test_serialize_scalar_arguments_stay_opaque() {
    let mut ast = Ast::new();
    let root = ast.root();
    let file = ast.new_node(Node::Text("file".to_string()));
    let command = ast.new_node(Node::Command {
        name: "includegraphics".to_string(),
        args: vec![
            Some(Argument {
                kind: ArgumentKind::Optional,
                no_leading_space: false,
                value: ArgumentValue::KeyVal("width=1em".to_string()),
            }),
            Some(Argument {
                kind: ArgumentKind::Mandatory,
                no_leading_space: false,
                value: ArgumentValue::TextContent(file),
            }),
        ],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\includegraphics [width=1em] {file}");
}

#[test]
fn test_serialize_other_scalar_argument_variants() {
    let mut ast = Ast::new();
    let root = ast.root();

    let label = ast.new_node(Node::Command {
        name: "label".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::CSName("sec:intro".to_string()),
        })],
        known: true,
    });
    let numeral = ast.new_node(Node::Command {
        name: "romannumeral".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Integer("12".to_string()),
        })],
        known: true,
    });
    let columns = ast.new_node(Node::Command {
        name: "arraycols".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Column("lcr".to_string()),
        })],
        known: true,
    });
    let delim = ast.new_node(Node::Command {
        name: "delim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Control(
                "langle".to_string(),
            )),
        })],
        known: true,
    });

    ast.append_child(root, label);
    ast.append_child(root, numeral);
    ast.append_child(root, columns);
    ast.append_child(root, delim);

    assert_eq!(
        serialize(&ast),
        r"\label {sec:intro} \romannumeral {12} \arraycols {lcr} \delim \langle"
    );
}

#[test]
fn test_serialize_mandatory_delimiter_emits_bare_token() {
    let mut ast = Ast::new();
    let root = ast.root();

    let control = ast.new_node(Node::Command {
        name: "delim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Control(
                "vert".to_string(),
            )),
        })],
        known: true,
    });
    let paren = ast.new_node(Node::Command {
        name: "delim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Char('(')),
        })],
        known: true,
    });
    let empty = ast.new_node(Node::Command {
        name: "delim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::None),
        })],
        known: true,
    });

    ast.append_child(root, control);
    ast.append_child(root, paren);
    ast.append_child(root, empty);

    assert_eq!(serialize(&ast), r"\delim \vert \delim ( \delim .");

    let result = serialize_tokenized(&ast);
    assert_eq!(result.latex, serialize(&ast));
    assert_token_contract(&result);
    assert!(result.tokens.iter().any(|token| {
        token.text == r"\vert" && token.kind == SerializationTokenKind::Delimiter
    }));
}

#[test]
fn test_serialize_group_and_optional_delimiter_keep_wrappers() {
    let mut ast = Ast::new();
    let root = ast.root();

    let group = ast.new_node(Node::Command {
        name: "gdelim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Group,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Char('|')),
        })],
        known: true,
    });
    let optional = ast.new_node(Node::Command {
        name: "odelim".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Optional,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Char('|')),
        })],
        known: true,
    });

    ast.append_child(root, group);
    ast.append_child(root, optional);

    assert_eq!(serialize(&ast), r"\gdelim {|} \odelim [|]");
}

#[test]
fn test_serialize_control_delimiter_keeps_space_before_letter() {
    let mut ast = Ast::new();
    let root = ast.root();
    let command = ast.new_node(Node::Command {
        name: "big".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::Delimiter(texform_core::ast::Delimiter::Control(
                "langle".to_string(),
            )),
        })],
        known: true,
    });
    let letter = ast.new_node(Node::Char('a'));
    ast.append_child(root, command);
    ast.append_child(root, letter);

    assert_eq!(serialize(&ast), r"\big \langle a");

    let result = serialize_tokenized(&ast);
    assert_eq!(result.latex, serialize(&ast));
    assert_token_contract(&result);
}

#[test]
fn test_serialize_paired_argument_replays_recorded_delimiters_and_skips_missing_slots() {
    let mut ast = Ast::new();
    let root = ast.root();
    let x = ast.new_node(Node::Char('x'));
    let command = ast.new_node(Node::Command {
        name: "qty".to_string(),
        args: vec![
            None,
            Some(Argument {
                kind: ArgumentKind::Paired {
                    open: texform_core::ast::Delimiter::Char('|'),
                    close: texform_core::ast::Delimiter::Char('|'),
                },
                no_leading_space: false,
                value: ArgumentValue::MathContent(x),
            }),
        ],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\qty | x |");
}

#[test]
fn test_serialize_star_slot_sticks_to_command_name() {
    let mut ast = Ast::new();
    let root = ast.root();
    let body = ast.new_node(Node::Char('x'));
    let command = ast.new_node(Node::Command {
        name: "operatorname".to_string(),
        args: vec![
            Some(Argument {
                kind: ArgumentKind::Star,
                no_leading_space: false,
                value: ArgumentValue::Boolean(true),
            }),
            Some(Argument {
                kind: ArgumentKind::Mandatory,
                no_leading_space: false,
                value: ArgumentValue::MathContent(body),
            }),
        ],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\operatorname* { x }");
}

#[test]
fn test_serialize_environment_uses_spaced_header_by_default() {
    let ast = parse_to_ast(r"\begin {matrix}a&b\\c&d\end {matrix}");

    assert_eq!(
        serialize(&ast),
        r"\begin {matrix} a & b \\ c & d \end {matrix}"
    );
}

#[test]
fn test_serialize_with_compact_environment_header() {
    let ast = parse_to_ast(r"\begin {matrix}ab\end {matrix}");
    let options = SerializeOptions {
        environment_name_spacing: EnvironmentNameSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(
        serialize_with(&ast, &options),
        r"\begin{matrix} a b \end{matrix}"
    );
}

#[test]
fn test_environment_name_spacing_is_independent_from_command_spacing() {
    let ast = parse_to_ast(r"\begin {matrix}ab\end {matrix}");
    let options = SerializeOptions {
        command_spacing: CommandSpacing::Minimal,
        environment_name_spacing: EnvironmentNameSpacing::Spaced,
        ..SerializeOptions::default()
    };

    assert_eq!(
        serialize_with(&ast, &options),
        r"\begin {matrix} a b \end {matrix}"
    );
}

#[test]
fn test_serialize_infix_node_without_transform() {
    let ast = parse_to_ast(r"a\over b");

    assert_eq!(serialize(&ast), r"a \over b");
}

#[test]
fn test_serialize_control_sequence_keeps_boundary_before_math_char() {
    let ast = parse_to_ast(r"\\x");

    assert_eq!(serialize(&ast), r"\\ x");
}

#[test]
fn test_serialize_flat_declarative_without_scope_wrapper() {
    let mut ast = Ast::new();
    let root = ast.root();
    let decl = ast.new_node(Node::Declarative {
        name: "bfseries".to_string(),
        args: Vec::new(),
    });
    let x = ast.new_node(Node::Char('x'));
    ast.append_child(root, decl);
    ast.append_child(root, x);

    assert_eq!(serialize(&ast), r"\bfseries x");
}

#[test]
fn test_serialize_infix_always_explicit_groups_operands() {
    let ast = parse_to_ast(r"a \over b");
    let options = SerializeOptions {
        infix_operand_grouping: InfixGrouping::AlwaysExplicit,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"{ a } \over { b }");
}

#[test]
fn test_serialize_infix_when_required_keeps_nested_braces() {
    let ast = parse_to_ast(r"{a \over b} \over c");
    let options = SerializeOptions {
        infix_operand_grouping: InfixGrouping::WhenRequired,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"{ a \over b } \over c");
}

#[test]
fn test_serialize_infix_when_required_keeps_flat_declarative_unbraced() {
    let ast = parse_to_ast(r"a \displaystyle b \over c");
    let options = SerializeOptions {
        infix_operand_grouping: InfixGrouping::WhenRequired,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"a \displaystyle b \over c");
}

#[test]
fn test_serialize_infix_empty_left_operand_stays_unbraced() {
    let ast = parse_to_ast(r"\over x");
    let explicit = SerializeOptions {
        infix_operand_grouping: InfixGrouping::AlwaysExplicit,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize(&ast), r"\over x");
    assert_eq!(serialize_with(&ast, &explicit), r"\over { x }");
}

#[test]
fn test_serialize_infix_empty_right_operand_stays_unbraced() {
    let ast = parse_to_ast(r"x \over");
    let explicit = SerializeOptions {
        infix_operand_grouping: InfixGrouping::AlwaysExplicit,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize(&ast), r"x \over");
    assert_eq!(serialize_with(&ast, &explicit), r"{ x } \over");
}

#[test]
fn test_serialize_environment_preserves_explicit_body_group() {
    let mut ast = Ast::new();
    let root = ast.root();
    let body = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode: ContentMode::Math,
    });
    let x = ast.new_node(Node::Char('x'));
    ast.append_child(body, x);
    let env = ast.new_node(Node::Environment {
        name: "matrix".to_string(),
        args: Vec::new(),
        known: true,
        body,
    });
    ast.append_child(root, env);

    assert_eq!(serialize(&ast), r"\begin {matrix} { x } \end {matrix}");
}

#[test]
fn test_serialize_environment_inside_text_mode_stays_compact() {
    let mut ast = Ast::new();
    let root = ast.root();
    let body = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    });
    let body_text = ast.new_node(Node::Text("x".to_string()));
    ast.append_child(body, body_text);

    let env = ast.new_node(Node::Environment {
        name: "quote".to_string(),
        args: Vec::new(),
        known: true,
        body,
    });

    let text_group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    });
    let left = ast.new_node(Node::Text("a".to_string()));
    let right = ast.new_node(Node::Text("b".to_string()));
    ast.append_child(text_group, left);
    ast.append_child(text_group, env);
    ast.append_child(text_group, right);

    let command = ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(text_group),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\text {a\begin {quote}x\end {quote}b}");
}

#[test]
fn test_serialize_scalar_paired_argument_keeps_math_spacing() {
    let mut ast = Ast::new();
    let root = ast.root();
    let command = ast.new_node(Node::Command {
        name: "qty".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Paired {
                open: texform_core::ast::Delimiter::Char('|'),
                close: texform_core::ast::Delimiter::Char('|'),
            },
            no_leading_space: false,
            value: ArgumentValue::Integer("12".to_string()),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\qty | 12 |");
}

#[test]
fn test_serialize_text_mode_control_word_keeps_text_boundary() {
    let mut ast = Ast::new();
    let root = ast.root();

    let text_group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    });
    let alpha = ast.new_node(Node::Command {
        name: "alpha".to_string(),
        args: Vec::new(),
        known: true,
    });
    let suffix = ast.new_node(Node::Text("x".to_string()));
    ast.append_child(text_group, alpha);
    ast.append_child(text_group, suffix);

    let command = ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(text_group),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\text {\alpha x}");
}

#[test]
fn test_serialize_paired_argument_unwraps_multi_item_content_group() {
    let mut ast = Ast::new();
    let root = ast.root();
    let content = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    });
    let x = ast.new_node(Node::Char('x'));
    let plus = ast.new_node(Node::Char('+'));
    let y = ast.new_node(Node::Char('y'));
    ast.append_child(content, x);
    ast.append_child(content, plus);
    ast.append_child(content, y);

    let command = ast.new_node(Node::Command {
        name: "qty".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Paired {
                open: texform_core::ast::Delimiter::Char('('),
                close: texform_core::ast::Delimiter::Char(')'),
            },
            no_leading_space: false,
            value: ArgumentValue::MathContent(content),
        })],
        known: true,
    });
    ast.append_child(root, command);

    assert_eq!(serialize(&ast), r"\qty ( x + y )");
}

#[test]
fn test_serialize_text_mode_paired_scalar_stays_compact() {
    let mut ast = Ast::new();
    let root = ast.root();

    let text_group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Text,
    });
    let left = ast.new_node(Node::Text("a".to_string()));
    let right = ast.new_node(Node::Text("b".to_string()));
    let command = ast.new_node(Node::Command {
        name: "mark".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Paired {
                open: texform_core::ast::Delimiter::Char('|'),
                close: texform_core::ast::Delimiter::Char('|'),
            },
            no_leading_space: false,
            value: ArgumentValue::Integer("12".to_string()),
        })],
        known: true,
    });
    ast.append_child(text_group, left);
    ast.append_child(text_group, command);
    ast.append_child(text_group, right);

    let wrapper = ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(text_group),
        })],
        known: true,
    });
    ast.append_child(root, wrapper);

    assert_eq!(serialize(&ast), r"\text {a\mark|12|b}");
}

#[test]
fn test_serialize_with_compact_script_spacing() {
    let ast = parse_to_ast("x^2_i");
    let options = SerializeOptions {
        script_spacing: ScriptSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), "x_{ i }^{ 2 }");
}

#[test]
fn test_serialize_minimal_command_spacing_compacts_left_right_delimiter() {
    let ast = parse_to_ast(r"\left (a+b\right )");
    let options = SerializeOptions {
        command_spacing: CommandSpacing::Minimal,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"\left( a + b \right)");
}

#[test]
fn test_compact_math_group_inner_spacing_affects_optional_argument_brackets() {
    let ast = parse_to_ast(r"\sqrt[3]{x}");
    let options = SerializeOptions {
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        ..SerializeOptions::default()
    };

    assert_eq!(serialize_with(&ast, &options), r"\sqrt [3] {x}");
}

#[test]
fn test_serialize_escaped_syntax_chars_round_trips_as_visible_chars() {
    let first = serialize(&parse_to_ast(r"\%\$\#\_\{\}"));
    let second = serialize(&parse_to_ast(&first));

    assert_eq!(first, r"\% \$ \# \_ \{ \}");
    assert_eq!(second, first);
}

#[test]
fn test_serialize_text_escaped_braces_round_trips_as_visible_chars() {
    let first = serialize(&parse_to_ast(r"\text{\{a\}}"));
    let second = serialize(&parse_to_ast(&first));

    assert_eq!(first, r"\text {\{a\}}");
    assert_eq!(second, first);
}

#[test]
fn test_serialize_is_text_idempotent_for_canonical_samples() {
    let samples = [
        (r"\frac{a}{b}", r"\frac { a } { b }"),
        ("x^2_i", "x _ { i } ^ { 2 }"),
        (r"\left (a+b\right )", r"\left ( a + b \right )"),
        (
            r"\begin {matrix}ab\end {matrix}",
            r"\begin {matrix} a b \end {matrix}",
        ),
    ];

    for (src, expected) in samples {
        let first = serialize(&parse_to_ast(src));
        let second = serialize(&parse_to_ast(&first));
        assert_eq!(first, expected);
        assert_eq!(second, first);
    }
}

fn all_serialize_option_combinations() -> Vec<SerializeOptions> {
    let mut options = Vec::with_capacity(128);
    for command_spacing in [CommandSpacing::Spaced, CommandSpacing::Minimal] {
        for group_inner_spacing in [
            MathGroupInnerSpacing::Padded,
            MathGroupInnerSpacing::Compact,
        ] {
            for adjacent_char_spacing in [AdjacentCharSpacing::Spaced, AdjacentCharSpacing::Compact]
            {
                for script_spacing in [ScriptSpacing::Spaced, ScriptSpacing::Compact] {
                    for script_order in [ScriptOrder::SubFirst, ScriptOrder::SupFirst] {
                        for infix_operand_grouping in
                            [InfixGrouping::WhenRequired, InfixGrouping::AlwaysExplicit]
                        {
                            for environment_name_spacing in [
                                EnvironmentNameSpacing::Spaced,
                                EnvironmentNameSpacing::Compact,
                            ] {
                                options.push(SerializeOptions {
                                    command_spacing,
                                    group_inner_spacing,
                                    adjacent_char_spacing,
                                    script_spacing,
                                    script_order,
                                    infix_operand_grouping,
                                    environment_name_spacing,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    options
}

#[test]
fn every_serialize_option_combination_matches_tokenized_latex() {
    let complete = [
        r"\sqrt[3]{x}",
        r"\text{a}",
        "123",
        "x_i^2",
        r"a \over b",
        r"\begin{pmatrix}a\end{pmatrix}",
        "{}",
        "f'",
        "f'^2",
    ]
    .map(parse_to_ast);
    let error = {
        let mut ast = Ast::new();
        let error = ast.new_node(Node::Error {
            message: "unexpected".to_string(),
            snippet: r"\bad{".to_string(),
        });
        ast.append_child(ast.root(), error);
        ast
    };
    let trees = complete.iter().chain(std::iter::once(&error));

    let combinations = all_serialize_option_combinations();
    assert_eq!(combinations.len(), 128);

    for ast in trees {
        for options in &combinations {
            let latex = serialize_with(ast, options);
            let tokenized = serialize_tokenized_with(ast, options);
            assert_eq!(tokenized.latex, latex);
            assert_token_contract(&tokenized);
        }
    }
}

#[test]
fn every_serialize_option_combination_is_text_idempotent_on_complete_formulas() {
    let sources = [
        r"\sqrt[3]{x}",
        r"\text{a}",
        "123",
        "x_i^2",
        r"a \over b",
        r"\begin{pmatrix}a\end{pmatrix}",
        "{}",
        "f'",
        "f'^2",
    ];
    let combinations = all_serialize_option_combinations();

    for src in sources {
        let ast = parse_to_ast(src);
        for options in &combinations {
            let first = serialize_with(&ast, options);
            let second = serialize_with(&parse_to_ast(&first), options);
            assert_eq!(second, first, "src={src:?} options={options:?}");
        }
    }
}

#[test]
fn serialize_options_serde_rejects_legacy_nested_objects() {
    let error = serde_json::from_str::<SerializeOptions>(r#"{"math":{}}"#).unwrap_err();
    assert!(
        error.to_string().contains("unknown field `math`"),
        "{error}"
    );
}

fn gap_before(result: &TokenizedLatex, index: usize) -> &str {
    let start = if index == 0 {
        0
    } else {
        result.tokens[index - 1].span.end
    };
    &result.latex[start..result.tokens[index].span.start]
}

fn token_at(result: &TokenizedLatex, text: &str) -> usize {
    result
        .tokens
        .iter()
        .position(|token| token.text == text)
        .unwrap_or_else(|| panic!("missing token {text} in {}", result.latex))
}

fn assert_parsed_idempotent(src: &str, expected: &str) {
    let first = serialize(&parse_to_ast(src));
    let second = serialize(&parse_to_ast(&first));
    assert_eq!(first, expected, "src={src}");
    assert_eq!(second, first, "src={src}");
}

fn command_node(name: &str) -> Node {
    Node::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

fn empty_error_node() -> Node {
    Node::Error {
        message: "empty".to_string(),
        snippet: String::new(),
    }
}

/// Add an explicit group holding `children` and return its id.
fn group_node(ast: &mut Ast, mode: ContentMode, children: &[NodeId]) -> NodeId {
    let group = ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode,
    });
    for child in children {
        ast.append_child(group, *child);
    }
    group
}

/// Add a `\text` command whose mandatory argument is `body`.
fn text_command_node(ast: &mut Ast, body: NodeId) -> NodeId {
    ast.new_node(Node::Command {
        name: "text".to_string(),
        args: vec![Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value: ArgumentValue::TextContent(body),
        })],
        known: true,
    })
}

/// Root in `mode` holding `nodes`, which the parser would never build this way.
fn ast_with(mode: ContentMode, nodes: Vec<Node>) -> Ast {
    let mut ast = Ast::with_root_mode(mode);
    let root = ast.root();
    for node in nodes {
        let id = ast.new_node(node);
        ast.append_child(root, id);
    }
    ast
}

/// `\text{...}` around `nodes`, in text mode.
fn text_wrapper(nodes: Vec<Node>) -> Ast {
    let mut ast = Ast::new();
    let children = nodes
        .into_iter()
        .map(|node| ast.new_node(node))
        .collect::<Vec<_>>();
    let body = group_node(&mut ast, ContentMode::Text, &children);
    let command = text_command_node(&mut ast, body);
    ast.append_child(ast.root(), command);
    ast
}

fn minimal_command_spacing() -> SerializeOptions {
    SerializeOptions {
        command_spacing: CommandSpacing::Minimal,
        ..SerializeOptions::default()
    }
}

/// Every optional space the options can remove, removed at once.
fn fully_compact() -> SerializeOptions {
    SerializeOptions {
        command_spacing: CommandSpacing::Minimal,
        group_inner_spacing: MathGroupInnerSpacing::Compact,
        adjacent_char_spacing: AdjacentCharSpacing::Compact,
        script_spacing: ScriptSpacing::Compact,
        ..SerializeOptions::default()
    }
}

#[test]
fn text_control_symbols_do_not_inject_body_whitespace() {
    // A text-mode control symbol is already terminated, so the serializer must
    // not separate it from whatever follows, whatever that is.
    for (src, expected) in [
        (r"\mbox{mod\ 1}", r"\mbox {mod\ 1}"),
        (r"\text{a\%b}", r"\text {a\%b}"),
        (r"\text{a\,b}", r"\text {a\,b}"),
        (r"\text{a\,1}", r"\text {a\,1}"),
        (r"\text{a\,.b}", r"\text {a\,.b}"),
        (r"\text{a\;b}", r"\text {a\;b}"),
        (r"\text{a\ b}", r"\text {a\ b}"),
        (r"\text{a\ 1}", r"\text {a\ 1}"),
        (r"\text{a\ .}", r"\text {a\ .}"),
    ] {
        assert_parsed_idempotent(src, expected);
    }

    // The thin space stays its own text-mode control-sequence token with no
    // gap before the letter that follows it.
    let thin = serialize_tokenized(&parse_to_ast(r"\text{a\,b}"));
    assert_eq!(thin.latex, r"\text {a\,b}");
    assert_token_contract(&thin);
    let comma = token_at(&thin, r"\,");
    assert_eq!(
        thin.tokens[comma].kind,
        SerializationTokenKind::ControlSequence
    );
    assert_eq!(thin.tokens[comma].mode, ContentMode::Text);
    assert_eq!(thin.tokens[comma + 1].text, "b");
    assert_eq!(gap_before(&thin, comma + 1), "");

    // An escaped character is a character token, not a control sequence.
    let escaped = serialize_tokenized(&parse_to_ast(r"\text{a\%b}"));
    assert_token_contract(&escaped);
    let percent = token_at(&escaped, r"\%");
    assert_eq!(
        escaped.tokens[percent].kind,
        SerializationTokenKind::Character
    );
    assert_eq!(escaped.tokens[percent].mode, ContentMode::Text);
}

#[test]
fn control_space_stays_distinct_from_ordinary_text_whitespace() {
    assert_parsed_idempotent(r"\mbox{mod\  1}", r"\mbox {mod\  1}");

    let result = serialize_tokenized(&parse_to_ast(r"\mbox{mod\  1}"));
    assert_eq!(result.latex, r"\mbox {mod\  1}");
    assert_token_contract(&result);
    let control_space = &result.tokens[token_at(&result, "\\ ")];
    assert_eq!(control_space.kind, SerializationTokenKind::ControlSequence);
    assert_eq!(control_space.mode, ContentMode::Text);
    let following = &result.tokens[token_at(&result, " 1")];
    assert_eq!(following.kind, SerializationTokenKind::Text);
    assert_eq!(control_space.span.end, following.span.start);
}

#[test]
fn handcrafted_control_word_keeps_lexical_separator_under_compact_options() {
    // Transforms can place a letter straight after a control word. The
    // separator is lexical, so no spacing option may remove it.
    let math = ast_with(
        ContentMode::Math,
        vec![command_node("alpha"), Node::Char('x')],
    );
    let text = text_wrapper(vec![command_node("dagger"), Node::Char('x')]);
    for options in [
        SerializeOptions::default(),
        minimal_command_spacing(),
        fully_compact(),
    ] {
        assert_eq!(serialize_with(&math, &options), r"\alpha x");
        let text_expected = match options.command_spacing {
            CommandSpacing::Spaced => r"\text {\dagger x}",
            CommandSpacing::Minimal => r"\text{\dagger x}",
        };
        assert_eq!(serialize_with(&text, &options), text_expected);
    }

    let mut grouped = Ast::new();
    let command = grouped.new_node(command_node("alpha"));
    let letter = grouped.new_node(Node::Char('x'));
    let group = group_node(&mut grouped, ContentMode::Math, &[command, letter]);
    grouped.append_child(grouped.root(), group);
    assert_eq!(serialize_with(&grouped, &fully_compact()), r"{\alpha x}");

    // The separator is a real gap between two tokens, and it survives a round
    // trip through the parser.
    let tokenized = serialize_tokenized(&math);
    assert_eq!(tokenized.latex, r"\alpha x");
    assert_token_contract(&tokenized);
    assert_eq!(tokenized.tokens.len(), 2);
    assert_eq!(gap_before(&tokenized, 1), " ");
    assert_eq!(tokenized.tokens[1].mode, ContentMode::Math);
    assert_eq!(serialize(&parse_to_ast(&tokenized.latex)), r"\alpha x");
    assert_eq!(
        serialize(&parse_to_ast(&serialize(&text))),
        r"\text {\dagger x}"
    );
}

#[test]
fn handcrafted_control_word_does_not_add_lexical_space_before_digit_whitespace_or_structure() {
    // Only an ASCII letter can extend a control word. Digits, whitespace, and
    // braces close it, so no lexical separator is due before them.
    let math_digit = ast_with(
        ContentMode::Math,
        vec![command_node("alpha"), Node::Char('1')],
    );
    assert_eq!(serialize(&math_digit), r"\alpha 1");
    assert_eq!(serialize(&parse_to_ast(r"\alpha 1")), r"\alpha 1");

    let text_digit = text_wrapper(vec![command_node("dagger"), Node::Char('1')]);
    assert_eq!(serialize(&text_digit), r"\text {\dagger1}");
    assert_eq!(
        serialize(&parse_to_ast(&serialize(&text_digit))),
        r"\text {\dagger1}"
    );

    let text_space = text_wrapper(vec![command_node("dagger"), Node::Text("  x".to_string())]);
    assert_eq!(serialize(&text_space), r"\text {\dagger  x}");

    let mut nested = Ast::new();
    let dagger = nested.new_node(command_node("dagger"));
    let letter = nested.new_node(Node::Char('x'));
    let inner = group_node(&mut nested, ContentMode::Text, &[letter]);
    let body = group_node(&mut nested, ContentMode::Text, &[dagger, inner]);
    let text = text_command_node(&mut nested, body);
    nested.append_child(nested.root(), text);
    assert_eq!(serialize(&nested), r"\text {\dagger{x}}");
    assert_eq!(
        serialize_with(&nested, &minimal_command_spacing()),
        r"\text{\dagger{x}}"
    );

    let mut math_group = Ast::new();
    let alpha = math_group.new_node(command_node("alpha"));
    let digit = math_group.new_node(Node::Char('1'));
    let brace = group_node(&mut math_group, ContentMode::Math, &[digit]);
    math_group.append_child(math_group.root(), alpha);
    math_group.append_child(math_group.root(), brace);
    assert_eq!(serialize(&math_group), r"\alpha { 1 }");
    assert_eq!(
        serialize_with(&math_group, &minimal_command_spacing()),
        r"\alpha{ 1 }"
    );
}

#[test]
fn starprobe_star_does_not_inject_text_space() {
    // The star closes the control word, so the following letter stays glued in
    // text mode and only math command spacing may separate it.
    let ctx = test_context_with_items([command_item(
        "starprobe",
        CommandKind::Prefix,
        AllowedMode::Both,
        "s",
    )]);
    let text = parse_to_ast_with_context(&ctx, r"\text{\starprobe*x}");
    assert_eq!(serialize(&text), r"\text {\starprobe*x}");
    assert_eq!(
        serialize_with(&text, &minimal_command_spacing()),
        r"\text{\starprobe*x}"
    );

    let tokenized = serialize_tokenized(&text);
    assert_eq!(tokenized.latex, serialize(&text));
    assert_token_contract(&tokenized);
    let star = token_at(&tokenized, "*");
    assert_eq!(tokenized.tokens[star - 1].text, r"\starprobe");
    assert_eq!(
        tokenized.tokens[star].kind,
        SerializationTokenKind::Character
    );
    assert_eq!(tokenized.tokens[star].mode, ContentMode::Text);
    assert_eq!(gap_before(&tokenized, star), "");
    assert_eq!(tokenized.tokens[star + 1].text, "x");
    assert_eq!(gap_before(&tokenized, star + 1), "");

    let math = parse_to_ast_with_context(&ctx, r"\starprobe*x");
    assert_eq!(serialize(&math), r"\starprobe* x");
    assert_eq!(
        serialize_with(&math, &minimal_command_spacing()),
        r"\starprobe* x"
    );
}

#[test]
fn minimal_operatorname_star_keeps_math_space_after_argument() {
    // The star glues to the name and the argument stays compact, but the math
    // atom after the argument still needs its optional space.
    let ast = parse_to_ast(r"\operatorname*{x}y");
    let options = minimal_command_spacing();
    assert_eq!(serialize_with(&ast, &options), r"\operatorname*{x} y");
    assert_eq!(
        serialize_with(&parse_to_ast(&serialize_with(&ast, &options)), &options),
        r"\operatorname*{x} y"
    );

    let tokenized = serialize_tokenized_with(&ast, &options);
    assert_eq!(tokenized.latex, r"\operatorname*{x} y");
    assert_token_contract(&tokenized);
    let star = token_at(&tokenized, "*");
    assert_eq!(tokenized.tokens[star - 1].text, r"\operatorname");
    assert_eq!(gap_before(&tokenized, star), "");
    // The operator name is text mode; the sibling after it is back in math.
    assert_eq!(
        tokenized.tokens[token_at(&tokenized, "x")].mode,
        ContentMode::Text
    );
    let follower = token_at(&tokenized, "y");
    assert_eq!(tokenized.tokens[follower].mode, ContentMode::Math);
    assert_eq!(gap_before(&tokenized, follower), " ");
}

#[test]
fn direct_output_paths_keep_a_single_written_space() {
    // Environment heads and padded empty groups append their space directly.
    // That byte is the emitted ending, so no second separator may follow.
    let matrix = parse_to_ast(r"\begin {matrix}x\end {matrix}");
    assert_eq!(serialize(&matrix), r"\begin {matrix} x \end {matrix}");
    let tokenized = serialize_tokenized(&matrix);
    assert_eq!(tokenized.latex, serialize(&matrix));
    assert_token_contract(&tokenized);
    let begin = token_at(&tokenized, r"\begin");
    assert_eq!(tokenized.tokens[begin + 1].text, "{");
    assert_eq!(gap_before(&tokenized, begin + 1), " ");
    let compact_name = SerializeOptions {
        environment_name_spacing: EnvironmentNameSpacing::Compact,
        ..SerializeOptions::default()
    };
    assert_eq!(
        serialize_with(&matrix, &compact_name),
        r"\begin{matrix} x \end{matrix}"
    );

    let empty = serialize_tokenized(&parse_to_ast("{}"));
    assert_eq!(empty.latex, "{ }");
    assert_token_contract(&empty);
    assert_eq!(gap_before(&empty, 1), " ");

    let frac = parse_to_ast(r"\frac{}{a}");
    assert_eq!(serialize(&frac), r"\frac { } { a }");
    assert_eq!(
        serialize_with(&frac, &minimal_command_spacing()),
        r"\frac{ } { a }"
    );

    // A padded empty group right after a control word keeps exactly one space.
    let mut padded = Ast::new();
    let command = padded.new_node(command_node("alpha"));
    let group = group_node(&mut padded, ContentMode::Math, &[]);
    padded.append_child(padded.root(), command);
    padded.append_child(padded.root(), group);
    assert_eq!(serialize(&padded), r"\alpha { }");
    assert_eq!(
        serialize_with(&padded, &minimal_command_spacing()),
        r"\alpha{ }"
    );
}

#[test]
fn empty_error_keeps_control_word_separation_for_the_next_atom() {
    // An error node with an empty snippet writes nothing, so it must not be
    // treated as the atom that closed the preceding control word.
    let letter = ast_with(
        ContentMode::Text,
        vec![command_node("alpha"), empty_error_node(), Node::Char('x')],
    );
    assert_eq!(serialize(&letter), r"\alpha x");

    let digit = ast_with(
        ContentMode::Text,
        vec![command_node("alpha"), empty_error_node(), Node::Char('1')],
    );
    assert_eq!(serialize(&digit), r"\alpha1");
}

#[test]
fn serialize_options_serde_fills_omitted_fields_from_defaults() {
    let options: SerializeOptions =
        serde_json::from_str(r#"{"script_order":"sup_first"}"#).unwrap();
    assert_eq!(options.script_order, ScriptOrder::SupFirst);
    assert_eq!(
        options,
        SerializeOptions {
            script_order: ScriptOrder::SupFirst,
            ..SerializeOptions::default()
        }
    );
}

#[test]
fn constructed_optional_content_protects_closing_tokens_in_all_spacing_styles() {
    for kind in [GroupKind::Explicit, GroupKind::Implicit] {
        let mut ast = Ast::new();
        let close = ast.new_node(Node::Char(']'));
        let group = ast.new_node(Node::Group {
            children: vec![close],
            mode: ContentMode::Math,
            kind,
        });
        let command = ast.new_node(Node::Command {
            name: "probe".into(),
            known: false,
            args: vec![Some(Argument::from_value(
                ArgumentKind::Optional,
                ArgumentValue::MathContent(group),
            ))],
        });
        ast.append_child(ast.root(), command);
        for spacing in [
            MathGroupInnerSpacing::Padded,
            MathGroupInnerSpacing::Compact,
        ] {
            let options = SerializeOptions {
                group_inner_spacing: spacing,
                ..SerializeOptions::default()
            };
            let result = serialize_tokenized_with(&ast, &options);
            assert_eq!(result.latex, serialize_with(&ast, &options));
            assert_token_contract(&result);
            assert!(result.latex.contains('{') && result.latex.contains('}'));
            let ctx = texform_core::parse::ParseContextBuilder::empty()
                .insert_item(support::command_item(
                    "probe",
                    CommandKind::Prefix,
                    AllowedMode::Math,
                    "o",
                ))
                .build()
                .unwrap();
            let reparsed = parse_to_ast_with_context(&ctx, &result.latex);
            assert_eq!(serialize_with(&reparsed, &options), result.latex);
        }
    }
}

#[test]
fn nested_optional_arguments_reuse_protection_decisions() {
    let mut source = "n".to_string();
    for _ in 0..8 {
        source = format!(r"\sqrt[{{{source}}}]{{x}}");
    }
    let ast = parse_to_ast(&source);
    let output = serialize(&ast);
    assert_eq!(serialize(&parse_to_ast(&output)), output);
    assert_eq!(serialize_tokenized(&ast).latex, output);
}
