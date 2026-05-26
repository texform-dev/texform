mod support;

use support::parser::*;
use texform_core::parse::{
    AllowedMode, CommandKind, ContextItem, DelimiterControlItem, ParseConfig,
};
use texform_interface::syntax_node::{
    ArgumentKind, ArgumentValue, ContentMode, Delimiter, GroupKind, SyntaxNode,
};

#[test]
fn test_qty_supports_multiple_delimiter_pairs() {
    let cases = [
        (r"\qty(x)", Delimiter::Char('('), Delimiter::Char(')')),
        (r"\qty[x]", Delimiter::Char('['), Delimiter::Char(']')),
        (r"\qty{x}", Delimiter::Char('{'), Delimiter::Char('}')),
        (r"\qty|x|", Delimiter::Char('|'), Delimiter::Char('|')),
    ];

    for (src, open, close) in cases {
        let (result, _) = parse(src, false).unwrap();
        let (name, args) = extract_first_command(result);
        assert_eq!(name, "qty");
        assert_eq!(args.len(), 1);

        let arg = expect_arg(&args[0]);
        assert_eq!(arg.value, ArgumentValue::MathContent(SyntaxNode::Char('x')));
        match arg.kind {
            ArgumentKind::Paired {
                open: matched_open,
                close: matched_close,
            } => {
                assert_eq!(matched_open, open);
                assert_eq!(matched_close, close);
            }
            other => panic!("Expected paired argument kind, got {:?}", other),
        }
    }
}

#[test]
fn test_qty_optional_slot_can_be_missing() {
    let (result, _) = parse(r"\qty", false).unwrap();
    let (name, args) = extract_first_command(result);
    assert_eq!(name, "qty");
    assert_eq!(args.len(), 1);
    assert!(args[0].is_none());
}

#[test]
fn test_arg_true_quantity_commands_require_braces() {
    let (pqty_ok, _) = parse(r"\pqty{x}", false).unwrap();
    let (name, args) = extract_first_command(pqty_ok);
    assert_eq!(name, "pqty");
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));

    let content = expect_arg(&args[1]);
    assert_eq!(
        content.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match content.kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected brace-delimited argument, got {:?}", other),
    }

    let (abs_ok, _) = parse(r"\abs{x}", false).unwrap();
    let (name, args) = extract_first_command(abs_ok);
    assert_eq!(name, "abs");
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );

    assert!(parse(r"\pqty(x)", false).is_err());
    assert!(parse(r"\abs|x|", false).is_err());
}

#[test]
fn test_eval_uses_nonsymmetric_paired_delimiter() {
    let (result, _) = parse(r"\eval(x|", false).unwrap();
    let (name, args) = extract_first_command(result);
    assert_eq!(name, "eval");
    assert_eq!(args.len(), 2);

    let star = expect_arg(&args[0]);
    assert_eq!(star.kind, ArgumentKind::Star);
    assert_eq!(star.value, ArgumentValue::Boolean(false));

    let paired = expect_arg(&args[1]);
    assert_eq!(
        paired.value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match paired.kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('('));
            assert_eq!(close, Delimiter::Char('|'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }
}

#[test]
fn test_dv_and_pdv_group_slots_are_stable() {
    let (dv_result, _) = parse(r"\dv{f}", false).unwrap();
    let (name, args) = extract_first_command(dv_result);
    assert_eq!(name, "dv");
    assert_eq!(args.len(), 4);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Boolean(false),
        "star slot should always exist",
    );
    assert!(args[1].is_none(), "optional bracket slot should be None");
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('f'))
    );
    assert!(args[3].is_none(), "group slot should be None when absent");

    let (pdv_result, _) = parse(r"\pdv{f}{x}{y}", false).unwrap();
    let (name, args) = extract_first_command(pdv_result);
    assert_eq!(name, "pdv");
    assert_eq!(args.len(), 5);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
    assert!(args[1].is_none());
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('f'))
    );
    assert_eq!(expect_arg(&args[3]).kind, ArgumentKind::Group);
    assert_eq!(expect_arg(&args[4]).kind, ArgumentKind::Group);
}

