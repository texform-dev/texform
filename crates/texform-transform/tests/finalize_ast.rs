use texform_core::ast::Ast;
use texform_interface::syntax_node::{
    Argument, ArgumentKind, ArgumentValue, ContentMode, GroupKind, SyntaxNode,
};
use texform_transform::{FinalizeAstConfig, FinalizeAstReport, finalize_ast};

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

fn math_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Math,
        kind: GroupKind::Explicit,
        children,
    }
}

fn text_group(children: Vec<SyntaxNode>) -> SyntaxNode {
    SyntaxNode::Group {
        mode: ContentMode::Text,
        kind: GroupKind::Explicit,
        children,
    }
}

fn math_arg(node: SyntaxNode) -> Option<Argument> {
    Some(Argument {
        kind: ArgumentKind::Mandatory,
        value: ArgumentValue::MathContent(node),
    })
}

fn root_children(ast: &Ast) -> Vec<SyntaxNode> {
    match ast.to_syntax_root() {
        SyntaxNode::Root { children, .. } => children,
        other => panic!("expected root node, got {other:?}"),
    }
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
fn ignores_text_mode_sequences_and_prime_commands() {
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
