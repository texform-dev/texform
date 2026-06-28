mod support;

use support::{
    assert_first_diagnostic_span_eq, collect_messages, command_item, contains_command_named,
    contains_error_node, parse_many_with_items, parse_single_with_items, parse_with_items,
};
use texform_core::parse::{
    AllowedMode, CommandKind, ContextItem, ParseContext, ParseContextBuilder, ParseResult,
};
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

fn text_command_item() -> ContextItem {
    command_item("text", CommandKind::Prefix, AllowedMode::Math, "m:T")
}

fn frac_command_item() -> ContextItem {
    command_item("frac", CommandKind::Prefix, AllowedMode::Math, "m m")
}

fn underline_math_item() -> ContextItem {
    command_item("underline", CommandKind::Prefix, AllowedMode::Math, "m")
}

fn underline_text_item() -> ContextItem {
    command_item("underline", CommandKind::Prefix, AllowedMode::Text, "m:T")
}

fn content_test_context() -> ParseContext {
    ParseContextBuilder::empty()
        .insert_item(text_command_item())
        .insert_item(frac_command_item())
        .insert_item(underline_math_item())
        .insert_item(underline_text_item())
        .build()
        .expect("content test context should build")
}

fn expect_arg(slot: &Option<Argument>) -> &Argument {
    slot.as_ref()
        .unwrap_or_else(|| panic!("expected argument slot to be present"))
}

fn first_command(output: &ParseResult) -> (String, Vec<Option<Argument>>) {
    let result = output
        .document()
        .unwrap_or_else(|| panic!("expected parse result"));
    match result.to_syntax() {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => (name.clone(), args.clone()),
            other => panic!("expected command node, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }
}

#[test]
fn test_no_leading_space_metadata_is_preserved_on_custom_optional() {
    let output = parse_with_items(
        &[command_item(
            "probe",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m !o",
        )],
        r"\probe a[b]",
        true,
    );
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let (name, args) = first_command(&output);
    assert_eq!(name, "probe");
    assert!(!expect_arg(&args[0]).no_leading_space);
    assert!(expect_arg(&args[1]).no_leading_space);
}

fn single_root_child(output: &ParseResult) -> SyntaxNode {
    let result = output
        .document()
        .unwrap_or_else(|| panic!("expected parse result"));
    match result.to_syntax() {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1, "expected a single root child");
            children[0].clone()
        }
        other => panic!("expected root node, got {:?}", other),
    }
}

fn expect_command_with_math_arg(
    node: &SyntaxNode,
    expected_name: &str,
    arg_index: usize,
) -> SyntaxNode {
    match node {
        SyntaxNode::Command { name, args, .. } => {
            assert_eq!(name, expected_name);
            match &expect_arg(&args[arg_index]).value {
                ArgumentValue::MathContent(value) => value.clone(),
                other => panic!("expected math content argument, got {:?}", other),
            }
        }
        other => panic!("expected command node, got {:?}", other),
    }
}

fn known_command(name: &str) -> SyntaxNode {
    SyntaxNode::Command {
        name: name.to_string(),
        args: Vec::new(),
        known: true,
    }
}

fn assert_prefix_shorthand_keeps_script_outside(
    src: &str,
    command_name: &str,
    expected_arg: SyntaxNode,
    expected_subscript: Option<SyntaxNode>,
    expected_superscript: Option<SyntaxNode>,
) {
    let output = ParseContext::shared().parse(src, &texform_core::parse::ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for {src}: {:?}",
        output.diagnostics
    );

    match single_root_child(&output) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, command_name, 0),
                expected_arg
            );
            assert_eq!(subscript.map(|node| *node), expected_subscript);
            assert_eq!(superscript.map(|node| *node), expected_superscript);
        }
        other => panic!("expected outer scripted command for {src}, got {:?}", other),
    }
}

