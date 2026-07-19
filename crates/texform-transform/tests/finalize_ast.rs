use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::serialize;
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, GroupKind as SyntaxGroupKind, SyntaxNode,
};
use texform_transform::{
    BuildConfig, FinalizeAstConfig, FinalizeAstReport, FlattenGroupsConfig, Profile,
    TransformConfig, TransformContext, finalize_ast,
};

fn run_finalize(ast: &mut Ast, enabled: bool) -> FinalizeAstReport {
    let mut report = FinalizeAstReport::default();
    let config = FinalizeAstConfig { enabled };
    finalize_ast::run(ast, &config, &mut report);
    ast.assert_invariants();
    report
}

fn ast_from(children: Vec<SyntaxNode>) -> Ast {
    Ast::from_syntax_root(&SyntaxNode::Root {
        mode: ContentMode::Math,
        children,
    })
}

fn text_root(children: Vec<SyntaxNode>) -> Ast {
    Ast::from_syntax_root(&SyntaxNode::Root {
        mode: ContentMode::Text,
        children,
    })
}

fn math_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: SyntaxGroupKind::Explicit,
        children,
    }
}

fn text_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Text,
        kind: SyntaxGroupKind::Explicit,
        children,
    }
}

fn empty_implicit_text_group() -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Text,
        kind: SyntaxGroupKind::Implicit,
        children: vec![],
    }
}

fn math_arg(node: SyntaxNode) -> Option<Argument> {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::MathContent(node),
    ))
}

fn text_arg(node: SyntaxNode) -> Option<Argument> {
    Some(Argument::from_value(
        ArgumentKind::Mandatory,
        ArgumentValue::TextContent(node),
    ))
}

fn root_children(ast: &Ast) -> Vec<SyntaxNode> {
    match ast.to_syntax_root() {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {other:?}"),
    }
}

fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
    let document = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("source should parse")
        .0;
    Ast::from_syntax_root(&document.to_syntax())
}

fn run_engine(
    src: &str,
    finalize_ast: FinalizeAstConfig,
    flatten_groups: FlattenGroupsConfig,
) -> (Ast, texform_transform::TransformReport, String) {
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let mut ast = parse_to_ast(&parse_ctx, src);
    let config = TransformConfig {
        rewrite_enabled: true,
        lower_attributes_enabled: true,
        finalize_ast,
        flatten_groups,
        max_iterations: 100,
    };
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("transform context should build");
    let report = context
        .run_with(&mut ast, &parse_ctx, &config)
        .expect("transform should succeed");
    ast.assert_invariants();
    let text = serialize(&ast);
    (ast, report, text)
}

#[test]
fn disabled_config_leaves_adjacent_primes_unchanged() {
    let mut ast = ast_from(vec![
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 2 },
    ]);

    let report = run_finalize(&mut ast, false);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Prime { count: 1 },
            SyntaxNode::Prime { count: 2 },
        ]
    );
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 0);
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 0);
}

#[test]
fn enabled_config_merges_adjacent_primes_in_same_sequence() {
    let mut ast = ast_from(vec![
        SyntaxNode::Char('a'),
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 2 },
        SyntaxNode::Char('b'),
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 1 },
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Prime { count: 3 },
            SyntaxNode::Char('b'),
            SyntaxNode::Prime { count: 3 },
        ]
    );
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 2);
}

#[test]
fn one_contiguous_prime_run_counts_as_one_action() {
    let mut ast = ast_from(vec![
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 1 },
        SyntaxNode::Prime { count: 3 },
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(root_children(&ast), vec![SyntaxNode::Prime { count: 5 }]);
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 1);
}

