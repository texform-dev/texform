use texform_core::ast::{ArgumentValue, Ast, GroupKind, Node, Slot};
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::serialize;
use texform_transform::{
    FlattenGroupsConfig, LowerAttributesConfig, RewriteConfig, TransformConfig, run as transform,
};

struct Outcome {
    ast: Ast,
    report: texform_transform::FlattenGroupsReport,
    text: String,
}

fn run_flatten_groups(src: &str) -> Outcome {
    run_flatten_groups_with_config(src, FlattenGroupsConfig::ENABLED)
}

fn run_flatten_groups_with_config(src: &str, flatten_groups: FlattenGroupsConfig) -> Outcome {
    let parse_ctx = ParseContext::from_packages(&["base", "ams"]);
    let mut ast = parse_ctx
        .parse_to_ast(src, &ParseConfig::default())
        .expect("source should parse");
    let config = TransformConfig {
        lower_attributes: LowerAttributesConfig::DISABLED,
        rewrite: RewriteConfig::DISABLED,
        flatten_groups,
    };
    let report = transform(&mut ast, &parse_ctx, &config)
        .expect("transform should succeed")
        .flatten_groups;
    ast.assert_invariants();
    let text = serialize(&ast);

    Outcome { ast, report, text }
}

#[test]
fn simplifies_nonempty_group_child_slots() {
    let outcome = run_flatten_groups(r"a{}{b}{cd}");

    assert_eq!(outcome.text, "a { } b c d");
    assert_eq!(outcome.report.removed_empty, 0);
    assert_eq!(outcome.report.replaced_single_child, 1);
    assert_eq!(outcome.report.inlined_multi_child, 1);
    assert_eq!(outcome.report.unwrapped_slot, 0);
}

#[test]
fn redirects_single_child_argument_and_script_slots() {
    let outcome = run_flatten_groups(r"\frac{{a}}{b} x^{y}");

    assert_eq!(outcome.text, r"\frac { a } { b } x ^ { y }");
    assert_eq!(outcome.report.unwrapped_slot, 2);

    let frac = outcome
        .ast
        .children(outcome.ast.root())
        .iter()
        .copied()
        .find(|&child| matches!(outcome.ast.node(child), Node::Command { name, .. } if name == "frac"))
        .expect("frac command should exist");
    let Node::Command { args, .. } = outcome.ast.node(frac) else {
        panic!("frac should be a command");
    };
    let ArgumentValue::MathContent(numerator) =
        args[0].as_ref().expect("numerator should exist").value
    else {
        panic!("numerator should be math content");
    };
    assert!(matches!(outcome.ast.node(numerator), Node::Char('a')));
    assert_eq!(outcome.ast.slot(numerator), Some(Slot::Argument(0)));
}

#[test]
fn keeps_script_base_groups() {
    let outcome = run_flatten_groups(r"{x_i}^2 + {x}^2");

    assert_eq!(outcome.text, r"{ x _ { i } } ^ { 2 } + x ^ { 2 }");
    assert_eq!(outcome.report.unwrapped_slot, 1);
    assert_eq!(outcome.report.preserved_group_in_script_base_slot, 1);
    assert_eq!(outcome.report.replaced_single_child, 0);
}

#[test]
fn redirects_single_child_infix_operands() {
    let outcome = run_flatten_groups(r"{a} \over {b}");

    assert_eq!(outcome.text, r"a \over b");
    assert_eq!(outcome.report.unwrapped_slot, 2);

    let [infix] = outcome.ast.children(outcome.ast.root()) else {
        panic!("root should contain only the infix node");
    };
    let Node::Infix { left, right, .. } = outcome.ast.node(*infix) else {
        panic!("root child should be an infix node");
    };
    assert!(matches!(outcome.ast.node(*left), Node::Char('a')));
    assert!(matches!(outcome.ast.node(*right), Node::Char('b')));
    assert_eq!(outcome.ast.slot(*left), Some(Slot::InfixLeft));
    assert_eq!(outcome.ast.slot(*right), Some(Slot::InfixRight));
}

