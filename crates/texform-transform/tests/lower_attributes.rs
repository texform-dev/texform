//! End-to-end behavior of the LowerAttributes phase.
//!
//! Tests go through the public `run` entry point with the standard
//! rule level, so they implicitly check that LowerAttributes composes cleanly
//! with the Rewrite phase.

use texform_core::ast::{Ast, Node, NodeId, Slot};
use texform_core::parse::{ParseConfig, ParseContext};
use texform_core::serialize::serialize;
use texform_interface::syntax_node::{ContentMode, GroupKind, SyntaxNode};
use texform_transform::lower_attributes::MathFontValue;
use texform_transform::{
    Attr, AttrValue, AttributeSet, BuildConfig, FlattenGroupsConfig, LowerAttributesConfig,
    Profile, RuleLevel, RuleLevelSet, TransformConfig, TransformContext,
};

struct Outcome {
    text: String,
    ast: Ast,
}

const PROFILES: [Profile; 4] = [
    Profile::Authoring,
    Profile::Faithful,
    Profile::Corpus,
    Profile::Equiv,
];

fn run_with_packages(src: &str, packages: &[&str]) -> Outcome {
    run_with_packages_and_levels(src, packages, &[RuleLevel::Authoring])
}

fn run_with_packages_and_levels(src: &str, packages: &[&str], levels: &[RuleLevel]) -> Outcome {
    let parse_ctx = ParseContext::from_packages(packages);
    let mut ast = parse_to_ast(&parse_ctx, src);
    let levels = levels
        .iter()
        .copied()
        .fold(RuleLevelSet::empty(), |set, level| set | level.into());
    let context = TransformContext::from_build_config(
        BuildConfig::profile(Profile::Authoring).rule_levels(levels),
        &parse_ctx,
    )
    .expect("transform context should build");
    context
        .run(&mut ast, &parse_ctx)
        .expect("transform should succeed");
    ast.assert_invariants();
    Outcome {
        text: serialize(&ast),
        ast,
    }
}

fn run(src: &str) -> Outcome {
    run_with_packages(src, &["base", "textmacros"])
}

fn serialized_with_packages(src: &str, expected: &str, packages: &[&str]) {
    let actual = run_with_packages(src, packages);
    actual.ast.assert_invariants();

    let parse_ctx = ParseContext::from_packages(packages);
    let expected_ast = parse_to_ast(&parse_ctx, expected);
    expected_ast.assert_invariants();

    assert_eq!(actual.text, serialize(&expected_ast));
}

fn serialized(src: &str, expected: &str) {
    serialized_with_packages(src, expected, &["base", "textmacros"]);
}

fn serialized_text(src: &str, expected: &str) {
    assert_eq!(run(src).text, expected);
}

fn parse_to_ast(parse_ctx: &ParseContext, src: &str) -> Ast {
    let document = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .expect("source should parse")
        .0;
    Ast::from_syntax_root(&document.to_syntax())
}

#[test]
fn consumes_explicit_groups_that_scope_declaratives() {
    serialized(r"{\bf x}y", r"\mathbf{x}y");
    serialized(r"{\bf \rm x}", r"\mathrm{x}");
    serialized_text(r"{\bf {\bf x}}", r"\mathbf { x }");
}

#[test]
fn flattens_structural_explicit_groups_after_lowering() {
    serialized_text(r"\mathbf{{x}}", r"\mathbf { x }");
    serialized_text(r"\mathbf{{\mathbf{x}}}", r"\mathbf { x }");
    serialized(r"{{\bf x}}", r"{\mathbf{x}}");
}

#[test]
fn absorbs_nested_math_prefix_wrappers() {
    serialized(r"\mathbf{\mathbf{x}}", r"\mathbf{x}");
    serialized(r"\mathbf{\mathit{x}}", r"\mathit{x}");
    serialized(r"\mathbf{\mathit{x}y}", r"\mathit{x}\mathbf{y}");
    serialized(r"\mathbf{\mathrm{a\mathbf{b}}}", r"\mathrm{a}\mathbf{b}");
}

#[test]
fn falls_back_to_preserving_noop_declarative_groups() {
    serialized(r"{x \bf}", r"x");
    serialized_text(r"\mathbf{{\bf x}}", r"\mathbf { x }");
    serialized_text(r"{\bf}", r"{ }");
    serialized_text(r"{\bf \rm}", r"{ }");
}