#[test]
fn recursively_enters_math_arguments_scripts_and_environment_body() {
    let command = SyntaxNode::Command {
        name: "wrap".to_string(),
        args: vec![math_arg(math_group(vec![
            SyntaxNode::Prime { count: 1 },
            SyntaxNode::Prime { count: 1 },
        ]))],
        known: true,
    };
    let scripted = SyntaxNode::Scripted {
        base: Box::new(SyntaxNode::Char('x')),
        subscript: None,
        superscript: Some(Box::new(math_group(vec![
            SyntaxNode::Prime { count: 2 },
            SyntaxNode::Prime { count: 1 },
        ]))),
    };
    let environment = SyntaxNode::Environment {
        name: "matrix".to_string(),
        args: vec![],
        known: true,
        body: Box::new(math_group(vec![
            SyntaxNode::Prime { count: 1 },
            SyntaxNode::Prime { count: 2 },
        ])),
    };
    let mut ast = ast_from(vec![command, scripted, environment]);

    let report = run_finalize(&mut ast, true);

    let children = root_children(&ast);
    match &children[0] {
        SyntaxNode::Command { args, .. } => match &args[0].as_ref().unwrap().value {
            ArgumentValue::MathContent(node) => {
                assert_eq!(node, &math_group(vec![SyntaxNode::Prime { count: 2 }]))
            }
            other => panic!("expected math content argument, got {other:?}"),
        },
        other => panic!("expected command node, got {other:?}"),
    }
    match &children[1] {
        SyntaxNode::Scripted { superscript, .. } => assert_eq!(
            superscript.as_deref(),
            Some(&math_group(vec![SyntaxNode::Prime { count: 3 }]))
        ),
        other => panic!("expected scripted node, got {other:?}"),
    }
    match &children[2] {
        SyntaxNode::Environment { body, .. } => assert_eq!(
            body.as_ref(),
            &math_group(vec![SyntaxNode::Prime { count: 3 }])
        ),
        other => panic!("expected environment node, got {other:?}"),
    }
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 3);
}

#[test]
fn does_not_merge_across_containers_or_slots() {
    let scripted = SyntaxNode::Scripted {
        base: Box::new(SyntaxNode::Prime { count: 1 }),
        subscript: Some(Box::new(SyntaxNode::Prime { count: 1 })),
        superscript: Some(Box::new(SyntaxNode::Prime { count: 1 })),
    };
    let mut ast = ast_from(vec![
        SyntaxNode::Prime { count: 1 },
        math_group(vec![SyntaxNode::Prime { count: 1 }]),
        SyntaxNode::Prime { count: 1 },
        scripted,
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Prime { count: 1 },
            math_group(vec![SyntaxNode::Prime { count: 1 }]),
            SyntaxNode::Prime { count: 1 },
            SyntaxNode::Scripted {
                base: Box::new(SyntaxNode::Prime { count: 1 }),
                subscript: Some(Box::new(SyntaxNode::Prime { count: 1 })),
                superscript: Some(Box::new(SyntaxNode::Prime { count: 1 })),
            },
        ]
    );
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 0);
}

