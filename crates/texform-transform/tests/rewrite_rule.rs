use texform_core::ast::Ast;
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::serialize;
use texform_interface::syntax_node::{ContentMode, SyntaxNode};
use texform_transform::{BuildConfig, Profile, TransformContext};

#[test]
fn prime_character_alias_rewrites_command_to_prime_node() {
    let parse_ctx = ParseContext::from_packages(&["base"]);
    let context =
        TransformContext::from_build_config(BuildConfig::profile(Profile::Authoring), &parse_ctx)
            .expect("transform context should build");
    let mut ast = parse_to_ast(&parse_ctx, r"\prime");

    let report = context
        .run_with_report(&mut ast, &parse_ctx, context.default_config())
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

/// The long physics names are plain operators, while the short names they alias grab
/// a following power or argument, so the authoring aliases rename only bare forms
/// and faithful rewrites the rest to `\operatorname`.
#[test]
fn physics_long_named_functions_keep_plain_operator_semantics() {
    let parse_ctx = ParseContext::from_packages(&["base", "ams", "physics"]);
    let cases = [
        (
            r"\sine(\frac{x}{y})",
            r"\sine ( \frac { x } { y } )",
            r"\operatorname {sin} ( \frac { x } { y } )",
        ),
        (
            r"\naturallogarithm(x)",
            r"\naturallogarithm ( x )",
            r"\operatorname {ln} ( x )",
        ),
        (
            r"\sine[2](x)",
            r"\sine [ 2 ] ( x )",
            r"\operatorname {sin} [ 2 ] ( x )",
        ),
        (
            r"\Probability(\frac{x}{y})",
            r"\Probability ( \frac { x } { y } )",
            r"\operatorname {Pr} ( \frac { x } { y } )",
        ),
        (
            r"\sine x+\cosine^2 x",
            r"\sin x + \cos ^ { 2 } x",
            r"\sin x + \cos ^ { 2 } x",
        ),
    ];
    for profile in [
        Profile::Authoring,
        Profile::Faithful,
        Profile::Corpus,
        Profile::Equiv,
    ] {
        let context =
            TransformContext::from_build_config(BuildConfig::profile(profile), &parse_ctx)
                .expect("transform context should build");
        for (input, authoring, faithful) in cases {
            let mut ast = parse_to_ast(&parse_ctx, input);
            context
                .run_with(&mut ast, &parse_ctx, context.default_config())
                .unwrap_or_else(|error| panic!("{profile:?} failed on {input}: {error}"));
            let expected = if profile == Profile::Authoring {
                authoring
            } else {
                faithful
            };
            assert_eq!(serialize(&ast), expected, "{profile:?}: {input}");
        }
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