#[test]
fn preserves_text_multi_axis_combinations() {
    serialized(r"\text{\textbf{\textit{x}}}", r"\text{\textbf{\textit{x}}}");
    serialized(r"\text{\textbf{\textbf{x}}}", r"\text{\textbf{x}}");
    serialized(r"\text{\textsf{\textrm{x}}}", r"\text{\textrm{x}}");
}

#[test]
fn lower_attributes_preserves_whitespace_only_text_prefix_body() {
    serialized_text(r"\textrm{ }", r"\textrm { }");
    serialized_text(r"\textbf{ }", r"\textbf { }");
}

#[test]
fn lower_attributes_preserves_text_prefix_edge_spaces() {
    serialized_text(r"\textbf{ a }", r"\textbf { a }");
    serialized_text(r"\textbf{ a}", r"\textbf { a}");
    serialized_text(r"\textbf{a }", r"\textbf {a }");
}

#[test]
fn lower_attributes_keeps_empty_prefix_body_empty() {
    serialized_text(r"\textrm{}", r"");
    serialized_text(r"\textbf{}", r"");
}

#[test]
fn isolates_math_and_text_attribute_state() {
    serialized(
        r"{\bf \text{\textit{y}}} z",
        r"\mathbf{\text{\textit{y}}} z",
    );
}

#[test]
fn keeps_prefix_argument_declaratives_inside_the_wrapper() {
    // Size and style have no prefix, so they stay inside the original argument
    // instead of being emitted beside the rebuilt `\mathbf`. The parser folds a
    // single argument group away, so the contract is the serialized text.
    let cases = [
        (r"\mathbf{\Huge \Huge x}", r"\mathbf { \Huge x }"),
        (r"\mathbf{\large \Huge x}", r"\mathbf { \Huge x }"),
        (
            r"\mathbf{\large \scriptstyle x}",
            r"\mathbf { \scriptstyle \large x }",
        ),
    ];
    for (source, expected) in cases {
        let outcome = run(source);
        assert_eq!(outcome.text, expected);
        let declaratives = node_ids_where(&outcome.ast, |node| {
            matches!(node, Node::Declarative { .. })
        });
        assert!(!declaratives.is_empty(), "{source}");
        for id in declaratives {
            assert!(
                has_ancestor(&outcome.ast, id, |node, _| {
                    matches!(node, Node::Command { name, .. } if name == "mathbf")
                }),
                "{source} lifted a declarative out of \\mathbf"
            );
        }
    }
}

#[test]
fn lower_attributes_is_idempotent_on_serialized_output() {
    for src in [
        r"{\bf {\bf x}}",
        r"\mathbf{\mathit{x}y}",
        r"\text{\textbf{\textit{x}}}",
        r"\mathbf{{\mathbf{x}}}",
        r"{\scriptstyle x} y",
        r"\mathbf{\scriptstyle x}y",
        r"\text{{\cal x} y}",
    ] {
        let once = run(src).text;
        let twice = run(&once).text;
        assert_eq!(
            twice, once,
            "LowerAttributes should be idempotent for {src}"
        );
    }
}

#[test]
fn post_pass_normalizes_prefixes_created_by_apply_rules() {
    let actual = run_with_packages_and_levels(
        r"\vb{\rm x}",
        &["base", "textmacros", "physics", "boldsymbol"],
        &[RuleLevel::Authoring, RuleLevel::Faithful],
    );
    assert_eq!(actual.text, r"\mathrm { x }");
}

#[test]
fn lower_attributes_report_counts_declarative_and_prefix_forms_for_same_attribute() {
    let parse_ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let mut ast = parse_to_ast(&parse_ctx, r"\bf \mathbf{x}");
    let mut recorder = texform_transform::report::ReportRecorder::collecting();
    texform_transform::lower_attributes::run(
        &mut ast,
        &LowerAttributesConfig::ENABLED,
        &mut recorder,
    );
    let report = recorder.into_report().lower_attributes;
    let bold = AttributeSet::new(
        Attr::MathFont,
        AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
    );
    let stat = report
        .attributes
        .get(&bold)
        .expect("bold math font should be reported");

    assert_eq!(stat.consumed.declaratives, 1);
    assert_eq!(stat.consumed.prefixes, 1);
    assert_eq!(stat.redundant.prefixes, 1);
    assert_eq!(stat.emitted.prefixes, 1);
}