#[test]
fn test_braket_optional_group_slot() {
    let (result_full, _) = parse(r"\braket{a}{b}", false).unwrap();
    let (_, args_full) = extract_first_command(result_full);
    assert_eq!(args_full.len(), 3);
    assert_eq!(
        expect_arg(&args_full[0]).value,
        ArgumentValue::Boolean(false)
    );
    assert_eq!(
        expect_arg(&args_full[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('a'))
    );
    assert_eq!(expect_arg(&args_full[2]).kind, ArgumentKind::Group);

    let (result_short, _) = parse(r"\braket{a}", false).unwrap();
    let (_, args_short) = extract_first_command(result_short);
    assert_eq!(args_short.len(), 3);
    assert!(args_short[2].is_none());
}

#[test]
fn test_exp_does_not_consume_star_without_s_slot() {
    let (result, _) = parse(r"\exp*", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "exp");
                    assert!(args.is_empty());
                }
                other => panic!("Expected exp command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('*'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_exp_does_not_consume_brackets_without_optional_slot() {
    let (result, _) = parse(r"\exp[x]", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "exp");
                    assert!(args.is_empty());
                }
                other => panic!("Expected exp command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        _ => panic!("Expected root Group"),
    }
}

#[test]
fn test_bare_brackets_parse_as_regular_characters() {
    let (result, _) = parse("[a]", false).unwrap();
    match result {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 3);
            assert_eq!(children[0], SyntaxNode::Char('['));
            assert_eq!(children[1], SyntaxNode::Char('a'));
            assert_eq!(children[2], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_no_leading_space_after_single_token_m_for_optional_brackets() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m !o",
    )]);

    let spaced = ctx.parse(r"\probe a [b]", &ParseConfig::STRICT);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match spaced_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 2);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('a'))
                    );
                    assert!(args[1].is_none(), "spaced !o slot should not match");
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('b'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    let tight = ctx.parse(r"\probe a[b]", &ParseConfig::STRICT);
    assert!(
        tight.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        tight.diagnostics
    );
    let tight_node = tight
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match tight_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 2);
                    assert_eq!(
                        expect_arg(&args[0]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('a'))
                    );
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('b'))
                    );
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_no_leading_space_after_single_token_m_for_group_slot() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "s o m !g",
    )]);

    let spaced = ctx.parse(r"\probe*[n]f {x}", &ParseConfig::STRICT);
    assert!(
        spaced.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        spaced.diagnostics
    );
    let spaced_node = spaced
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match spaced_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 2);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 4);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('n'))
                    );
                    assert_eq!(
                        expect_arg(&args[2]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('f'))
                    );
                    assert!(args[3].is_none(), "spaced !g slot should not match");
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
            match &children[1] {
                SyntaxNode::Group {
                    kind: GroupKind::Explicit,
                    children: group_children,
                    ..
                } => {
                    assert_eq!(group_children.len(), 1);
                    assert_eq!(group_children[0], SyntaxNode::Char('x'));
                }
                other => panic!("Expected trailing explicit group, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    let tight = ctx.parse(r"\probe*[n]f{x}", &ParseConfig::STRICT);
    assert!(
        tight.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        tight.diagnostics
    );
    let tight_node = tight
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();

    match tight_node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 1);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "probe");
                    assert_eq!(args.len(), 4);
                    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                    assert_eq!(
                        expect_arg(&args[1]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('n'))
                    );
                    assert_eq!(
                        expect_arg(&args[2]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('f'))
                    );
                    assert_eq!(expect_arg(&args[3]).kind, ArgumentKind::Group);
                    assert_eq!(
                        expect_arg(&args[3]).value,
                        ArgumentValue::MathContent(SyntaxNode::Char('x'))
                    );
                }
                other => panic!("Expected probe command, got {:?}", other),
            }
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_required_group_form_enforces_braces() {
    let ctx = test_context_with_items([command_item(
        "reqgrp",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m{}",
    )]);

    let present = ctx.parse(r"\reqgrp{x}", &ParseConfig::STRICT);
    assert!(
        present.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        present.diagnostics
    );
    let present_node = present
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match present_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "reqgrp");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::MathContent(SyntaxNode::Char('x'))
                );
            }
            other => panic!("Expected reqgrp command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let missing = ctx.parse(r"\reqgrp x", &ParseConfig::STRICT);
    assert!(
        missing.result.is_none(),
        "missing required group should fail"
    );
    assert!(
        !missing.diagnostics.is_empty(),
        "missing required group should report diagnostics, got {:?}",
        missing.diagnostics
    );

    let wrong_form = ctx.parse(r"\reqgrp|x|", &ParseConfig::STRICT);
    assert!(
        wrong_form.result.is_none(),
        "non-braced required group should fail"
    );
    assert!(
        !wrong_form.diagnostics.is_empty(),
        "non-braced required group should report diagnostics, got {:?}",
        wrong_form.diagnostics
    );
}