#[test]
fn integer_argument_is_verified_by_parser() {
    let items = [command_item(
        "romannumeral",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:I",
    )];

    let valid = parse_single_with_items(&items, r"\romannumeral+42", true);
    assert!(
        valid.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        valid.diagnostics
    );
    let (name, args) = first_command(&valid);
    assert_eq!(name, "romannumeral");
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Integer("+42".to_string())
    );

    let invalid = parse_single_with_items(&items, r"\romannumeral+", true);
    assert!(invalid.document().is_none(), "invalid integer should fail");
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected integer diagnostics"
    );
}

#[test]
fn non_nullable_integer_argument_rejects_empty_group() {
    let items = [command_item(
        "romannumeral",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:I",
    )];

    let output = parse_single_with_items(&items, r"\romannumeral{}", true);
    assert!(output.document().is_none(), "empty integer should fail");
    assert!(
        !output.diagnostics.is_empty(),
        "expected integer diagnostics"
    );
}

#[test]
fn genfrac_accepts_empty_and_integer_style_arguments() {
    let outputs = parse_many_with_items(
        &[],
        &[
            r"\genfrac{}{}{0.0pt}{}{a}{b}",
            r"\genfrac{}{}{0.0pt}{1}{a}{b}",
        ],
        Some(&["base", "ams"]),
        &texform_core::parse::ParseConfig::STRICT,
    );
    assert_eq!(outputs.len(), 2);

    for item in outputs {
        assert!(
            item.output.diagnostics.is_empty(),
            "unexpected diagnostics for {}: {:?}",
            item.input,
            item.output.diagnostics
        );
        assert!(
            item.output.document().is_some(),
            "expected parse result for {}",
            item.input
        );
    }
}

#[test]
fn dimension_argument_is_verified_by_parser() {
    let items = [command_item(
        "hspace",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:L",
    )];

    let valid = parse_single_with_items(&items, r"\hspace{1,5 em}", true);
    assert!(
        valid.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        valid.diagnostics
    );
    let (name, args) = first_command(&valid);
    assert_eq!(name, "hspace");
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Dimension("1.5em".to_string())
    );

    let invalid = parse_single_with_items(&items, r"\hspaceabc", true);
    assert!(
        invalid.document().is_none(),
        "invalid dimension should fail"
    );
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected dimension diagnostics"
    );
}

#[test]
fn dimension_argument_accepts_shared_unit_set_and_rejects_unknown_units() {
    let items = [command_item(
        "hspace",
        CommandKind::Prefix,
        AllowedMode::Both,
        "m:L",
    )];

    for unit in ["em", "ex", "pt", "pc", "px", "in", "cm", "mm", "mu"] {
        let src = format!(r"\hspace{{1{unit}}}");
        let output = parse_single_with_items(&items, &src, true);
        assert!(
            output.diagnostics.is_empty(),
            "unexpected diagnostics for {src}: {:?}",
            output.diagnostics
        );
        let (_name, args) = first_command(&output);
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::Dimension(format!("1{unit}"))
        );
    }

    let invalid = parse_single_with_items(&items, r"\hspace{1zz}", true);
    assert!(invalid.document().is_none(), "unsupported unit should fail");
    assert!(
        !invalid.diagnostics.is_empty(),
        "expected diagnostics for unsupported unit, got {:?}",
        invalid.diagnostics
    );
}

#[test]
fn keyval_argument_accepts_nested_and_escaped_shapes() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let outputs = parse_many_with_items(
        &items,
        &[
            r"\includegraphics[key=val]{file}",
            r"\includegraphics[key={a,b},other=c]{file}",
            r"\includegraphics[key=\{,other=c]{file}",
        ],
        None,
        &texform_core::parse::ParseConfig::STRICT,
    );

    let expected = ["key=val", "key={a,b},other=c", r"key=\{,other=c"];

    for (item, expected_keyval) in outputs.iter().zip(expected) {
        assert!(
            item.output.diagnostics.is_empty(),
            "unexpected diagnostics for {}: {:?}",
            item.input,
            item.output.diagnostics
        );
        let (_, args) = first_command(&item.output);
        assert_eq!(
            expect_arg(&args[0]).value,
            ArgumentValue::KeyVal(expected_keyval.to_string())
        );
    }
}