#[test]
fn lower_attributes_report_counts_empty_prefix_body_as_redundant() {
    let parse_ctx = ParseContext::from_packages(&["base", "textmacros"]);
    let mut ast = parse_to_ast(&parse_ctx, r"\mathbf{}");
    let mut recorder = texform_transform::report::ReportRecorder::collecting();
    texform_transform::lower_attributes::run(
        &mut ast,
        &LowerAttributesConfig::ENABLED,
        &mut recorder,
    );
    let report = recorder.into_report().lower_attributes;
    let bold = AttributeSet::new(
        Attr::MathFont,
        AttrValue::MathFont(MathFontValue("VARIANT.BOLD")),
    );
    let stat = report
        .attributes
        .get(&bold)
        .expect("empty bold prefix should be reported");

    assert_eq!(stat.consumed.prefixes, 1);
    assert_eq!(stat.redundant.prefixes, 1);
    assert_eq!(stat.emitted.prefixes, 0);
}

const SCOPE_PACKAGES: &[&str] = &["base", "textmacros"];

/// A local-scope case: `(source, expected, declarative, inside, outside)`.
///
/// `expected` is LaTeX that is reparsed and reserialized before comparison, so
/// the table never hardcodes serializer spacing.
type ScopeCase = (&'static str, &'static str, &'static str, char, char);

/// A case whose canonical serialization the phase must leave unchanged.
const fn kept(
    source: &'static str,
    declarative: &'static str,
    inside: char,
    outside: char,
) -> ScopeCase {
    (source, source, declarative, inside, outside)
}

/// A case the phase normalizes into a different but equivalent form.
const fn normalized(
    source: &'static str,
    expected: &'static str,
    declarative: &'static str,
    inside: char,
    outside: char,
) -> ScopeCase {
    (source, expected, declarative, inside, outside)
}

/// Declarative scopes that must survive normalization unchanged.
///
/// Each `declarative` must still govern `inside` and must not reach `outside`.
const SCOPE_CASES: &[ScopeCase] = &[
    kept(r"{\scriptstyle x} y", "scriptstyle", 'x', 'y'),
    kept(
        r"{\textstyle\frac{1}{2}}+\frac{3}{4}",
        "textstyle",
        '1',
        '3',
    ),
    kept(r"{\large x} y", "large", 'x', 'y'),
    kept(r"\text{{\large x} y}", "large", 'x', 'y'),
    kept(r"{\oldstyle 1}2", "oldstyle", '1', '2'),
    kept(r"\text{{\cal x} y}", "cal", 'x', 'y'),
    kept(r"\text{{\mit x} y}", "mit", 'x', 'y'),
    kept(r"\text{{\oldstyle 1}2}", "oldstyle", '1', '2'),
    kept(r"{\scriptstyle {\large x}} y", "scriptstyle", 'x', 'y'),
    kept(r"{\scriptstyle x}{\large y}z", "scriptstyle", 'x', 'z'),
    normalized(
        r"\mathbf{\scriptstyle x}y",
        r"\mathbf { \scriptstyle x } y",
        "scriptstyle",
        'x',
        'y',
    ),
    normalized(
        r"{\large \scriptstyle x} y",
        r"{\scriptstyle \large x} y",
        "scriptstyle",
        'x',
        'y',
    ),
    normalized(
        r"{\scriptstyle x \large} y",
        r"{\scriptstyle x} y",
        "scriptstyle",
        'x',
        'y',
    ),
    normalized(
        r"{\bf \large \scriptstyle x} y",
        r"{\scriptstyle \large \mathbf{x}} y",
        "scriptstyle",
        'x',
        'y',
    ),
    normalized(
        r"{\scriptstyle \scriptstyle x} y",
        r"{\scriptstyle x} y",
        "scriptstyle",
        'x',
        'y',
    ),
];

fn run_configured(
    src: &str,
    packages: &[&str],
    profile: Profile,
    config: &TransformConfig,
) -> Outcome {
    let parse_ctx = ParseContext::from_packages(packages);
    let mut ast = parse_to_ast(&parse_ctx, src);
    let context = TransformContext::from_build_config(BuildConfig::profile(profile), &parse_ctx)
        .expect("transform context should build");
    context
        .run_with(&mut ast, &parse_ctx, config)
        .expect("transform should succeed");
    ast.assert_invariants();
    Outcome {
        text: serialize(&ast),
        ast,
    }
}

fn canonical(packages: &[&str], src: &str) -> String {
    let parse_ctx = ParseContext::from_packages(packages);
    serialize(&parse_to_ast(&parse_ctx, src))
}

fn assert_reparseable(packages: &[&str], src: &str) {
    let parse_ctx = ParseContext::from_packages(packages);
    let (document, _) = parse_ctx
        .parse(src, &ParseConfig::default())
        .try_into_document()
        .unwrap_or_else(|_| panic!("normalized output should reparse: {src}"));
    assert!(
        !document.has_errors(),
        "normalized output should not contain error nodes: {src}"
    );
}

/// Normalize `source` on every profile with the profile default config.
///
/// Each profile must produce `expected`, satisfy `check`, reparse cleanly, and
/// be a fixed point of a second normalize.
fn for_each_profile(
    source: &str,
    packages: &[&str],
    expected: &str,
    mut check: impl FnMut(&Ast, &str),
) {
    for profile in PROFILES {
        let config = profile.default_transform_config();
        let context = format!("{profile:?} {source}");
        let first = run_configured(source, packages, profile, &config);
        assert_eq!(first.text, expected, "{context}");
        check(&first.ast, &context);
        assert_reparseable(packages, &first.text);
        let second = run_configured(&first.text, packages, profile, &config);
        assert_eq!(second.text, first.text, "{context}: N(N(x)) != N(x)");
    }
}

fn node_ids_where(ast: &Ast, mut pred: impl FnMut(&Node) -> bool) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut stack = vec![ast.root()];
    while let Some(id) = stack.pop() {
        if pred(ast.node(id)) {
            found.push(id);
        }
        for (child, _) in ast.edges(id).into_iter().rev() {
            stack.push(child);
        }
    }
    found
}

