mod support;

use support::parser::{command_item, test_context, test_context_with_items};
use texform_core::{
    ast::{Argument, ArgumentKind, ArgumentValue, Ast, ContentMode, GroupKind, Node},
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
fn tokenized_options_never_change_canonical_text() {
    let ast = parse_to_ast(r"\sqrt[3]{x_i}");
    let mut options = SerializeOptions::default();
    options.math.scripts.spacing = ScriptSpacing::Compact;
    options.math.spacing.group_inner_spacing = MathGroupInnerSpacing::Compact;
    options.math.spacing.commands = CommandSpacing::Minimal;

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
    let mut options = SerializeOptions::default();
    options.math.spacing.commands = CommandSpacing::Minimal;

    assert_eq!(serialize_with(&ast, &options), r"\sqrt{ a }");
}

#[test]
fn test_compact_math_group_inner_spacing_affects_command_wrapper_braces() {
    let ast = parse_to_ast(r"\sqrt{a}");
    let mut options = SerializeOptions::default();
    options.math.spacing.group_inner_spacing = MathGroupInnerSpacing::Compact;

    assert_eq!(serialize_with(&ast, &options), r"\sqrt {a}");
}

#[test]
fn test_serialize_with_compact_adjacent_char_spacing() {
    let ast = parse_to_ast("a+b");
    let mut options = SerializeOptions::default();
    options.math.spacing.adjacent_chars = AdjacentCharSpacing::Compact;

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
    let mut options = SerializeOptions::default();
    options.math.scripts.order = ScriptOrder::SupFirst;

    assert_eq!(serialize_with(&ast, &options), "f' _ { n }");
}

#[test]
fn test_serialize_mixed_prime_superscript_keeps_script_group() {
    assert_eq!(serialize(&parse_to_ast("f'^2")), "f ^ { ' 2 }");
}

#[test]
fn test_compact_math_group_inner_spacing_affects_script_wrapper_braces() {
    let ast = parse_to_ast("x^2_i");
    let mut options = SerializeOptions::default();
    options.math.spacing.group_inner_spacing = MathGroupInnerSpacing::Compact;

    assert_eq!(serialize_with(&ast, &options), "x _ {i} ^ {2}");
}

#[test]
fn test_serialize_with_sup_first_order() {
    let ast = parse_to_ast("x_i^2");
    let mut options = SerializeOptions::default();
    options.math.scripts.order = ScriptOrder::SupFirst;

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
    let mut options = SerializeOptions::default();
    options.math.spacing.group_inner_spacing = MathGroupInnerSpacing::Compact;

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
        r"\label {sec:intro} \romannumeral {12} \arraycols {lcr} \delim {\langle}"
    );
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
    let mut options = SerializeOptions::default();
    options.syntax.environments.name_spacing = EnvironmentNameSpacing::Compact;

    assert_eq!(
        serialize_with(&ast, &options),
        r"\begin{matrix} a b \end{matrix}"
    );
}

#[test]
fn test_environment_name_spacing_is_independent_from_command_spacing() {
    let ast = parse_to_ast(r"\begin {matrix}ab\end {matrix}");
    let mut options = SerializeOptions::default();
    options.math.spacing.commands = CommandSpacing::Minimal;
    options.syntax.environments.name_spacing = EnvironmentNameSpacing::Spaced;

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
    let mut options = SerializeOptions::default();
    options.math.infix.grouping = InfixGrouping::AlwaysExplicit;

    assert_eq!(serialize_with(&ast, &options), r"{ a } \over { b }");
}

#[test]
fn test_serialize_infix_when_required_keeps_nested_braces() {
    let ast = parse_to_ast(r"{a \over b} \over c");
    let mut options = SerializeOptions::default();
    options.math.infix.grouping = InfixGrouping::WhenRequired;

    assert_eq!(serialize_with(&ast, &options), r"{ a \over b } \over c");
}

#[test]
fn test_serialize_infix_when_required_keeps_flat_declarative_unbraced() {
    let ast = parse_to_ast(r"a \displaystyle b \over c");
    let mut options = SerializeOptions::default();
    options.math.infix.grouping = InfixGrouping::WhenRequired;

    assert_eq!(serialize_with(&ast, &options), r"a \displaystyle b \over c");
}

#[test]
fn test_serialize_infix_empty_left_operand_stays_unbraced() {
    let ast = parse_to_ast(r"\over x");
    let mut explicit = SerializeOptions::default();
    explicit.math.infix.grouping = InfixGrouping::AlwaysExplicit;

    assert_eq!(serialize(&ast), r"\over x");
    assert_eq!(serialize_with(&ast, &explicit), r"\over { x }");
}

#[test]
fn test_serialize_infix_empty_right_operand_stays_unbraced() {
    let ast = parse_to_ast(r"x \over");
    let mut explicit = SerializeOptions::default();
    explicit.math.infix.grouping = InfixGrouping::AlwaysExplicit;

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
    let mut options = SerializeOptions::default();
    options.math.scripts.spacing = ScriptSpacing::Compact;

    assert_eq!(serialize_with(&ast, &options), "x_{ i }^{ 2 }");
}

#[test]
fn test_serialize_minimal_command_spacing_compacts_left_right_delimiter() {
    let ast = parse_to_ast(r"\left (a+b\right )");
    let mut options = SerializeOptions::default();
    options.math.spacing.commands = CommandSpacing::Minimal;

    assert_eq!(serialize_with(&ast, &options), r"\left( a + b \right)");
}

#[test]
fn test_compact_math_group_inner_spacing_affects_optional_argument_brackets() {
    let ast = parse_to_ast(r"\sqrt[3]{x}");
    let mut options = SerializeOptions::default();
    options.math.spacing.group_inner_spacing = MathGroupInnerSpacing::Compact;

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
