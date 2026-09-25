//! Parser and transform contracts compare slot content, not construction history.
#[path = "support/phase_cases.rs"]
mod phase_cases;

use texform_core::{
    ast::Ast,
    parse::{ParseConfig, ParseContext},
    serialize::serialize,
};
use texform_interface::syntax_node::{ArgumentSlot, ArgumentValue, GroupKind, SyntaxNode};
use texform_transform::{BuildConfig, Profile, TransformContext};

const PACKAGES: &[&str] = &[
    "base",
    "ams",
    "braket",
    "textmacros",
    "bboldx",
    "boldsymbol",
];
const PROFILES: [Profile; 4] = [
    Profile::Authoring,
    Profile::Faithful,
    Profile::Corpus,
    Profile::Equiv,
];

fn canonical(mut node: SyntaxNode, slot: bool) -> SyntaxNode {
    if slot
        && let SyntaxNode::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            children,
            ..
        } = &mut node
        && children.len() == 1
    {
        // Strip exactly the slot's own container, never its child's braces.
        node = children.remove(0);
    }
    fn args(args: &mut [ArgumentSlot]) {
        for arg in args.iter_mut().flatten() {
            match &mut arg.value {
                ArgumentValue::MathContent(n)
                | ArgumentValue::TextContent(n)
                | ArgumentValue::OperatorNameContent(n) => *n = canonical(n.clone(), true),
                _ => {}
            }
        }
    }
    match &mut node {
        SyntaxNode::Root { children, .. } | SyntaxNode::Group { children, .. } => {
            for child in children {
                *child = canonical(child.clone(), false);
            }
        }
        SyntaxNode::Command { args: a, .. } | SyntaxNode::Declarative { args: a, .. } => args(a),
        SyntaxNode::Environment { args: a, body, .. } => {
            args(a);
            **body = canonical((**body).clone(), true);
        }
        SyntaxNode::Infix {
            args: a,
            left,
            right,
            ..
        } => {
            args(a);
            **left = canonical((**left).clone(), true);
            **right = canonical((**right).clone(), true);
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            **base = canonical((**base).clone(), true);
            for child in [subscript, superscript].into_iter().flatten() {
                **child = canonical((**child).clone(), true);
            }
        }
        _ => {}
    }
    if let SyntaxNode::Group {
        kind: kind @ (GroupKind::Explicit | GroupKind::Implicit),
        ..
    } = &mut node
    {
        *kind = GroupKind::Implicit;
    }
    node
}

fn parse(p: &ParseContext, source: &str) -> Option<Ast> {
    p.parse(source, &ParseConfig::default())
        .try_into_document()
        .ok()
        .map(|(d, _)| Ast::from_syntax_root(&d.to_syntax()))
}

fn check(p: &ParseContext, engines: &[TransformContext], source: &str) -> Result<(), String> {
    let Some(original) = parse(p, source) else {
        return Ok(());
    };
    let serialized = serialize(&original);
    let reparsed =
        parse(p, &serialized).ok_or_else(|| format!("parser failed: {source} => {serialized}"))?;
    if canonical(original.to_syntax_root(), false) != canonical(reparsed.to_syntax_root(), false) {
        return Err(format!("parser structure: {source} => {serialized}"));
    }
    for (profile, engine) in PROFILES.into_iter().zip(engines) {
        let mut ast = original.clone();
        if engine.run(&mut ast, p).is_err() {
            continue;
        }
        let once = serialize(&ast);
        let mut second = parse(p, &once)
            .ok_or_else(|| format!("{profile:?} parse failed: {source} => {once}"))?;
        engine
            .run(&mut second, p)
            .map_err(|e| format!("{profile:?} second transform: {source}: {e}"))?;
        if canonical(ast.to_syntax_root(), false) != canonical(second.to_syntax_root(), false)
            || once != serialize(&second)
        {
            return Err(format!(
                "{profile:?} transform structure: {source} => {once} => {}",
                serialize(&second)
            ));
        }
        let stable = ast.to_syntax_root();
        engine.run(&mut ast, p).unwrap();
        if ast.to_syntax_root() != stable {
            return Err(format!("{profile:?} continuation: {source}"));
        }
    }
    Ok(())
}

fn engines(p: &ParseContext) -> Vec<TransformContext> {
    PROFILES
        .into_iter()
        .map(|profile| {
            TransformContext::from_build_config(BuildConfig::profile(profile), p).unwrap()
        })
        .collect()
}