fn command_ids(ast: &Ast, name: &str) -> Vec<NodeId> {
    node_ids_where(
        ast,
        |node| matches!(node, Node::Command { name: found, .. } if found == name),
    )
}

fn declarative_ids(ast: &Ast, name: &str) -> Vec<NodeId> {
    node_ids_where(
        ast,
        |node| matches!(node, Node::Declarative { name: found, .. } if found == name),
    )
}

/// First node under `root` whose own text carries `ch`, in document order.
fn char_node_in(ast: &Ast, root: NodeId, ch: char) -> Option<NodeId> {
    match ast.node(root) {
        Node::Char(found) if *found == ch => return Some(root),
        Node::Text(text) if text.contains(ch) => return Some(root),
        _ => {}
    }
    ast.edges(root)
        .into_iter()
        .find_map(|(child, _)| char_node_in(ast, child, ch))
}

fn subtree_contains_char(ast: &Ast, root: NodeId, ch: char) -> bool {
    char_node_in(ast, root, ch).is_some()
}

/// Declaratives that still govern `ch`, ordered outermost first.
///
/// A declarative applies to the siblings that follow it, so the chain is
/// collected level by level from the character's own sibling list outwards.
/// Groups and prefix-command arguments do not leak their declaratives into the
/// enclosing list, which is the scope rule under test. The last entry is the
/// innermost declarative, the one whose value wins.
fn governing_declaratives(ast: &Ast, ch: char) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut current = char_node_in(ast, ast.root(), ch).unwrap_or_else(|| panic!("missing {ch}"));
    while let Some(link) = ast.parent(current) {
        let edges = ast.edges(link.parent);
        let position = edges
            .iter()
            .position(|(id, _)| *id == current)
            .expect("child should be in its parent");
        let enclosing = edges[..position]
            .iter()
            .filter_map(|(id, _)| match ast.node(*id) {
                Node::Declarative { name, .. } => Some(name.clone()),
                _ => None,
            });
        chain.splice(0..0, enclosing);
        current = link.parent;
    }
    chain
}