#[test]
fn ignores_text_mode_sequences_and_prime_commands_for_prime_merge() {
    let mut ast = ast_from(vec![
        text_group(vec![
            SyntaxNode::Prime { count: 1 },
            SyntaxNode::Prime { count: 1 },
        ]),
        SyntaxNode::Command {
            name: "prime".to_string(),
            args: vec![],
            known: true,
        },
        SyntaxNode::Command {
            name: "prime".to_string(),
            args: vec![],
            known: true,
        },
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![
            text_group(vec![
                SyntaxNode::Prime { count: 1 },
                SyntaxNode::Prime { count: 1 },
            ]),
            SyntaxNode::Command {
                name: "prime".to_string(),
                args: vec![],
                known: true,
            },
            SyntaxNode::Command {
                name: "prime".to_string(),
                args: vec![],
                known: true,
            },
        ]
    );
    assert_eq!(report.steps.merge_adjacent_primes.applied_count, 0);
}

#[test]
fn merges_adjacent_text_in_text_mode_root_and_group() {
    let mut root = text_root(vec![
        SyntaxNode::Text("a".into()),
        SyntaxNode::Text("b".into()),
        SyntaxNode::Text("c".into()),
    ]);
    let root_report = run_finalize(&mut root, true);
    assert_eq!(root_children(&root), vec![SyntaxNode::Text("abc".into())]);
    assert_eq!(root_report.steps.normalize_text_sequences.applied_count, 1);

    let mut nested = ast_from(vec![text_group(vec![
        SyntaxNode::Text("x".into()),
        SyntaxNode::Text("y".into()),
    ])]);
    let nested_report = run_finalize(&mut nested, true);
    assert_eq!(
        root_children(&nested),
        vec![text_group(vec![SyntaxNode::Text("xy".into())])]
    );
    assert_eq!(
        nested_report.steps.normalize_text_sequences.applied_count,
        1
    );
}

#[test]
fn collapses_singleton_text_whitespace_without_trimming_edges() {
    let cases = [
        ("  a", " a"),
        ("a  ", "a "),
        ("  a  ", " a "),
        (" \t ", " "),
        ("a \t\n\u{000C}\u{00A0}b", "a b"),
    ];

    for (input, expected) in cases {
        let mut ast = text_root(vec![SyntaxNode::Text(input.into())]);
        let report = run_finalize(&mut ast, true);
        assert_eq!(
            root_children(&ast),
            vec![SyntaxNode::Text(expected.into())],
            "input {input:?}"
        );
        assert_eq!(report.steps.normalize_text_sequences.applied_count, 1);
    }
}

#[test]
fn collapses_whitespace_across_adjacent_text_boundaries() {
    let mut ast = text_root(vec![
        SyntaxNode::Text("a ".into()),
        SyntaxNode::Text("\tb".into()),
        SyntaxNode::Text(" ".into()),
        SyntaxNode::Text("\n".into()),
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(root_children(&ast), vec![SyntaxNode::Text("a b ".into())]);
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 1);
}

#[test]
fn preserves_non_lexer_unicode_whitespace() {
    let mut ast = text_root(vec![SyntaxNode::Text(
        "a\u{2007}\u{202F}\u{3000}  b".into(),
    )]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![SyntaxNode::Text("a\u{2007}\u{202F}\u{3000} b".into())]
    );
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 1);
}

#[test]
fn non_text_children_block_text_merge() {
    let mut ast = text_root(vec![
        SyntaxNode::Text("a".into()),
        SyntaxNode::Command {
            name: "foo".to_string(),
            args: vec![],
            known: false,
        },
        SyntaxNode::Text("b".into()),
        text_group(vec![SyntaxNode::Text("c".into())]),
        SyntaxNode::Text("d".into()),
        SyntaxNode::ActiveSpace,
        SyntaxNode::Text("e".into()),
        SyntaxNode::Error {
            message: "x".into(),
            snippet: "x".into(),
        },
        SyntaxNode::Text("f".into()),
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Text("a".into()),
            SyntaxNode::Command {
                name: "foo".to_string(),
                args: vec![],
                known: false,
            },
            SyntaxNode::Text("b".into()),
            text_group(vec![SyntaxNode::Text("c".into())]),
            SyntaxNode::Text("d".into()),
            SyntaxNode::ActiveSpace,
            SyntaxNode::Text("e".into()),
            SyntaxNode::Error {
                message: "x".into(),
                snippet: "x".into(),
            },
            SyntaxNode::Text("f".into()),
        ]
    );
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 0);
}

#[test]
fn does_not_merge_text_across_arguments_or_slots() {
    let mut ast = ast_from(vec![SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![text_arg(SyntaxNode::Text("a".into()))],
        known: true,
    }]);
    // Two separate text arguments must stay separate even when adjacent in edges.
    let mut two_args = ast_from(vec![SyntaxNode::Command {
        name: "hbox".to_string(),
        args: vec![
            text_arg(SyntaxNode::Text("a".into())),
            text_arg(SyntaxNode::Text("b".into())),
        ],
        known: true,
    }]);

    let report = run_finalize(&mut ast, true);
    let report2 = run_finalize(&mut two_args, true);

    assert_eq!(report.steps.normalize_text_sequences.applied_count, 0);
    assert_eq!(report2.steps.normalize_text_sequences.applied_count, 0);
    match &root_children(&two_args)[0] {
        SyntaxNode::Command { args, .. } => {
            assert_eq!(
                args[0].as_ref().unwrap().value,
                ArgumentValue::TextContent(SyntaxNode::Text("a".into()))
            );
            assert_eq!(
                args[1].as_ref().unwrap().value,
                ArgumentValue::TextContent(SyntaxNode::Text("b".into()))
            );
        }
        other => panic!("expected command, got {other:?}"),
    }
}