#[test]
fn test_group_form_supports_dimension_kind() {
    let ctx = test_context_with_items([command_item(
        "gdim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "g:L",
    )]);

    let missing = ctx.parse(r"\gdim", &ParseConfig::STRICT);
    assert!(
        missing.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        missing.diagnostics
    );
    let missing_node = missing
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match missing_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "gdim");
                assert_eq!(args.len(), 1);
                assert!(args[0].is_none(), "group slot should be None when absent");
            }
            other => panic!("Expected gdim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let present = ctx.parse(r"\gdim{1.5em}", &ParseConfig::STRICT);
    assert!(
        present.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        present.diagnostics
    );
    let present_node = present
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match present_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "gdim");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Dimension("1.5em".to_string())
                );
            }
            other => panic!("Expected gdim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_required_group_form_composes_with_star_and_standard_slots() {
    let ctx = test_context_with_items([command_item(
        "probe",
        CommandKind::Prefix,
        AllowedMode::Math,
        "s m{} m",
    )]);

    let basic = ctx.parse(r"\probe{A}B", &ParseConfig::STRICT);
    assert!(
        basic.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        basic.diagnostics
    );
    let (name, args) = extract_first_command(
        basic
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(name, "probe");
    assert_eq!(args.len(), 3);
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
    assert_eq!(expect_arg(&args[1]).kind, ArgumentKind::Group);
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('A'))
    );
    assert_eq!(expect_arg(&args[2]).kind, ArgumentKind::Mandatory);
    assert_eq!(
        expect_arg(&args[2]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('B'))
    );

    let starred = ctx.parse(r"\probe*{A}B", &ParseConfig::STRICT);
    assert!(
        starred.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        starred.diagnostics
    );
    let (_, starred_args) = extract_first_command(
        starred
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&starred_args[0]).value,
        ArgumentValue::Boolean(true)
    );
    assert_eq!(expect_arg(&starred_args[1]).kind, ArgumentKind::Group);
    assert_eq!(expect_arg(&starred_args[2]).kind, ArgumentKind::Mandatory);

    let missing = ctx.parse(r"\probe B", &ParseConfig::STRICT);
    assert!(
        missing.result.is_none(),
        "missing required group slot should fail"
    );
    assert!(
        !missing.diagnostics.is_empty(),
        "missing required group slot should report diagnostics, got {:?}",
        missing.diagnostics
    );
}