/// Assert that `\name` governs every char of `inside` and none of `outside`.
fn assert_scope(ast: &Ast, name: &str, inside: &str, outside: &str, context: &str) {
    for ch in inside.chars() {
        let chain = governing_declaratives(ast, ch);
        assert!(
            chain.iter().any(|found| found == name),
            "{context}: \\{name} should still govern {ch}, governed by {chain:?}"
        );
    }
    for ch in outside.chars() {
        let chain = governing_declaratives(ast, ch);
        assert!(
            !chain.iter().any(|found| found == name),
            "{context}: \\{name} leaked onto {ch}"
        );
    }
}

/// Assert that `\name` is the innermost declarative governing each char, which
/// is what distinguishes a restored outer value from a dropped one.
fn assert_innermost_scope(ast: &Ast, name: &str, chars: &str, context: &str) {
    for ch in chars.chars() {
        assert_eq!(
            governing_declaratives(ast, ch).last().map(String::as_str),
            Some(name),
            "{context}: {ch} should keep \\{name}"
        );
    }
}

fn assert_prefix_scopes(ast: &Ast, command: &str, inside: char, outside: char) {
    let commands = command_ids(ast, command);
    assert_eq!(
        commands.len(),
        1,
        "expected one \\{command}, found {}",
        commands.len()
    );
    assert!(subtree_contains_char(ast, commands[0], inside));
    assert!(!subtree_contains_char(ast, commands[0], outside));
}

/// `true` when any ancestor link of `node` matches `pred`.
fn has_ancestor(ast: &Ast, mut node: NodeId, pred: impl Fn(&Node, Slot) -> bool) -> bool {
    while let Some(link) = ast.parent(node) {
        if pred(ast.node(link.parent), link.slot) {
            return true;
        }
        node = link.parent;
    }
    false
}

#[test]
fn local_declarative_scope_matches_original_on_every_profile() {
    for (source, expected, declarative, inside, outside) in SCOPE_CASES {
        let expected = canonical(SCOPE_PACKAGES, expected);
        for profile in PROFILES {
            let config = profile.default_transform_config();
            let first = run_configured(source, SCOPE_PACKAGES, profile, &config);
            assert_eq!(
                first.text, expected,
                "{profile:?} changed the scope of {source}"
            );
            assert_scope(
                &first.ast,
                declarative,
                &inside.to_string(),
                &outside.to_string(),
                source,
            );
            assert_reparseable(SCOPE_PACKAGES, &first.text);

            let second = run_configured(&first.text, SCOPE_PACKAGES, profile, &config);
            assert_eq!(
                second.text, first.text,
                "{profile:?} {source}: N(N(x)) != N(x)"
            );
            // A third round catches a two-cycle that N(N(x)) alone would miss.
            let third = run_configured(&second.text, SCOPE_PACKAGES, profile, &config);
            assert_eq!(third.text, first.text, "{profile:?} {source}");
        }
    }
}

#[test]
fn prefix_font_still_lowers_and_does_not_cover_the_next_sibling() {
    let outcome = run(r"{\bf x}y");
    assert_eq!(outcome.text, canonical(SCOPE_PACKAGES, r"\mathbf{x}y"));
    assert!(declarative_ids(&outcome.ast, "bf").is_empty());
    assert_prefix_scopes(&outcome.ast, "mathbf", 'x', 'y');

    let overridden = run(r"{\oldstyle \bf 1}2");
    assert_eq!(overridden.text, canonical(SCOPE_PACKAGES, r"\mathbf{1}2"));
    assert_prefix_scopes(&overridden.ast, "mathbf", '1', '2');
}

/// An implicit-style boundary case: `(source, expected, rewrite enabled, slot
/// that must still hold the inner style, char the inner style must govern)`.
type BoundaryCase = (&'static str, &'static str, bool, fn(Slot) -> bool, char);