#[test]
fn keyval_argument_rejects_invalid_shapes() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let outputs = parse_many_with_items(
        &items,
        &[
            r"\includegraphics[key=]{file}",
            r"\includegraphics[=value]{file}",
            r"\includegraphics[key={a]{file}",
        ],
        None,
        &texform_core::parse::ParseConfig::STRICT,
    );

    for item in &outputs {
        assert!(
            item.output.document().is_none(),
            "{} should fail",
            item.input
        );
        assert!(
            !item.output.diagnostics.is_empty(),
            "expected diagnostics for {}",
            item.input
        );
    }
}

#[test]
fn keyval_argument_diagnostic_span_covers_bracket_argument() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];
    let src = r"\includegraphics[key=]{file}";

    let output = parse_with_items(&items, src, true);
    assert!(output.document().is_none(), "invalid keyval should fail");

    let diagnostic = output
        .diagnostics
        .first()
        .expect("expected keyval diagnostic");
    assert_eq!(diagnostic.message, "keyval missing value");
    assert_eq!(&src[diagnostic.span.start..diagnostic.span.end], "[key=]");
}

#[test]
fn optional_bracket_content_stops_at_top_level_closer() {
    let items = [command_item(
        "includegraphics",
        CommandKind::Prefix,
        AllowedMode::Both,
        "o:K m:T",
    )];

    let output = parse_with_items(&items, r"\includegraphics[key={[[},width=1em]{file}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::KeyVal("key={[[},width=1em".to_string())
    );
}

#[test]
fn delimited_argument_collects_nested_content() {
    let items = [command_item(
        "reqdelim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "r{}",
    )];

    let output = parse_with_items(&items, r"\reqdelim{a{b}c}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    match expect_arg(&args[0]).kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("expected delimited argument kind, got {:?}", other),
    }
}

#[test]
fn mandatory_argument_normalizes_single_explicit_group() {
    let items = [command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )];

    let output = parse_with_items(&items, r"\probe{x}", true);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = first_command(&output);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
}

#[test]
fn non_braced_mandatory_math_argument_leaves_subscript_outside_prefix_command() {
    assert_prefix_shorthand_keeps_script_outside(
        r"\vec A_\mu",
        "vec",
        SyntaxNode::Char('A'),
        Some(known_command("mu")),
        None,
    );
}

#[test]
fn non_braced_mandatory_math_argument_leaves_superscript_outside_prefix_command() {
    assert_prefix_shorthand_keeps_script_outside(
        r"\bar C^\mu",
        "bar",
        SyntaxNode::Char('C'),
        None,
        Some(known_command("mu")),
    );

    assert_prefix_shorthand_keeps_script_outside(
        r"\widehat \lambda^k",
        "widehat",
        known_command("lambda"),
        None,
        Some(SyntaxNode::Char('k')),
    );
}

#[test]
fn non_braced_mandatory_math_argument_leaves_prime_outside_prefix_command() {
    let output =
        ParseContext::shared().parse(r"\vec A'", &texform_core::parse::ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for \\vec A': {:?}",
        output.diagnostics
    );

    match single_root_child(&output) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "vec", 0),
                SyntaxNode::Char('A')
            );
            assert!(subscript.is_none());
            assert_eq!(
                superscript.as_deref(),
                Some(&SyntaxNode::Prime { count: 1 }),
                "prime should stay outside \\vec"
            );
        }
        other => panic!("expected outer scripted command, got {:?}", other),
    }

    let output =
        ParseContext::shared().parse(r"\bar C'", &texform_core::parse::ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for \\bar C': {:?}",
        output.diagnostics
    );

    match single_root_child(&output) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "bar", 0),
                SyntaxNode::Char('C')
            );
            assert!(subscript.is_none());
            assert_eq!(
                superscript.as_deref(),
                Some(&SyntaxNode::Prime { count: 1 }),
                "prime should stay outside \\bar"
            );
        }
        other => panic!("expected outer scripted command, got {:?}", other),
    }
}