#[test]
fn deletes_empty_text_from_sequence_but_keeps_space_text() {
    let mut with_empties = text_root(vec![
        SyntaxNode::Text("".into()),
        SyntaxNode::Text("a".into()),
        SyntaxNode::Text("".into()),
    ]);
    let empty_report = run_finalize(&mut with_empties, true);
    assert_eq!(
        root_children(&with_empties),
        vec![SyntaxNode::Text("a".into())]
    );
    assert_eq!(empty_report.steps.normalize_text_sequences.applied_count, 1);

    let mut space_only = text_root(vec![
        SyntaxNode::Text(" ".into()),
        SyntaxNode::Text("\t\n".into()),
    ]);
    let space_report = run_finalize(&mut space_only, true);
    assert_eq!(
        root_children(&space_only),
        vec![SyntaxNode::Text(" ".into())]
    );
    assert_eq!(space_report.steps.normalize_text_sequences.applied_count, 1);

    let mut all_empty = text_root(vec![
        SyntaxNode::Text("".into()),
        SyntaxNode::Text("".into()),
    ]);
    let all_empty_report = run_finalize(&mut all_empty, true);
    assert_eq!(root_children(&all_empty), vec![]);
    assert_eq!(
        all_empty_report
            .steps
            .normalize_text_sequences
            .applied_count,
        1
    );
}

#[test]
fn text_content_slot_normalizes_and_replaces_empty_with_implicit_group() {
    let mut ast = ast_from(vec![SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![text_arg(SyntaxNode::Text("a  b".into()))],
        known: true,
    }]);
    let report = run_finalize(&mut ast, true);
    match &root_children(&ast)[0] {
        SyntaxNode::Command { args, .. } => assert_eq!(
            args[0].as_ref().unwrap().value,
            ArgumentValue::TextContent(SyntaxNode::Text("a b".into()))
        ),
        other => panic!("expected command, got {other:?}"),
    }
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 1);

    let mut canonical = ast_from(vec![SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![text_arg(SyntaxNode::Text("a b".into()))],
        known: true,
    }]);
    let canonical_report = run_finalize(&mut canonical, true);
    assert_eq!(
        canonical_report
            .steps
            .normalize_text_sequences
            .applied_count,
        0
    );

    let mut empty = ast_from(vec![SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![text_arg(SyntaxNode::Text("".into()))],
        known: true,
    }]);
    let empty_report = run_finalize(&mut empty, true);
    match &root_children(&empty)[0] {
        SyntaxNode::Command { args, .. } => assert_eq!(
            args[0].as_ref().unwrap().value,
            ArgumentValue::TextContent(empty_implicit_text_group())
        ),
        other => panic!("expected command, got {other:?}"),
    }
    assert_eq!(empty_report.steps.normalize_text_sequences.applied_count, 1);

    let mut already_empty_group = ast_from(vec![SyntaxNode::Command {
        name: "text".to_string(),
        args: vec![text_arg(empty_implicit_text_group())],
        known: true,
    }]);
    let already_report = run_finalize(&mut already_empty_group, true);
    assert_eq!(
        already_report.steps.normalize_text_sequences.applied_count,
        0
    );
}

#[test]
fn math_mode_sequences_do_not_merge_adjacent_text() {
    let mut ast = ast_from(vec![
        SyntaxNode::Text("a".into()),
        SyntaxNode::Text("b".into()),
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![SyntaxNode::Text("a".into()), SyntaxNode::Text("b".into()),]
    );
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 0);
}

#[test]
fn disabled_config_leaves_text_sequences_unchanged() {
    let mut ast = text_root(vec![
        SyntaxNode::Text("a ".into()),
        SyntaxNode::Text("\tb".into()),
    ]);

    let report = run_finalize(&mut ast, false);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Text("a ".into()),
            SyntaxNode::Text("\tb".into()),
        ]
    );
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 0);
}