#[test]
fn generated_inputs_preserve_parser_and_transform_structure() {
    let p = ParseContext::from_packages(PACKAGES);
    let engines = engines(&p);
    let mut inputs: Vec<_> = phase_cases::combinations()
        .into_iter()
        .map(|s| s.replace(r"\vb", r"\mathbf"))
        .collect();
    for depth in 0..5 {
        for content in [
            r"\sum",
            "",
            r"\mathcal{L}",
            r"\displaystyle x",
            "]",
            "[0,1]",
            r"\left[0,1\right]",
        ] {
            let nested = format!("{}{}{}", "{".repeat(depth), content, "}".repeat(depth));
            for source in [
                format!(r"\overline{{{nested}}}"),
                format!(r"\mathrm{{{nested}}}"),
                format!(r"\xrightarrow[{{{nested}}}]{{x}}"),
                format!(r"\begin{{matrix}}\frac{{{nested}}}{{b}}\end{{matrix}}"),
            ] {
                inputs.push(source);
            }
        }
    }
    inputs.extend(
        [
            "C \\",
            "C \\\nx",
            "C \\\r\nx",
            "\\text{a\\\nb}",
            r"\mathrm{C \}",
            r"\xrightarrow[{\right]}]{x}",
        ]
        .map(str::to_owned),
    );
    for source in inputs {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check(&p, &engines, &source)
        }))
        .unwrap_or_else(|_| panic!("panic for {source}"))
        .unwrap_or_else(|error| panic!("{error}"));
    }
}

#[test]
fn design_examples_reach_the_expected_output_in_one_run() {
    let p = ParseContext::from_packages(PACKAGES);
    for engine in engines(&p) {
        for (source, expected) in [
            (r"\overline{{{\Psi}}}", r"\overline { \Psi }"),
            (r"\mathrel{{\mathrel{=}}}", r"\mathrel { = }"),
            (
                r"\begin{matrix}\frac{{{q}}}{b}\end{matrix}",
                r"\begin {matrix} \frac { q } { b } \end {matrix}",
            ),
            (r"y = 1, \mathrm{{}} z", "y = 1 , z"),
            (r"x{\rm{ }}y", "x y"),
            (
                r"\begin{array}{r}{\mathrm{{\mathcal{L}}}=x}\end{array}",
                r"\begin {array} {r} { \mathcal { L } = x } \end {array}",
            ),
            (
                r"\xrightarrow[{[0,1]}]{x}",
                r"\xrightarrow [ { [ 0 , 1 ] } ] { x }",
            ),
            (r"\xrightarrow[{]}]{x}", r"\xrightarrow [ { ] } ] { x }"),
            (r"\xrightarrow[{{]}}]{x}", r"\xrightarrow [ { ] } ] { x }"),
            (r"\overline{{\sum}}", r"\overline { \sum }"),
            (
                r"\frac{{\displaystyle x}}{y}",
                r"\frac { \displaystyle x } { y }",
            ),
            ("C \\", "C ~"),
        ] {
            let mut ast = parse(&p, source).unwrap();
            engine.run(&mut ast, &p).unwrap();
            assert_eq!(serialize(&ast), expected, "{source}");
            check(&p, std::slice::from_ref(&engine), source).unwrap();
        }
    }
    let mut packages = PACKAGES.to_vec();
    packages.push("physics");
    let p = ParseContext::from_packages(&packages);
    for engine in engines(&p) {
        let mut ast = parse(&p, r"\qty[{]}]").unwrap();
        engine.run(&mut ast, &p).unwrap();
        assert_eq!(serialize(&ast), r"\qty [ { ] } ]");
        check(&p, std::slice::from_ref(&engine), r"\qty[{]}]").unwrap();
    }
}

#[test]
#[ignore = "set TEXFORM_ROUNDTRIP_CORPUS to a complete single-line corpus file"]
fn corpus_preserves_parser_and_transform_structure() {
    use std::io::BufRead;
    let path = std::env::var("TEXFORM_ROUNDTRIP_CORPUS").expect("corpus path required");
    let p = ParseContext::from_packages(PACKAGES);
    let engines = engines(&p);
    let mut failures = 0;
    let mut checked = 0;
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let source = line.unwrap();
        checked += 1;
        if let Err(error) = check(&p, &engines, &source) {
            eprintln!("{error}");
            failures += 1;
        }
    }
    println!("roundtrip corpus: {checked} inputs, {failures} failures");
    assert!(checked > 0);
    assert_eq!(failures, 0);
}

#[test]
fn companion_fixes_keep_parser_shaped_trees() {
    let p = ParseContext::from_packages(PACKAGES);
    let engines = engines(&p);
    for source in [
        // Rewritten nodes match what the parser builds.
        r"\Biggr\}",
        r"\Bigl\{",
        r"\newline",
        r"\break",
        // Prime runs merged with an explicit superscript.
        "f'^{2}",
        "x''^2",
        r"\operatorname{\prime\prime}",
        // Whitespace after a text-mode control word.
        r"\text{\tiny{M}}",
        r"\text{\textcircled{O}}",
        r"\text{\bf\large x\rm y}",
        // Environment bodies stay groups.
        r"\begin{array}{cc}\rm{ab} & y\end{array}",
        r"\begin{matrix} a \over b \end{matrix}",
        r"\begin{array}{c} a \over b \end{array}",
        // Lowering consumes argument containers in one run.
        r"\mathbf{{{\rm{ab}}}}",
        r"{\rm{ }}",
        r"T_{\rm{eff}}",
    ] {
        check(&p, &engines, source).unwrap_or_else(|error| panic!("{error}"));
    }
}