#[test]
fn explicit_style_survives_implicit_style_boundaries() {
    // Scripts, fraction parts, infix operands, and environment bodies change
    // the implicit style, so an explicit `\scriptstyle` written inside one of
    // them is not redundant and must stay in that slot.
    let boundaries: [BoundaryCase; 5] = [
        (
            r"\scriptstyle x^{\scriptstyle y}",
            r"\scriptstyle x^{\scriptstyle y}",
            true,
            |slot| matches!(slot, Slot::ScriptSup),
            'y',
        ),
        (
            r"\scriptstyle\frac{\scriptstyle 1}{2}",
            r"\scriptstyle\frac{\scriptstyle 1}{2}",
            true,
            |slot| matches!(slot, Slot::Argument(0)),
            '1',
        ),
        (
            r"\scriptstyle {a \over \scriptstyle b}",
            r"\scriptstyle\frac{a}{\scriptstyle b}",
            true,
            |slot| matches!(slot, Slot::Argument(1)),
            'b',
        ),
        (
            r"\scriptstyle {a \over \scriptstyle b}",
            r"\scriptstyle {a \over \scriptstyle b}",
            false,
            |slot| matches!(slot, Slot::InfixRight),
            'b',
        ),
        (
            r"\scriptstyle\begin{matrix}\scriptstyle a\end{matrix}",
            r"\scriptstyle\begin{matrix}\scriptstyle a\end{matrix}",
            true,
            |slot| matches!(slot, Slot::EnvBody),
            'a',
        ),
    ];

    for (source, expected, rewrite, slot, governed) in boundaries {
        let mut config = Profile::Authoring.default_transform_config();
        config.rewrite.enabled = rewrite;
        let outcome = run_configured(source, SCOPE_PACKAGES, Profile::Authoring, &config);
        assert_eq!(
            outcome.text,
            canonical(SCOPE_PACKAGES, expected),
            "{source}"
        );
        let styles = declarative_ids(&outcome.ast, "scriptstyle");
        assert_eq!(styles.len(), 2, "{source}: outer and inner style expected");
        let inner = styles
            .iter()
            .filter(|id| has_ancestor(&outcome.ast, **id, |_, found| slot(found)))
            .count();
        assert_eq!(inner, 1, "{source}: inner style left its slot");
        assert_scope(
            &outcome.ast,
            "scriptstyle",
            &governed.to_string(),
            "",
            source,
        );
    }

    // A style repeated inside the boundary still collapses to one.
    assert_eq!(
        run(r"\scriptstyle x^{\scriptstyle \scriptstyle y}").text,
        canonical(SCOPE_PACKAGES, r"\scriptstyle x^{\scriptstyle y}")
    );
}

#[test]
fn declarative_scope_holds_when_rewrite_or_flatten_is_disabled() {
    // Neither Rewrite nor FlattenGroups creates the scope, so switching them
    // off must not move a declarative. One math style, one text family, and
    // one declarative inside a prefix argument cover the three shapes.
    let representatives = [
        r"{\scriptstyle x} y",
        r"\text{{\cal x} y}",
        r"\mathbf{\scriptstyle x}y",
    ];
    let ablations = [
        ("rewrite disabled", false, true),
        ("flatten disabled", true, false),
        ("rewrite and flatten disabled", false, false),
    ];
    for (label, rewrite_enabled, flatten_enabled) in ablations {
        let mut config = Profile::Authoring.default_transform_config();
        config.rewrite.enabled = rewrite_enabled;
        if !flatten_enabled {
            config.flatten_groups = FlattenGroupsConfig::DISABLED;
        }
        for (source, expected, declarative, inside, outside) in SCOPE_CASES
            .iter()
            .filter(|case| representatives.contains(&case.0))
        {
            let outcome = run_configured(source, SCOPE_PACKAGES, Profile::Authoring, &config);
            assert_eq!(
                outcome.text,
                canonical(SCOPE_PACKAGES, expected),
                "{label} changed {source}"
            );
            assert_scope(
                &outcome.ast,
                declarative,
                &inside.to_string(),
                &outside.to_string(),
                label,
            );
        }
    }
}

#[test]
fn disabling_lower_attributes_leaves_declaratives_unnormalized() {
    for profile in PROFILES {
        let mut config = profile.default_transform_config();
        config.lower_attributes = LowerAttributesConfig::DISABLED;
        let font = run_configured(r"{\bf x}y", SCOPE_PACKAGES, profile, &config);
        assert!(
            font.text.contains(r"\bf"),
            "{profile:?} normalized \\bf without LowerAttributes: {}",
            font.text
        );
        assert!(
            !font.text.contains(r"\mathbf"),
            "{profile:?} emitted \\mathbf without LowerAttributes: {}",
            font.text
        );
        assert_scope(&font.ast, "bf", "x", "y", "lower attributes disabled");

        let style = run_configured(r"{\scriptstyle x} y", SCOPE_PACKAGES, profile, &config);
        assert_eq!(
            style.text,
            canonical(SCOPE_PACKAGES, r"{\scriptstyle x} y"),
            "{profile:?} rewrote the scriptstyle scope without LowerAttributes: {}",
            style.text
        );
        assert_scope(
            &style.ast,
            "scriptstyle",
            "x",
            "y",
            "lower attributes disabled",
        );
    }
}