#[test]
fn keeps_group_child_groups_that_scope_infix() {
    let outcome = run_flatten_groups(r"{a \over b}, c");

    assert_eq!(outcome.text, r"{ a \over b } , c");
    assert_eq!(outcome.report.replaced_single_child, 0);
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn keeps_group_child_groups_adjacent_to_commands() {
    let outcome = run_flatten_groups(r"\cos{A} + {\not\! p} + {\int}");

    assert_eq!(outcome.text, r"\cos { A } + { \not \! p } + { \int }");
    assert_eq!(outcome.report.replaced_single_child, 0);
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn keeps_group_child_groups_adjacent_to_scripted_commands() {
    let outcome = run_flatten_groups(r"\sum_i{(x_i)} + {\lim_{n} x_n}");

    assert_eq!(
        outcome.text,
        r"\sum _ { i } { ( x _ { i } ) } + { \lim _ { n } x _ { n } }"
    );
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn keeps_empty_and_operator_singleton_group_children() {
    let outcome = run_flatten_groups(r"{} + {>} + {a} + {-} n");

    assert_eq!(outcome.text, r"{ } + { > } + a + { - } n");
    assert_eq!(outcome.report.removed_empty, 0);
    assert_eq!(outcome.report.replaced_single_child, 1);
}

#[test]
fn keeps_operator_prefixed_group_children() {
    let outcome = run_flatten_groups(r"f{-n} + exp{-\alpha x}");

    assert_eq!(outcome.text, r"f { - n } + e x p { - \alpha x }");
    assert_eq!(outcome.report.replaced_single_child, 0);
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn keeps_multi_child_single_value_slots_and_environment_body_groups() {
    let outcome = run_flatten_groups(r"\frac{a+b}{c}\begin{matrix}{x}\end{matrix}");

    assert_eq!(
        outcome.text,
        r"\frac { a + b } { c } \begin {matrix} { x } \end {matrix}"
    );
    assert_eq!(outcome.report.unwrapped_slot, 0);
    assert_eq!(outcome.report.replaced_single_child, 0);

    let frac = outcome
        .ast
        .children(outcome.ast.root())
        .iter()
        .copied()
        .find(|&child| matches!(outcome.ast.node(child), Node::Command { name, .. } if name == "frac"))
        .expect("frac command should exist");
    let Node::Command { args, .. } = outcome.ast.node(frac) else {
        panic!("frac should be a command");
    };
    let ArgumentValue::MathContent(numerator) =
        args[0].as_ref().expect("numerator should exist").value
    else {
        panic!("numerator should be math content");
    };
    assert!(matches!(
        outcome.ast.node(numerator),
        Node::Group { children, .. } if children.len() > 1
    ));
    assert_eq!(outcome.ast.slot(numerator), Some(Slot::Argument(0)));

    let env = outcome
        .ast
        .children(outcome.ast.root())
        .iter()
        .copied()
        .find(|&child| matches!(outcome.ast.node(child), Node::Environment { name, .. } if name == "matrix"))
        .expect("matrix environment should exist");
    let Node::Environment { body, .. } = outcome.ast.node(env) else {
        panic!("matrix should be an environment");
    };
    assert!(matches!(
        outcome.ast.node(*body),
        Node::Group {
            kind: GroupKind::Implicit,
            ..
        }
    ));
    assert_eq!(outcome.ast.slot(*body), Some(Slot::EnvBody));
}

#[test]
fn keeps_groups_inside_environment_bodies() {
    let outcome = run_flatten_groups(r"\begin{array}{r l}{F}&{{}\approx k}\end{array}");

    assert_eq!(
        outcome.text,
        r"\begin {array} {r l} { F } & { { } \approx k } \end {array}"
    );
    assert_eq!(outcome.report.removed_empty, 0);
    assert_eq!(outcome.report.replaced_single_child, 0);
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn keeps_delimited_groups() {
    let outcome = run_flatten_groups(r"\left(a\right)");

    assert_eq!(outcome.text, r"\left ( a \right )");
    assert_eq!(outcome.report.removed_empty, 0);
    assert_eq!(outcome.report.replaced_single_child, 0);
    assert_eq!(outcome.report.inlined_multi_child, 0);
    assert_eq!(outcome.report.unwrapped_slot, 0);

    let [delimited] = outcome.ast.children(outcome.ast.root()) else {
        panic!("root should contain only the delimited group");
    };
    assert!(matches!(
        outcome.ast.node(*delimited),
        Node::Group {
            kind: GroupKind::Delimited { .. },
            ..
        }
    ));
}

#[test]
fn keeps_group_child_groups_that_wrap_delimited_groups() {
    let outcome = run_flatten_groups(r"f{\left(x\right)} + {a}");

    assert_eq!(outcome.text, r"f { \left ( x \right ) } + a");
    assert_eq!(outcome.report.replaced_single_child, 1);
}

#[test]
fn keeps_groups_that_scope_declarative_commands() {
    let outcome = run_flatten_groups(r"{\cal M} + {\bf f}(x) + {a}");

    assert_eq!(outcome.text, r"{ \cal M } + { \bf f } ( x ) + a");
    assert_eq!(outcome.report.replaced_single_child, 1);
    assert_eq!(outcome.report.inlined_multi_child, 0);
}

#[test]
fn reports_actual_preserve_guard_blockers() {
    let outcome = run_flatten_groups(
        r"{\cal M} + {x_i}^2 + {a \over b} + \cos{A} + {} + {+} + {-n} + f{\left(x\right)}",
    );

    assert_eq!(
        outcome
            .report
            .preserved_group_containing_declarative_command,
        1
    );
    assert_eq!(outcome.report.preserved_group_in_script_base_slot, 1);
    assert_eq!(outcome.report.preserved_group_containing_infix, 1);
    assert_eq!(outcome.report.preserved_group_adjacent_to_command_like, 1);
    assert_eq!(outcome.report.preserved_empty_group, 1);
    assert_eq!(
        outcome.report.preserved_group_with_lone_atom_spacing_char,
        1
    );
    assert_eq!(
        outcome
            .report
            .preserved_group_starting_with_atom_spacing_char,
        1
    );
    assert_eq!(outcome.report.preserved_group_containing_delimited_pair, 1);
}

#[test]
fn reports_scripted_command_like_subflag_hits() {
    let outcome = run_flatten_groups(r"\sum_i{(x_i)}");

    assert_eq!(outcome.report.preserved_group_adjacent_to_command_like, 1);
    assert_eq!(
        outcome.report.preserved_group_after_scripted_command_like,
        1
    );
}

#[test]
fn turning_off_spacing_guards_flattens_spacing_only_cases() {
    let outcome = run_flatten_groups_with_config(
        r"\cos{A} + {} + {+} + {-n} + f{\left(x\right)}",
        FlattenGroupsConfig::STRUCTURAL_ONLY,
    );

    assert_eq!(outcome.text, r"\cos A + + + + - n + f \left ( x \right )");
    assert_eq!(outcome.report.removed_empty, 1);
    assert_eq!(outcome.report.replaced_single_child, 3);
    assert_eq!(outcome.report.inlined_multi_child, 1);
}

#[test]
fn structural_only_still_keeps_semantic_guard_cases() {
    let outcome = run_flatten_groups_with_config(
        r"{\cal M} + {x_i}^2 + {a \over b} + \begin{matrix}{x}\end{matrix}",
        FlattenGroupsConfig::STRUCTURAL_ONLY,
    );

    assert_eq!(
        outcome.text,
        r"{ \cal M } + { x _ { i } } ^ { 2 } + { a \over b } + \begin {matrix} { x } \end {matrix}"
    );
    assert_eq!(
        outcome
            .report
            .preserved_group_containing_declarative_command,
        1
    );
    assert_eq!(outcome.report.preserved_group_in_script_base_slot, 1);
    assert_eq!(outcome.report.preserved_group_containing_infix, 1);
    assert_eq!(outcome.report.preserved_group_inside_env_body, 2);
}

#[test]
fn individual_guard_toggles_affect_only_their_cases() {
    let mut cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_adjacent_to_command_like = false;
    let outcome = run_flatten_groups_with_config(r"\cos{A} + {a}", cfg);
    assert_eq!(outcome.text, r"\cos A + a");
    assert_eq!(outcome.report.replaced_single_child, 2);

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_empty_group = false;
    let outcome = run_flatten_groups_with_config(r"a{} + {+}", cfg);
    assert_eq!(outcome.text, r"a + { + }");
    assert_eq!(outcome.report.removed_empty, 1);
    assert_eq!(
        outcome.report.preserved_group_with_lone_atom_spacing_char,
        1
    );

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_with_lone_atom_spacing_char = false;
    let outcome = run_flatten_groups_with_config(r"{+} + {-n}", cfg);
    assert_eq!(outcome.text, r"+ + { - n }");
    assert_eq!(outcome.report.replaced_single_child, 1);
    assert_eq!(
        outcome
            .report
            .preserved_group_starting_with_atom_spacing_char,
        1
    );

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_starting_with_atom_spacing_char = false;
    let outcome = run_flatten_groups_with_config(r"{+} + {-n}", cfg);
    assert_eq!(outcome.text, r"{ + } + - n");
    assert_eq!(
        outcome.report.preserved_group_with_lone_atom_spacing_char,
        1
    );
    assert_eq!(outcome.report.inlined_multi_child, 1);

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_containing_delimited_pair = false;
    let outcome = run_flatten_groups_with_config(r"f{\left(x\right)} + {a}", cfg);
    assert_eq!(outcome.text, r"f \left ( x \right ) + a");
    assert_eq!(outcome.report.replaced_single_child, 2);
}

#[test]
fn semantic_guard_toggles_affect_their_cases() {
    let mut cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_containing_declarative_command = false;
    cfg.preserve_group_adjacent_to_command_like = false;
    let outcome = run_flatten_groups_with_config(r"{\cal M} + {a}", cfg);
    assert_eq!(outcome.text, r"\cal M + a");
    assert_eq!(outcome.report.inlined_multi_child, 1);

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_containing_infix = false;
    let outcome = run_flatten_groups_with_config(r"{a \over b}, c", cfg);
    assert_eq!(outcome.text, r"a \over b , c");
    assert_eq!(outcome.report.preserved_group_containing_infix, 0);

    cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_inside_env_body = false;
    let outcome = run_flatten_groups_with_config(r"\begin{matrix}{x}\end{matrix}", cfg);
    assert_eq!(outcome.text, r"\begin {matrix} x \end {matrix}");
    assert_eq!(outcome.report.replaced_single_child, 1);
}

#[test]
fn preserve_guard_counters_short_circuit_on_first_match() {
    let outcome = run_flatten_groups(r"{\cal a \over b}");

    assert_eq!(
        outcome
            .report
            .preserved_group_containing_declarative_command,
        2
    );
    assert_eq!(outcome.report.preserved_group_containing_infix, 0);
}

#[test]
fn script_base_single_atom_groups_are_unwrapped() {
    let outcome = run_flatten_groups(r"{x}^2 + {\frac{1}{2}}^2");

    assert_eq!(outcome.text, r"x ^ { 2 } + \frac { 1 } { 2 } ^ { 2 }");
    assert_eq!(outcome.report.preserved_group_in_script_base_slot, 0);
}

#[test]
fn script_base_non_atomic_groups_are_preserved() {
    let outcome = run_flatten_groups(r"{x_i}^2 + {ab}^2 + {\sum_i x}^2");

    assert_eq!(
        outcome.text,
        r"{ x _ { i } } ^ { 2 } + { a b } ^ { 2 } + { \sum _ { i } x } ^ { 2 }"
    );
    assert_eq!(outcome.report.preserved_group_in_script_base_slot, 1);
}

#[test]
fn script_base_guard_can_be_disabled() {
    let mut cfg = FlattenGroupsConfig::STRICT;
    cfg.preserve_group_in_script_base_slot = false;
    let outcome = run_flatten_groups_with_config(r"{x_i}^2", cfg);

    assert_eq!(outcome.text, r"x _ { i } ^ { 2 }");
    assert_eq!(outcome.report.unwrapped_slot, 1);
}

#[test]
fn is_idempotent() {
    let once = run_flatten_groups(r"{{a}{bc}}\frac{{x}}{y}").text;
    let twice = run_flatten_groups(&once).text;

    assert_eq!(twice, once);
}