#[test]
fn second_finalize_on_canonical_ast_is_idempotent() {
    let mut ast = text_root(vec![
        SyntaxNode::Text("a ".into()),
        SyntaxNode::Text("\tb".into()),
        SyntaxNode::Text("".into()),
    ]);

    let first = run_finalize(&mut ast, true);
    let after_first = root_children(&ast);
    let first_text = first.steps.normalize_text_sequences.applied_count;
    assert!(first_text > 0);

    let mut report = first;
    finalize_ast::run(&mut ast, &FinalizeAstConfig::ENABLED, &mut report);
    ast.assert_invariants();

    assert_eq!(root_children(&ast), after_first);
    assert_eq!(
        report.steps.normalize_text_sequences.applied_count,
        first_text
    );
}

#[test]
fn one_changed_text_run_counts_as_one_action() {
    let mut ast = text_root(vec![
        SyntaxNode::Text("a".into()),
        SyntaxNode::Text("".into()),
        SyntaxNode::Text("  b  ".into()),
        SyntaxNode::Char('x'),
        SyntaxNode::Text("c".into()),
        SyntaxNode::Text("d".into()),
    ]);

    let report = run_finalize(&mut ast, true);

    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Text("a b ".into()),
            SyntaxNode::Char('x'),
            SyntaxNode::Text("cd".into()),
        ]
    );
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 2);
}

#[test]
fn empty_text_group_becomes_empty_group_before_flatten_strict_and_structural() {
    let mut pre = text_root(vec![text_group(vec![SyntaxNode::Text("".into())])]);
    let pre_report = run_finalize(&mut pre, true);
    assert_eq!(root_children(&pre), vec![text_group(vec![])]);
    assert_eq!(pre_report.steps.normalize_text_sequences.applied_count, 1);

    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");

    // Text group under text root so FlattenGroups mode matching allows unwrap.
    let mut structural = text_root(vec![text_group(vec![SyntaxNode::Text("".into())])]);
    let structural_cfg = TransformConfig {
        rewrite_enabled: false,
        lower_attributes_enabled: false,
        finalize_ast: FinalizeAstConfig::ENABLED,
        flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
        max_iterations: 100,
    };
    let structural_report = context
        .run_with(&mut structural, &parse_ctx, &structural_cfg)
        .expect("structural transform");
    assert_eq!(root_children(&structural), vec![]);
    assert!(structural_report.flatten_groups.actions.removed_empty >= 1);

    let mut strict = text_root(vec![text_group(vec![SyntaxNode::Text("".into())])]);
    let strict_cfg = TransformConfig {
        rewrite_enabled: false,
        lower_attributes_enabled: false,
        finalize_ast: FinalizeAstConfig::ENABLED,
        flatten_groups: FlattenGroupsConfig::STRICT,
        max_iterations: 100,
    };
    let strict_engine_report = context
        .run_with(&mut strict, &parse_ctx, &strict_cfg)
        .expect("strict transform");
    assert_eq!(root_children(&strict), vec![text_group(vec![])]);
    assert_eq!(strict_engine_report.flatten_groups.actions.removed_empty, 0);
    assert!(
        strict_engine_report
            .flatten_groups
            .guards
            .preserve_empty_group
            >= 1
    );
}