/// Run LowerAttributes twice over `syntax` and return the first serialization.
fn lower_twice(syntax: &SyntaxNode) -> (Ast, String) {
    let mut ast = Ast::from_syntax_root(syntax);
    let mut recorder = texform_transform::report::ReportRecorder::disabled();
    let mut run_once = |ast: &mut Ast| {
        texform_transform::lower_attributes::run(
            ast,
            &LowerAttributesConfig::ENABLED,
            &mut recorder,
        );
    };
    run_once(&mut ast);
    ast.assert_invariants();
    let once = serialize(&ast);
    run_once(&mut ast);
    assert_eq!(serialize(&ast), once, "second lowering changed the output");
    (ast, once)
}

fn scoped_group(mode: ContentMode, declarative: &str, body: &str, tail: SyntaxNode) -> SyntaxNode {
    SyntaxNode::Root {
        mode,
        children: vec![
            SyntaxNode::Group {
                mode,
                kind: GroupKind::Explicit,
                children: vec![
                    SyntaxNode::Declarative {
                        name: declarative.to_string(),
                        args: Vec::new(),
                    },
                    SyntaxNode::Text(body.to_string()),
                ],
            },
            tail,
        ],
    }
}

#[test]
fn math_whitespace_split_keeps_declarative_scope_inside_the_group() {
    // Splitting a math text run around its spaces must not push content past
    // the declarative's own group boundary.
    let (ast, once) = lower_twice(&scoped_group(
        ContentMode::Math,
        "scriptstyle",
        " x ",
        SyntaxNode::Char('y'),
    ));
    assert_scope(&ast, "scriptstyle", "x", "y", "math whitespace split");
    // Everything the split produced stays before the closing brace; only the
    // serializer's own separator sits between the group and `y`.
    assert_eq!(
        once.rsplit_once('}').expect("group braces").1,
        " y",
        "content leaked past the style group: {once}"
    );
}

#[test]
fn text_mode_scope_does_not_use_math_whitespace_splitting() {
    // Text runs carry their own spacing, so the text path keeps the run intact
    // instead of reusing the math split.
    let (ast, once) = lower_twice(&scoped_group(
        ContentMode::Text,
        "cal",
        "x y",
        SyntaxNode::Text(" z".to_string()),
    ));
    assert_scope(&ast, "cal", "xy", "z", "text mode scope");
    // The run keeps its single interior space; no split was applied.
    assert_eq!(once, r"{\cal x y} z");
}

#[test]
fn explicit_restore_to_outer_value_is_not_dropped_inside_the_group() {
    // Restating the outer value inside a group looks redundant but is not:
    // dropping it would leave the inner value governing the rest of the group.
    // Each case lists the chars whose innermost declarative must be the named
    // one.
    let cases: [(&str, &[(&str, &str)]); 6] = [
        (
            r"\scriptstyle {\textstyle x\scriptstyle y}z",
            &[("yz", "scriptstyle"), ("x", "textstyle")],
        ),
        (
            r"\large {\small x\large y}z",
            &[("yz", "large"), ("x", "small")],
        ),
        (
            r"\text{\cal {\mit x\cal y}z}",
            &[("yz", "cal"), ("x", "mit")],
        ),
        (
            r"\text{\large {\small x\large y}z}",
            &[("yz", "large"), ("x", "small")],
        ),
        (
            r"\scriptstyle {\textstyle x\scriptstyle y\textstyle z\scriptstyle w}v",
            &[("ywv", "scriptstyle"), ("xz", "textstyle")],
        ),
        (
            r"\scriptstyle {\textstyle {\scriptstyle x} y}z",
            &[("xz", "scriptstyle"), ("y", "textstyle")],
        ),
    ];
    for (source, expectations) in cases {
        let expected = canonical(SCOPE_PACKAGES, source);
        for_each_profile(source, SCOPE_PACKAGES, &expected, |ast, context| {
            for (chars, declarative) in expectations {
                assert_innermost_scope(ast, declarative, chars, context);
            }
        });
    }
}

