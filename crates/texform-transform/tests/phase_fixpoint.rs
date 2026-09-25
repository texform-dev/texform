//! Phase contracts and feedback opportunities across every runtime phase gate.
#[path = "support/phase_cases.rs"]
mod phase_cases;

use texform_core::{
    ast::Ast,
    parse::{ParseConfig, ParseContext},
    serialize::serialize,
};
use texform_transform::{report::ReportRecorder, *};

const PROFILES: [Profile; 4] = [
    Profile::Authoring,
    Profile::Faithful,
    Profile::Corpus,
    Profile::Equiv,
];
fn parser() -> ParseContext {
    ParseContext::from_packages(&[
        "base",
        "ams",
        "braket",
        "physics",
        "textmacros",
        "bboldx",
        "boldsymbol",
    ])
}
fn parse(p: &ParseContext, src: &str) -> Option<Ast> {
    p.parse(src, &ParseConfig::default())
        .try_into_document()
        .ok()
        .map(|(d, _)| Ast::from_syntax_root(&d.to_syntax()))
}
fn phase(
    ast: &mut Ast,
    p: &ParseContext,
    t: &TransformContext,
    cfg: &TransformConfig,
    index: usize,
) -> bool {
    let mut r = ReportRecorder::disabled();
    match index {
        0 => {
            cfg.lower_attributes.enabled
                && lower_attributes::run(ast, &cfg.lower_attributes, &mut r)
        }
        1 => {
            cfg.rewrite.enabled
                && rewrite::run(ast, p, t.rewrite_plan(), cfg.rewrite.max_iterations, &mut r)
                    .unwrap()
        }
        2 => finalize_ast::run(ast, &cfg.finalize_ast, &mut r),
        3 => {
            cfg.flatten_groups.enabled
                && flatten_groups::run(
                    ast,
                    &flatten_groups::FlattenGroupsGuards::from_config(cfg.flatten_groups),
                    &mut r,
                )
        }
        _ => unreachable!(),
    }
}
fn reference(ast: &mut Ast, p: &ParseContext, t: &TransformContext, cfg: &TransformConfig) {
    for _ in 0..16 {
        let before = ast.to_syntax_root();
        for index in [0, 1, 0, 2, 3] {
            phase(ast, p, t, cfg, index);
        }
        if before == ast.to_syntax_root() {
            return;
        }
    }
    panic!("reference closure did not converge");
}

#[test]
fn feedback_opportunities_finish_in_one_transform() {
    let p = parser();
    for profile in PROFILES {
        let t = TransformContext::from_build_config(BuildConfig::profile(profile), &p).unwrap();
        let structural = matches!(profile, Profile::Corpus | Profile::Equiv);
        let cases = [
            (
                r"\mathrel{{\mathrel{=}}}",
                if structural {
                    r"\mathrel { = }"
                } else {
                    r"\mathrel { \mathrel { = } }"
                },
            ),
            (r"\not{=}", if structural { r"\neq" } else { r"\not { = }" }),
            (
                r"a \dots {+ b}",
                if structural {
                    r"a \cdots + b"
                } else {
                    r"a \dots { + b }"
                },
            ),
            (
                r"D^{{\mathbf i}{\mathbf j}}",
                if structural {
                    r"D ^ { \mathbf { i j } }"
                } else {
                    r"D ^ { { \mathbf { i } } { \mathbf { j } } }"
                },
            ),
            (r"u^{{\prime}{\prime}}", "u''"),
            (r"\mathbf{\large \mathrm{x}}", r"\mathrm { \large x }"),
            (r"\mathbf{i}", r"\mathbf { i }"),
        ];
        for (src, expected) in cases {
            let mut ast = parse(&p, src).unwrap();
            t.run(&mut ast, &p).unwrap();
            assert_eq!(serialize(&ast), expected, "{profile:?}: {src}");
            let once = ast.to_syntax_root();
            t.run(&mut ast, &p).unwrap();
            assert_eq!(ast.to_syntax_root(), once, "{profile:?}: {src}");
        }
        for src in [
            r"\mathbf{\not\vb{=}}",
            r"\mathbf{\enspace\vb{\enspace}}",
            r"\dots{\prime\prime}",
            r"\enspace{\enspace}",
            r"\underline{{{+}}}",
            r"H^{{\frac{m-n}{4}}}",
        ] {
            let mut ast = parse(&p, src).unwrap();
            let mut expected = ast.clone();
            reference(&mut expected, &p, &t, t.default_config());
            t.run(&mut ast, &p).unwrap();
            assert_eq!(
                ast.to_syntax_root(),
                expected.to_syntax_root(),
                "{profile:?}: {src}"
            );
        }
        if structural {
            let mut ast = parse(&p, r"\dots{}+").unwrap();
            t.run(&mut ast, &p).unwrap();
            assert_eq!(serialize(&ast), r"\ldots +");
        }
    }
}