#[test]
fn non_braced_sqrt_and_frac_arguments_leave_following_scripts_outside() {
    let sqrt =
        ParseContext::shared().parse(r"\sqrt x^2", &texform_core::parse::ParseConfig::STRICT);
    assert!(
        sqrt.diagnostics.is_empty(),
        "unexpected diagnostics for \\sqrt x^2: {:?}",
        sqrt.diagnostics
    );

    match single_root_child(&sqrt) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "sqrt", 1),
                SyntaxNode::Char('x')
            );
            assert!(subscript.is_none());
            assert_eq!(superscript.map(|node| *node), Some(SyntaxNode::Char('2')));
        }
        other => panic!("expected outer scripted sqrt command, got {:?}", other),
    }

    let frac =
        ParseContext::shared().parse(r"\frac a b_c", &texform_core::parse::ParseConfig::STRICT);
    assert!(
        frac.diagnostics.is_empty(),
        "unexpected diagnostics for \\frac a b_c: {:?}",
        frac.diagnostics
    );

    match single_root_child(&frac) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "frac", 0),
                SyntaxNode::Char('a')
            );
            assert_eq!(
                expect_command_with_math_arg(&base, "frac", 1),
                SyntaxNode::Char('b')
            );
            assert_eq!(subscript.map(|node| *node), Some(SyntaxNode::Char('c')));
            assert!(superscript.is_none());
        }
        other => panic!("expected outer scripted frac command, got {:?}", other),
    }
}

#[test]
fn non_braced_mandatory_math_argument_accepts_leading_subscript_argument() {
    let output = ParseContext::shared().parse(
        r"\mod _ { 2 \pi }",
        &texform_core::parse::ParseConfig::STRICT,
    );
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for \\mod _ {{ 2 \\pi }}: {:?}",
        output.diagnostics
    );

    match single_root_child(&output) {
        SyntaxNode::Command { name, args, .. } => {
            assert_eq!(name, "mod");
            match &expect_arg(&args[0]).value {
                ArgumentValue::MathContent(SyntaxNode::Scripted {
                    base,
                    subscript,
                    superscript,
                }) => {
                    assert_eq!(
                        **base,
                        SyntaxNode::Group {
                            mode: ContentMode::Math,
                            kind: GroupKind::Implicit,
                            children: Vec::new(),
                        }
                    );
                    assert_eq!(
                        subscript.as_deref(),
                        Some(&SyntaxNode::Group {
                            mode: ContentMode::Math,
                            kind: GroupKind::Explicit,
                            children: vec![SyntaxNode::Char('2'), known_command("pi")],
                        })
                    );
                    assert!(superscript.is_none());
                }
                other => panic!("expected leading subscript math argument, got {:?}", other),
            }
        }
        other => panic!("expected command node, got {:?}", other),
    }
}

#[test]
fn non_braced_skew_argument_accepts_leading_subscript_argument() {
    let output = ParseContext::shared().parse(
        r"\skew 5 \hat { \bar { \psi } } _ { - }",
        &texform_core::parse::ParseConfig::STRICT,
    );
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for skew argument shorthand: {:?}",
        output.diagnostics
    );
}