#[test]
fn prefix_switch_still_canonicalizes_when_value_returns() {
    for_each_profile(
        r"{\bf {\rm x\bf y}z}",
        SCOPE_PACKAGES,
        r"\mathrm { x } \mathbf { y z }",
        |ast, _| {
            assert_prefix_scopes(ast, "mathrm", 'x', 'y');
            assert_prefix_scopes(ast, "mathrm", 'x', 'z');
            let bold = command_ids(ast, "mathbf");
            assert_eq!(bold.len(), 1);
            assert!(subtree_contains_char(ast, bold[0], 'y'));
            assert!(subtree_contains_char(ast, bold[0], 'z'));
            assert!(!subtree_contains_char(ast, bold[0], 'x'));
        },
    );

    for_each_profile(
        r"\text{\textbf{\textrm{x}\textbf{y}}z}",
        SCOPE_PACKAGES,
        r"\text {\textrm{\textbf{x}}\textbf{y}z}",
        |ast, _| {
            assert_prefix_scopes(ast, "textrm", 'x', 'y');
            assert_prefix_scopes(ast, "textrm", 'x', 'z');
            let bold = command_ids(ast, "textbf");
            assert!(bold.iter().any(|id| subtree_contains_char(ast, *id, 'y')));
            assert!(bold.iter().all(|id| !subtree_contains_char(ast, *id, 'z')));
        },
    );
}

#[test]
fn serializer_lexical_boundaries_survive_every_profile() {
    // The transform pipeline reserializes its result, so the serializer's
    // lexical boundary rules must survive every profile as well.
    let packages = ["base", "ams", "textmacros"];
    for source in [
        r"\mbox{mod\ 1}",
        r"\mbox{mod\  1}",
        r"\text{a\,b}",
        r"\text{a\%b}",
        r"\operatorname*{x}y",
    ] {
        let expected = canonical(&packages, source);
        for_each_profile(source, &packages, &expected, |_, _| {});
    }
}

#[test]
fn math_font_prefix_keeps_sibling_size_outside_the_wrapper() {
    // A math font declarative becomes a prefix wrapper, so a size declarative
    // governing the same run has to be repeated inside each wrapper without
    // reaching content that followed the original group.
    let cases = [
        (
            r"\bf\large x\rm y",
            r"\mathbf { \large x } \mathrm { \large y }",
            "xy",
            "",
        ),
        (
            r"{\bf\large x\rm y}z",
            r"{ \mathbf { \large x } \mathrm { \large y } } z",
            "xy",
            "z",
        ),
        (
            r"\bf\large x{\rm y}z",
            r"\mathbf { \large x } \mathrm { \large y } \mathbf { \large z }",
            "xyz",
            "",
        ),
        (
            r"\text{\bf\large x\rm y}",
            r"\text {\large\textbf{ x}\textrm{\textbf{ y}}}",
            "xy",
            "",
        ),
        (
            r"\text{{\bf\large x\rm y}z}",
            r"\text {{\large\textbf{ x}\textrm{\textbf{ y}}}z}",
            "xy",
            "z",
        ),
    ];
    for (source, expected, covered, uncovered) in cases {
        for_each_profile(source, SCOPE_PACKAGES, expected, |ast, context| {
            assert_scope(ast, "large", covered, uncovered, context);
        });
    }

    for_each_profile(
        r"\bf\large x\rm y",
        SCOPE_PACKAGES,
        r"\mathbf { \large x } \mathrm { \large y }",
        |ast, _| {
            assert_prefix_scopes(ast, "mathbf", 'x', 'y');
            assert_prefix_scopes(ast, "mathrm", 'y', 'x');
        },
    );

    // Style has no prefix form, so it stays a declarative beside the wrappers
    // while still governing both runs.
    for_each_profile(
        r"\bf\large\scriptstyle x\rm y",
        SCOPE_PACKAGES,
        r"\scriptstyle \large \mathbf { x } \mathrm { y }",
        |ast, context| {
            assert_scope(ast, "large", "xy", "", context);
            assert_scope(ast, "scriptstyle", "xy", "", context);
            assert_prefix_scopes(ast, "mathbf", 'x', 'y');
            assert_prefix_scopes(ast, "mathrm", 'y', 'x');
        },
    );
}