#[test]
fn phase_contracts_and_all_gate_combinations_reach_the_reference_fixed_point() {
    let p = parser();
    let inputs = phase_cases::combinations();
    let mut checked = 0;
    for profile in PROFILES {
        let t = TransformContext::from_build_config(BuildConfig::profile(profile), &p).unwrap();
        for src in &inputs {
            let Some(original) = parse(&p, src) else {
                continue;
            };
            // Audit each phase at intermediate states, including states created by Rewrite.
            let mut ast = original.clone();
            for index in [0, 1, 0, 2, 3] {
                let before = ast.to_syntax_root();
                let changed = phase(&mut ast, &p, &t, t.default_config(), index);
                let once = ast.to_syntax_root();
                assert!(
                    changed || before == once,
                    "C2 {profile:?} phase={index}: {src}"
                );
                let changed_again = phase(&mut ast, &p, &t, t.default_config(), index);
                assert_eq!(
                    ast.to_syntax_root(),
                    once,
                    "C1 {profile:?} phase={index}: {src}"
                );
                assert!(
                    index == 0 || !changed_again,
                    "C3 {profile:?} phase={index}: {src}"
                );
            }
            for gates in 0..16 {
                let mut cfg = *t.default_config();
                cfg.lower_attributes.enabled = gates & 1 != 0;
                cfg.rewrite.enabled = gates & 2 != 0;
                cfg.finalize_ast.enabled = gates & 4 != 0;
                cfg.flatten_groups.enabled = gates & 8 != 0;
                let mut ast = original.clone();
                t.run_with(&mut ast, &p, &cfg)
                    .unwrap_or_else(|e| panic!("{profile:?} gates={gates} {src}: {e}"));
                let once = ast.to_syntax_root();
                let mut expected = original.clone();
                reference(&mut expected, &p, &t, &cfg);
                assert_eq!(
                    once,
                    expected.to_syntax_root(),
                    "reference {profile:?} gates={gates}: {src}"
                );
                let report = t.run_with_report(&mut ast, &p, &cfg).unwrap();
                assert_eq!(
                    ast.to_syntax_root(),
                    once,
                    "continue {profile:?} gates={gates}: {src}"
                );
                if gates == 0 {
                    assert_eq!(report, TransformReport::default());
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked > 70_000,
        "the composition generator must exercise thousands of parsed inputs"
    );
}

#[test]
fn reports_accumulate_rewrite_iterations_across_phase_rounds() {
    let p = parser();
    let t = TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &p).unwrap();
    let mut ast = parse(&p, r"\not{=}").unwrap();
    let mut plain = ast.clone();
    t.run(&mut plain, &p).unwrap();
    let report = t.run_with_report(&mut ast, &p, t.default_config()).unwrap();
    assert_eq!(ast.to_syntax_root(), plain.to_syntax_root());
    assert_eq!(serialize(&ast), r"\neq");
    assert_eq!(report.rewrite.iterations, 3);
}