#[test]
fn runtime_math_shorthand_argument_leaves_scripts_outside_custom_command() {
    let items = [command_item(
        "myvec",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m",
    )];

    let subscript = parse_with_items(&items, r"\myvec A_c", true);
    assert!(
        subscript.diagnostics.is_empty(),
        "unexpected diagnostics for custom shorthand subscript: {:?}",
        subscript.diagnostics
    );
    match single_root_child(&subscript) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "myvec", 0),
                SyntaxNode::Char('A')
            );
            assert_eq!(subscript.as_deref(), Some(&SyntaxNode::Char('c')));
            assert!(superscript.is_none());
        }
        other => panic!("expected outer scripted custom command, got {:?}", other),
    }

    let superscript = parse_with_items(&items, r"\myvec A^d", true);
    assert!(
        superscript.diagnostics.is_empty(),
        "unexpected diagnostics for custom shorthand superscript: {:?}",
        superscript.diagnostics
    );
    match single_root_child(&superscript) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "myvec", 0),
                SyntaxNode::Char('A')
            );
            assert!(subscript.is_none());
            assert_eq!(superscript.as_deref(), Some(&SyntaxNode::Char('d')));
        }
        other => panic!("expected outer scripted custom command, got {:?}", other),
    }

    let prime = parse_with_items(&items, r"\myvec A'", true);
    assert!(
        prime.diagnostics.is_empty(),
        "unexpected diagnostics for custom shorthand prime: {:?}",
        prime.diagnostics
    );
    match single_root_child(&prime) {
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(
                expect_command_with_math_arg(&base, "myvec", 0),
                SyntaxNode::Char('A')
            );
            assert!(subscript.is_none());
            assert_eq!(
                superscript.as_deref(),
                Some(&SyntaxNode::Prime { count: 1 })
            );
        }
        other => panic!("expected outer scripted custom command, got {:?}", other),
    }
}

#[test]
fn braced_mandatory_math_argument_keeps_scripts_inside_command_argument() {
    let output =
        ParseContext::shared().parse(r"\vec{A_\mu}", &texform_core::parse::ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics for \\vec{{A_\\mu}}: {:?}",
        output.diagnostics
    );

    match single_root_child(&output) {
        SyntaxNode::Command { name, args, .. } => {
            assert_eq!(name, "vec");
            match &expect_arg(&args[0]).value {
                ArgumentValue::MathContent(SyntaxNode::Scripted {
                    base,
                    subscript,
                    superscript,
                }) => {
                    assert_eq!(**base, SyntaxNode::Char('A'));
                    assert_eq!(subscript.as_deref(), Some(&known_command("mu")));
                    assert!(superscript.is_none());
                }
                other => panic!("expected scripted braced argument, got {:?}", other),
            }
        }
        other => panic!("expected command node, got {:?}", other),
    }
}

#[test]
fn star_argument_uses_boolean_value() {
    let items = [command_item(
        "sqrt",
        CommandKind::Prefix,
        AllowedMode::Math,
        "s m",
    )];

    let with_star = parse_with_items(&items, r"\sqrt*{x}", true);
    assert!(
        with_star.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        with_star.diagnostics
    );
    let (_, args) = first_command(&with_star);
    assert_eq!(
        expect_arg(&args[0]),
        &Argument {
            kind: ArgumentKind::Star,
            no_leading_space: false,
            value: ArgumentValue::Boolean(true),
        }
    );

    let without_star = parse_with_items(&items, r"\sqrt{x}", true);
    assert!(
        without_star.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        without_star.diagnostics
    );
    let (_, args) = first_command(&without_star);
    assert_eq!(
        expect_arg(&args[0]),
        &Argument {
            kind: ArgumentKind::Star,
            no_leading_space: false,
            value: ArgumentValue::Boolean(false),
        }
    );
}

