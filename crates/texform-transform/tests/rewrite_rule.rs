use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_interface::syntax_node::{ContentMode, SyntaxNode};
use texform_knowledge::builtin::bboldx;
use texform_transform::{BuildConfig, Profile, RuleTarget, RuleTargetKind, TransformContext};

#[test]
fn character_rule_target_exposes_lookup_key() {
    let target = RuleTarget::Character(&bboldx::chars::BBDOTLESSI);
    let key = target.key();

    assert_eq!(key.kind, RuleTargetKind::Character);
    assert_eq!(key.name, "bbdotlessi");
}

#[test]
fn prime_character_alias_rewrites_command_to_prime_node() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");
    let mut ast = parse_to_ast(&parse_ctx, r"\prime");

    let report = context
        .run(&mut ast, &parse_ctx)
        .expect("transform should succeed");

    assert!(
        report
            .rewrite
            .rules
            .iter()
            .any(|rule| { rule.key.name == "prime-to-prime-node" && rule.applied_count == 1 })
    );
    assert_eq!(
        ast.to_syntax_root(),
        SyntaxNode::Root {
            mode: ContentMode::Math,
            children: vec![SyntaxNode::Prime { count: 1 }],
        }
    );
}

#[test]
fn prime_character_alias_metadata_has_no_produced_targets() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");
    let rule = context
        .rewrite_plan()
        .rules()
        .iter()
        .find(|rule| rule.meta().key.name == "prime-to-prime-node")
        .expect("prime rule should be registered");
    let meta = rule.meta();

    assert_eq!(meta.triggers.len(), 1);
    assert_eq!(meta.triggers[0].key().kind, RuleTargetKind::Character);
    assert_eq!(meta.triggers[0].key().name, "prime");
    assert_eq!(meta.consumes.eliminates.len(), 1);
    assert!(meta.produces.targets.is_empty());
}

fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
    let document = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("source should parse")
        .0;
    Ast::from_syntax_root(&document.to_syntax())
}