#[test]
fn wide_text_sequence_stays_linear() {
    let mut children = Vec::with_capacity(4_000);
    for i in 0..2_000 {
        children.push(SyntaxNode::Text(if i % 2 == 0 {
            "a ".into()
        } else {
            "\tb".into()
        }));
    }
    let mut ast = text_root(children);
    let report = run_finalize(&mut ast, true);
    assert_eq!(root_children(&ast).len(), 1);
    assert_eq!(report.steps.normalize_text_sequences.applied_count, 1);
    match &root_children(&ast)[0] {
        SyntaxNode::Text(text) => {
            assert!(text.starts_with("a b"));
            assert!(!text.contains('\t'));
            assert!(!text.contains("  "));
        }
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn engine_prime_merge_then_flatten_keeps_double_prime_shorthand() {
    let (_ast, report, text) = run_engine(
        r"f^{\prime\prime}",
        FinalizeAstConfig::ENABLED,
        FlattenGroupsConfig::STRUCTURAL_ONLY,
    );

    assert_eq!(text, "f''");
    assert_eq!(
        report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count,
        1
    );
}

#[test]
fn engine_text_merge_before_flatten_can_unwrap_singleton_group() {
    // Hand-built text-mode tree: group with two adjacent Text children becomes
    // a singleton after pre-FinalizeAst, then STRUCTURAL_ONLY FlattenGroups
    // unwraps it into the parent text sequence.
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");
    let mut ast = text_root(vec![text_group(vec![
        SyntaxNode::Text("a".into()),
        SyntaxNode::Text("b".into()),
    ])]);
    let report = context
        .run_with(
            &mut ast,
            &parse_ctx,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform");

    assert_eq!(root_children(&ast), vec![SyntaxNode::Text("ab".into())]);
    assert_eq!(
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count,
        1
    );
    assert!(report.flatten_groups.actions.replaced_single_child >= 1);
}

#[test]
fn engine_second_finalize_merges_text_exposed_by_flatten() {
    // Nested text groups: flatten inlines children, post-FinalizeAst merges.
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");
    let mut ast = text_root(vec![
        SyntaxNode::Text("a".into()),
        text_group(vec![SyntaxNode::Text("b".into())]),
        SyntaxNode::Text("c".into()),
    ]);
    let report = context
        .run_with(
            &mut ast,
            &parse_ctx,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform");

    assert_eq!(root_children(&ast), vec![SyntaxNode::Text("abc".into())]);
    assert!(
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count
            >= 1
    );
}

#[test]
fn engine_second_finalize_merges_primes_exposed_by_flatten() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");
    let mut ast = ast_from(vec![
        SyntaxNode::Prime { count: 1 },
        math_group(vec![SyntaxNode::Prime { count: 2 }]),
        SyntaxNode::Prime { count: 1 },
    ]);
    let report = context
        .run_with(
            &mut ast,
            &parse_ctx,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform");

    assert_eq!(root_children(&ast), vec![SyntaxNode::Prime { count: 4 }]);
    assert_eq!(
        report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count,
        1
    );
}

#[test]
fn engine_skips_second_finalize_when_flatten_disabled() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");
    let mut ast = text_root(vec![
        SyntaxNode::Text("a".into()),
        text_group(vec![SyntaxNode::Text("b".into())]),
        SyntaxNode::Text("c".into()),
    ]);
    let report = context
        .run_with(
            &mut ast,
            &parse_ctx,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::DISABLED,
                max_iterations: 100,
            },
        )
        .expect("transform");

    // Without FlattenGroups the nested group remains a merge barrier.
    assert_eq!(
        root_children(&ast),
        vec![
            SyntaxNode::Text("a".into()),
            text_group(vec![SyntaxNode::Text("b".into())]),
            SyntaxNode::Text("c".into()),
        ]
    );
    assert_eq!(
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count,
        0
    );
}

#[test]
fn engine_finalize_disabled_leaves_flatten_independent() {
    let (_ast, report, text) = run_engine(
        r"f^{\prime\prime}",
        FinalizeAstConfig::DISABLED,
        FlattenGroupsConfig::STRUCTURAL_ONLY,
    );

    assert_ne!(text, "f''");
    assert_eq!(
        report
            .finalize_ast
            .steps
            .merge_adjacent_primes
            .applied_count,
        0
    );
    assert_eq!(
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count,
        0
    );
}

#[test]
fn engine_report_accumulates_pre_and_post_without_double_counting() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
            .expect("context");
    let mut ast = text_root(vec![
        SyntaxNode::Text("a ".into()),
        SyntaxNode::Text("\tb".into()),
        text_group(vec![SyntaxNode::Text("c".into())]),
        SyntaxNode::Text("d".into()),
    ]);
    let report = context
        .run_with(
            &mut ast,
            &parse_ctx,
            &TransformConfig {
                rewrite_enabled: false,
                lower_attributes_enabled: false,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRUCTURAL_ONLY,
                max_iterations: 100,
            },
        )
        .expect("transform");

    assert_eq!(root_children(&ast), vec![SyntaxNode::Text("a bcd".into())]);
    // pre: merge "a "/"\tb" → 1; post: merge exposed adjacency → 1; no recount of
    // already-canonical fragments from the first pass.
    assert_eq!(
        report
            .finalize_ast
            .steps
            .normalize_text_sequences
            .applied_count,
        2
    );
}