#[test]
fn test_group_form_supports_delimiter_kind() {
    let ctx = test_context_with_items([command_item(
        "gdelim",
        CommandKind::Prefix,
        AllowedMode::Math,
        "g:D",
    )]);

    let output = ctx.parse(r"\gdelim{|}", &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let node = output
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { name, args, .. } => {
                assert_eq!(name, "gdelim");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
                assert_eq!(
                    expect_arg(&args[0]).value,
                    ArgumentValue::Delimiter(Delimiter::Char('|'))
                );
            }
            other => panic!("Expected gdelim command, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_nullable_delimiter_argument_accepts_empty_required_group() {
    let ctx = test_context_with_items(vec![
        ContextItem::from(command_item(
            "ndelim",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D?",
        )),
        ContextItem::from(command_item(
            "strictdelim",
            CommandKind::Prefix,
            AllowedMode::Math,
            "m:D",
        )),
        ContextItem::from(DelimiterControlItem::new("langle")),
    ]);

    let empty = ctx.parse(r"\ndelim{}", &ParseConfig::STRICT);
    assert!(
        empty.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        empty.diagnostics
    );
    let (_, empty_args) = extract_first_command(
        empty
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&empty_args[0]).value,
        ArgumentValue::Delimiter(Delimiter::None)
    );

    let explicit = ctx.parse(r"\ndelim\langle", &ParseConfig::STRICT);
    assert!(
        explicit.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        explicit.diagnostics
    );
    let (_, explicit_args) = extract_first_command(
        explicit
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(
        expect_arg(&explicit_args[0]).value,
        ArgumentValue::Delimiter(Delimiter::Control("langle"))
    );

    let strict_empty = ctx.parse(r"\strictdelim{}", &ParseConfig::STRICT);
    assert!(
        strict_empty.result.is_none(),
        "non-nullable delimiter should reject empty braces"
    );
    assert!(
        !strict_empty.diagnostics.is_empty(),
        "non-nullable delimiter should report diagnostics"
    );
}

#[test]
fn test_nullable_delimiter_group_accepts_empty_group() {
    let ctx = test_context_with_items([command_item(
        "gdelimnull",
        CommandKind::Prefix,
        AllowedMode::Math,
        "m{}:D?",
    )]);

    let output = ctx.parse(r"\gdelimnull{}", &ParseConfig::STRICT);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let (_, args) = extract_first_command(
        output
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Group);
    assert_eq!(
        expect_arg(&args[0]).value,
        ArgumentValue::Delimiter(Delimiter::None)
    );
}

#[test]
fn test_required_group_and_delimited_forms_have_distinct_ast_kinds() {
    let ctx = test_context_with_items([
        command_item("reqgrp", CommandKind::Prefix, AllowedMode::Math, "m{}"),
        command_item("reqdelim", CommandKind::Prefix, AllowedMode::Math, "r{}"),
    ]);

    let group = ctx.parse(r"\reqgrp{x}", &ParseConfig::STRICT);
    assert!(
        group.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        group.diagnostics
    );
    let (_, group_args) = extract_first_command(
        group
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    assert_eq!(expect_arg(&group_args[0]).kind, ArgumentKind::Group);

    let delimited = ctx.parse(r"\reqdelim{x}", &ParseConfig::STRICT);
    assert!(
        delimited.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        delimited.diagnostics
    );
    let (_, delimited_args) = extract_first_command(
        delimited
            .result
            .as_ref()
            .unwrap_or_else(|| panic!("expected parse result"))
            .node
            .clone(),
    );
    match expect_arg(&delimited_args[0]).kind {
        ArgumentKind::Delimited { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected delimited argument kind, got {:?}", other),
    }
}

#[test]
fn test_mqty_supports_star_plus_optional_paired_slot() {
    let (starred, _) = parse(r"\mqty*|x|", false).unwrap();
    let (name, args) = extract_first_command(starred);
    assert_eq!(name, "mqty");
    assert_eq!(args.len(), 2);
    assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
    assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match expect_arg(&args[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('|'));
            assert_eq!(close, Delimiter::Char('|'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (missing, _) = parse(r"\mqty*", false).unwrap();
    let (_, missing_args) = extract_first_command(missing);
    assert_eq!(missing_args.len(), 2);
    assert!(missing_args[1].is_none(), "paired slot should be optional");
}

#[test]
fn test_dd_supports_optional_then_paired_slots() {
    let (basic, _) = parse(r"\dd{x}", false).unwrap();
    let (name, args) = extract_first_command(basic);
    assert_eq!(name, "dd");
    assert_eq!(args.len(), 2);
    assert!(args[0].is_none(), "optional bracket slot should be None");
    assert_eq!(
        expect_arg(&args[1]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('x'))
    );
    match expect_arg(&args[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('{'));
            assert_eq!(close, Delimiter::Char('}'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (with_opt, _) = parse(r"\dd[y](x)", false).unwrap();
    let (_, args_with_opt) = extract_first_command(with_opt);
    assert_eq!(
        expect_arg(&args_with_opt[0]).value,
        ArgumentValue::MathContent(SyntaxNode::Char('y'))
    );
    match expect_arg(&args_with_opt[1]).kind {
        ArgumentKind::Paired { open, close } => {
            assert_eq!(open, Delimiter::Char('('));
            assert_eq!(close, Delimiter::Char(')'));
        }
        other => panic!("Expected paired argument kind, got {:?}", other),
    }

    let (unmatched, _) = parse(r"\dd[y]|x|", false).unwrap();
    match unmatched {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "dd");
                    assert_eq!(args.len(), 2);
                    assert!(
                        args[1].is_none(),
                        "non-candidate delimiter should not be consumed"
                    );
                }
                other => panic!("Expected dd command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('|'));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char('|'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_paired_form_required_vs_optional_semantics() {
    let ctx = test_context_with_items([
        command_item("mustpair", CommandKind::Prefix, AllowedMode::Math, "r<(,)>"),
        command_item(
            "maybepair",
            CommandKind::Prefix,
            AllowedMode::Math,
            "d<(,)>",
        ),
    ]);

    let required_ok = ctx.parse(r"\mustpair(x)", &ParseConfig::STRICT);
    assert!(
        required_ok.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        required_ok.diagnostics
    );
    assert!(
        required_ok.result.is_some(),
        "required paired arg should parse"
    );

    let required_missing = ctx.parse(r"\mustpair", &ParseConfig::STRICT);
    assert!(
        required_missing.result.is_none(),
        "missing required paired arg should fail"
    );
    assert!(
        !required_missing.diagnostics.is_empty(),
        "missing required paired arg should report diagnostics, got {:?}",
        required_missing.diagnostics
    );

    let optional_unmatched = ctx.parse(r"\maybepair[x]", &ParseConfig::STRICT);
    assert!(
        optional_unmatched.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        optional_unmatched.diagnostics
    );
    let node = optional_unmatched
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match node {
        SyntaxNode::Root { children, .. } => {
            assert_eq!(children.len(), 4);
            match &children[0] {
                SyntaxNode::Command { name, args, .. } => {
                    assert_eq!(name, "maybepair");
                    assert_eq!(args.len(), 1);
                    assert!(args[0].is_none(), "optional paired slot should stay empty");
                }
                other => panic!("Expected maybepair command, got {:?}", other),
            }
            assert_eq!(children[1], SyntaxNode::Char('['));
            assert_eq!(children[2], SyntaxNode::Char('x'));
            assert_eq!(children[3], SyntaxNode::Char(']'));
        }
        other => panic!("Expected root node, got {:?}", other),
    }
}

#[test]
fn test_environment_star_in_name_is_independent_from_s_arg_slot() {
    let ctx = test_context_with_items([
        environment_item("probenv", AllowedMode::Math, ContentMode::Math, "s"),
        environment_item("probenv*", AllowedMode::Math, ContentMode::Math, "s"),
    ]);

    let starred_name = ctx.parse(r"\begin{probenv*}x\end{probenv*}", &ParseConfig::STRICT);
    assert!(
        starred_name.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        starred_name.diagnostics
    );
    let starred_name_node = starred_name
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    assert!(
        matches!(starred_name_node, SyntaxNode::Root { .. }),
        "expected root node"
    );
    match starred_name_node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment { name, args, .. } => {
                assert_eq!(name, "probenv*");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(false));
            }
            other => panic!("Expected environment node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }

    let star_arg = ctx.parse(r"\begin{probenv}*x\end{probenv}", &ParseConfig::STRICT);
    assert!(
        star_arg.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        star_arg.diagnostics
    );
    let node = star_arg
        .result
        .as_ref()
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
        .clone();
    match node {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Environment {
                name, args, body, ..
            } => {
                assert_eq!(name, "probenv");
                assert_eq!(args.len(), 1);
                assert_eq!(expect_arg(&args[0]).kind, ArgumentKind::Star);
                assert_eq!(expect_arg(&args[0]).value, ArgumentValue::Boolean(true));
                match &**body {
                    SyntaxNode::Group { children, .. } => {
                        assert_eq!(children.len(), 1);
                        assert_eq!(children[0], SyntaxNode::Char('x'));
                    }
                    other => panic!("Expected environment body group, got {:?}", other),
                }
            }
            other => panic!("Expected environment node, got {:?}", other),
        },
        other => panic!("Expected root node, got {:?}", other),
    }
}