#[test]
fn text_content_arguments_accept_whitespace_only_body() {
    let output =
        ParseContext::shared().parse(r"\textrm { }", &texform_core::parse::ParseConfig::LENIENT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    let result = output
        .document()
        .expect("whitespace-only text argument should produce a document");
    assert!(
        !result.has_errors(),
        "whitespace-only text argument should not create recovery errors"
    );
}

#[test]
fn text_content_arguments_keep_empty_body_valid() {
    let output =
        ParseContext::shared().parse(r"\textrm{}", &texform_core::parse::ParseConfig::LENIENT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    assert!(
        output.document().is_some(),
        "empty text argument should still parse as a complete document"
    );
}

#[test]
fn text_content_generic_only_error_keeps_expected_found_diagnostic() {
    let items = [text_command_item()];

    let output = parse_with_items(&items, r"\text{$x}", true);

    assert!(
        output.document().is_none(),
        "strict mode should not keep a partial result"
    );
    assert_eq!(
        collect_messages(&output),
        vec!["found '$' expected something else, or end of input"]
    );
    assert!(
        !output.diagnostics[0].expected.is_empty(),
        "expected/found details should stay available"
    );
    assert_eq!(
        output.diagnostics[0].expected,
        ["something else", "end of input"]
    );
}

#[test]
fn strict_text_content_command_error_points_to_inner_command() {
    let src = r"\text{\frac{a}{b}}";
    let items = [text_command_item(), frac_command_item()];

    let output = parse_with_items(&items, src, true);

    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
    assert_first_diagnostic_span_eq(&output, src, r"\frac");
}

#[test]
fn strict_text_content_command_error_has_no_partial_result() {
    let items = [text_command_item(), frac_command_item()];

    let output = parse_with_items(&items, r"\text{\frac{a}{b}}", true);

    assert!(
        output.document().is_none(),
        "strict content argument errors should not keep a partial result"
    );
    assert_eq!(
        collect_messages(&output),
        vec![r"Command \frac is not allowed in text mode"]
    );
}

#[test]
fn nonstrict_text_content_direct_error_survives_trailing_generic() {
    let ctx = content_test_context();
    let src = r"\text{\underline{a^2}$}";
    let output = ctx.parse(src, &texform_core::parse::ParseConfig::default());

    assert_eq!(
        collect_messages(&output),
        vec!["Scripted syntax is not allowed in Text mode"]
    );
    assert_first_diagnostic_span_eq(&output, src, "^");

    let result = output
        .document()
        .expect("non-strict direct error should keep a partial result");
    assert!(contains_command_named(&result.to_syntax(), "text"));
    assert!(contains_command_named(&result.to_syntax(), "underline"));
    assert!(contains_error_node(&result.to_syntax()));
}

mod migrated_argument_regressions {
    use super::support::parser::{
        expect_arg, label_command_item, parse, test_context_with_items, unwrap_content,
    };
    use texform_core::parse::ParseConfig;
    use texform_interface::syntax_node::{ArgumentKind, ArgumentValue, SyntaxNode};

    #[test]

    fn test_dimension_argument() {
        // "\hspace1em"

        let (result, _) = parse(r"\hspace1em", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "hspace");

                    assert_eq!(args.len(), 1);

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::Dimension("1em".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_integer_argument() {
        // "\romannumeral12"

        let (result, _) = parse(r"\romannumeral12", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "romannumeral");

                    assert_eq!(args.len(), 1);

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::Integer("12".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_keyval_argument() {
        // "\includegraphics[width=1em,height=2pt]{file}"

        let (result, _) = parse(r"\includegraphics[width=1em,height=2pt]{file}", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "includegraphics");

                    assert_eq!(args.len(), 2);

                    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Optional);

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::KeyVal("width=1em,height=2pt".to_string())
                    );

                    assert_eq!(
                        unwrap_content(&args[1]),
                        &SyntaxNode::Text("file".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_csname_argument() {
        let (result, _) = parse(r"\label{sec:intro}", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "label");

                    assert_eq!(args.len(), 1);

                    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Mandatory);

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::CSName("sec:intro".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_csname_argument_rejects_escape_sequence() {
        let ctx = test_context_with_items([label_command_item()]);

        let command = ctx.parse(r"\label{\alpha}", &ParseConfig::STRICT);

        assert!(
            command.document().is_none(),
            "control sequence inside CSName should fail"
        );

        assert!(
            !command.diagnostics.is_empty(),
            "expected CSName diagnostics, got {:?}",
            command.diagnostics
        );

        let escaped_symbol = ctx.parse(r"\label{sec\_a}", &ParseConfig::STRICT);

        assert!(
            escaped_symbol.document().is_none(),
            "escaped symbol inside CSName should fail"
        );

        assert!(
            !escaped_symbol.diagnostics.is_empty(),
            "expected CSName diagnostics, got {:?}",
            escaped_symbol.diagnostics
        );
    }

    #[test]

    fn test_delimiter_argument_braced_matches_inline() {
        let (inline, _) = parse(r"\delim\langle", false).unwrap();

        let (braced, _) = parse(r"\delim{\langle}", false).unwrap();

        let inline_value = match inline {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        let braced_value = match braced {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        assert_eq!(inline_value, braced_value);
    }

    #[test]

    fn test_integer_argument_braced_matches_inline() {
        let (inline, _) = parse(r"\romannumeral12", false).unwrap();

        let (braced, _) = parse(r"\romannumeral{ 12 }", false).unwrap();

        let inline_value = match inline {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        let braced_value = match braced {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        assert_eq!(inline_value, braced_value);
    }

    #[test]

    fn test_dimension_argument_braced_matches_inline() {
        let (inline, _) = parse(r"\hspace1.5em", false).unwrap();

        let (braced, _) = parse(r"\hspace{ 1,5 em }", false).unwrap();

        let inline_value = match inline {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        let braced_value = match braced {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => expect_arg(&args[0]).value.clone(),

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        };

        assert_eq!(inline_value, braced_value);

        assert_eq!(inline_value, ArgumentValue::Dimension("1.5em".to_string()));
    }

    #[test]

    fn test_optional_bracket_closes_at_top_level() {
        let (result, _) = parse(r"\includegraphics[key={[[},width=1em]{file}", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { args, .. } => {
                    assert_eq!(args.len(), 2);

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::KeyVal("key={[[},width=1em".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_dimension_with_spaces() {
        // "\hspace{1.5 cm}" - dimension with spaces between number and unit

        let (result, _) = parse(r"\hspace{1.5 cm}", false).unwrap();

        match result {
            SyntaxNode::Root { children, .. } => match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "hspace");

                    assert_eq!(args.len(), 1);

                    // Should be normalized to "1.5cm" (no space)

                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::Dimension("1.5cm".to_string())
                    );
                }

                _ => panic!("Expected Command node"),
            },

            _ => panic!("Expected root Group"),
        }
    }

    #[test]

    fn test_keyval_empty() {
        // "\includegraphics[{}]{file}" - empty braces in optional keyval argument

        // This should error because keyval requires at least one key=value pair

        let result = parse(r"\includegraphics[{}]{file}", false);

        assert!(result.is_err(), "Expected error for empty keyval");
    }

    #[test]

    fn test_keyval_empty_brackets() {
        // "\includegraphics[]{file}" - empty optional argument brackets

        // This should also error because the brackets exist but contain no valid keyval

        let result = parse(r"\includegraphics[]{file}", false);

        assert!(
            result.is_err(),
            "Expected error for empty optional keyval brackets"
        );
    }

    #[test]

    fn test_optional_bracket_missing_closer_errors() {
        let result = parse(r"\includegraphics[width=1em{file}", false);

        assert!(result.is_err());
    }

    #[test]

    fn test_invalid_integer_argument_errors() {
        let result = parse(r"\romannumeral{12.5}", false);

        assert!(result.is_err());
    }

    #[test]

    fn test_invalid_dimension_argument_errors() {
        let result = parse(r"\hspace{abc}", false);

        assert!(result.is_err());
    }

    #[test]

    fn test_unclosed_brace_argument_errors() {
        let result = parse(r"\frac{a", false);

        assert!(result.is_err());
    }

    #[test]

    fn test_dimension_missing_number() {
        // "\hspace{cm}" - dimension missing number should error

        let result = parse(r"\hspace{cm}", false);

        assert!(
            result.is_err(),
            "Expected error for dimension missing number"
        );
    }

    #[test]

    fn test_dimension_missing_unit() {
        // "\hspace{1.5}" - dimension missing unit should error

        let result = parse(r"\hspace{1.5}", false);

        assert!(result.is_err(), "Expected error for dimension missing unit");
    }
}
