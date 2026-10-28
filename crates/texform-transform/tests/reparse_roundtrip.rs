//! Fast round-trip tests: the transformed-tree round trip must hold for every
//! input here, under three package sets, four profiles, and both output
//! styles. See `texform_transform::round_trip` for the checks.
//!
//! Any failure that is not listed in [`PENDING`], including an unexpected
//! transform error, fails the test. The attribution test checks that steps
//! promising identical rendering never rely on boundary separation.
#[path = "support/phase_cases.rs"]
mod phase_cases;

use std::panic::{AssertUnwindSafe, catch_unwind};

use texform_core::{
    parse::ParseContext,
    serialize::{Style, serialize, serialize_with},
};
use texform_transform::round_trip::{
    self, Check, Outcome, PACKAGE_SETS, PROFILES, STYLES, adversarial, parse,
};
use texform_transform::{
    BuildConfig, FinalizeAstConfig, FlattenGroupsConfig, LowerAttributesConfig, Profile,
    RewriteConfig, RuleFidelity, TransformConfig, TransformContext,
};

struct Lane {
    name: &'static str,
    parse_ctx: ParseContext,
    engines: Vec<TransformContext>,
}

fn lane(name: &str) -> Lane {
    let (name, packages) = PACKAGE_SETS
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .expect("named package set");
    let parse_ctx = ParseContext::from_packages(packages);
    let engines = round_trip::engines(&parse_ctx);
    Lane {
        name,
        parse_ctx,
        engines,
    }
}

fn lanes() -> Vec<Lane> {
    PACKAGE_SETS.iter().map(|(name, _)| lane(name)).collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Check(Check),
    TransformFailure,
    Attribution,
}

/// A known failure outside the scope of the change that introduced it.
///
/// Each entry names one input, package set, profile, style, and check, so
/// that every other combination of the same input is still checked. The
/// list only shrinks: `pending_entries_still_fail` fails once an entry is
/// fixed, so that the main tests take it over.
struct Pending {
    input: &'static str,
    packages: &'static str,
    /// `None` for `ParserRoundTrip`, which does not depend on the profile.
    profile: Option<Profile>,
    /// `None` for `Retransform`, transform failures, and attribution.
    style: Option<Style>,
    item: Item,
    reason: &'static str,
}

const PENDING: &[Pending] = &[];

fn pending(
    input: &str,
    packages: &str,
    profile: Option<Profile>,
    style: Option<Style>,
    item: Item,
) -> bool {
    PENDING.iter().any(|entry| {
        entry.input == input
            && entry.packages == packages
            && entry.profile == profile
            && entry.style == style
            && entry.item == item
    })
}

/// Every failure of `source` under `lane`, as `(item, profile, style, message)`.
fn all_failures(lane: &Lane, source: &str) -> Vec<(Item, Option<Profile>, Option<Style>, String)> {
    let p = &lane.parse_ctx;
    let Some(original) = parse(p, source) else {
        return Vec::new();
    };
    let mut failures = Vec::new();
    for style in STYLES {
        if let Some(failure) = round_trip::check_parser(p, &original, style) {
            failures.push((
                Item::Check(Check::ParserRoundTrip),
                None,
                Some(style),
                failure.to_string(),
            ));
        }
    }
    for (profile, engine) in PROFILES.into_iter().zip(&lane.engines) {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            round_trip::check_transform(p, engine, &original)
        }))
        .unwrap_or_else(|_| Outcome::TransformFailed("panic".into()));
        match outcome {
            Outcome::TransformFailed(error) => failures.push((
                Item::TransformFailure,
                Some(profile),
                None,
                format!("transform failure: {error}"),
            )),
            Outcome::Checked(checked) => failures.extend(checked.into_iter().map(|failure| {
                (
                    Item::Check(failure.check),
                    Some(profile),
                    failure.style,
                    failure.to_string(),
                )
            })),
        }
    }
    failures
}

/// Failures of `source` that are not pending, formatted for a test message.
fn unlisted_failures(lane: &Lane, source: &str) -> Vec<String> {
    all_failures(lane, source)
        .into_iter()
        .filter(|(item, profile, style, _)| !pending(source, lane.name, *profile, *style, *item))
        .map(|(_, profile, _, message)| {
            format!(
                "[{} {}] {source}: {message}",
                lane.name,
                profile.map_or("parser", round_trip::profile_name)
            )
        })
        .collect()
}

fn assert_round_trips<'a>(lane: &Lane, inputs: impl IntoIterator<Item = &'a str>) {
    let failures: Vec<String> = inputs
        .into_iter()
        .flat_map(|source| unlisted_failures(lane, source))
        .collect();
    assert!(
        failures.is_empty(),
        "{} round-trip failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn generated_inputs_round_trip() {
    let mut inputs = phase_cases::combinations();
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
            r"\root\of{x}",
            r"\root{1+2}\of x",
            r"\root 3\of{x}^2",
            r"\root{a\of b}\of x",
            r"\frac{\root n_i\of{x}}{y}",
            r"A\buildrel f\over=B",
            r"a\over\buildrel b\over c",
            r"\buildrel{a\over b}\over=",
            r"\buildrel x\over =^2",
            r"\frac{\buildrel a\over =}{b}",
        ]
        .map(str::to_owned),
    );
    for lane in lanes() {
        // `\vb` is the physics spelling; without physics it is unknown.
        let inputs: Vec<String> = if lane.name == "default" {
            inputs
                .iter()
                .map(|s| s.replace(r"\vb", r"\mathbf"))
                .collect()
        } else {
            inputs.clone()
        };
        assert_round_trips(&lane, inputs.iter().map(String::as_str));
    }
}

#[test]
fn design_examples_reach_the_expected_output_in_one_run() {
    let default = lane("default");
    for engine in &default.engines {
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
            let mut ast = parse(&default.parse_ctx, source).unwrap();
            engine.run(&mut ast, &default.parse_ctx).unwrap();
            assert_eq!(serialize(&ast), expected, "{source}");
        }
    }
    let mixed = lane("mixed");
    for engine in &mixed.engines {
        let mut ast = parse(&mixed.parse_ctx, r"\qty[{]}]").unwrap();
        engine.run(&mut ast, &mixed.parse_ctx).unwrap();
        assert_eq!(serialize(&ast), r"\qty [ { ] } ]");
    }
}

#[test]
fn handwritten_inputs_round_trip() {
    let inputs = [
        // Rewritten nodes match what the parser builds.
        r"\Biggr\}",
        r"A\hskip 1em B",
        r"\text{A\hskip 1em B}",
        r"\Bigl\{",
        r"\newline",
        r"\break",
        r"\break'",
        r"\break_1^2",
        r"\frac{\break_1}{x}",
        r"\newline'",
        r"{\cr}_1",
        r"{\newline}_1",
        r"{\\}_1",
        r"\begin{matrix}{\cr}_1\end{matrix}",
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
        // Equation numbers keep math content inside text-mode tags.
        r"\eqalignno{x&=y&(n_i)\cr}",
        r"\eqalignno{x&=y&{\frac{n}{m}}\cr\cr z&=w&(1.2)}",
        // Plain alignment row separators keep their `*` and spacing.
        r"\matrix{a&b\\*[2pt]c&d\\[2pt]}",
        r"\eqalignno{x&=y&(1)\\[2pt]z&=w&(2)\newline}",
        // Lowering consumes argument containers in one run.
        r"\mathbf{{{\rm{ab}}}}",
        r"{\rm{ }}",
        r"T_{\rm{eff}}",
    ];
    for lane in lanes() {
        assert_round_trips(&lane, inputs);
    }
}

#[test]
fn physics_inputs_round_trip() {
    let inputs = [
        // Each script command has an omitted slot that the sibling would fill.
        r"e^{\grad}(f)",
        r"x^{\qand}*",
        r"x^{\sin}[2]",
        r"x^{\Re}{z}",
        // Fences keep unpaired opening delimiters inside.
        r"\qty(a{\big(}b)",
        r"\qty[a{\big[}b]",
        r"\qty({a(b})",
        r"\qty[{a[b}]",
        r"\qty(a{\big(}b{\big)}c)",
        r"\qty|a{\big|}b|",
    ];
    let mut derivatives = Vec::new();
    for command in ["dv", "pdv", "fdv"] {
        for variable in ["x^2", "x_0", "x'", "x_0^2", "{x^2}"] {
            derivatives.push(format!(r"\{command}[2]{{f}}{{{variable}}}"));
        }
    }
    for name in ["physics", "mixed"] {
        let lane = lane(name);
        for source in &derivatives {
            assert!(parse(&lane.parse_ctx, source).is_some(), "{source}");
        }
        assert_round_trips(
            &lane,
            inputs
                .into_iter()
                .chain(derivatives.iter().map(String::as_str)),
        );
    }
}

#[test]
fn physics_fences_protect_unpaired_opening_delimiters_after_transforms() {
    let lane = lane("mixed");
    // Dropping `\big` and flattening its brace group leaves a bare opener
    // that would otherwise consume the fence's closing delimiter.
    let equiv = &lane.engines[3];
    for (source, expected) in [
        (r"\qty(a{\big(}b)", r"\qty({a(b})"),
        (r"\qty[a{\big[}b]", r"\qty[{a[b}]"),
    ] {
        let mut ast = parse(&lane.parse_ctx, source).unwrap();
        equiv.run(&mut ast, &lane.parse_ctx).unwrap();
        assert_eq!(
            serialize_with(&ast, &Style::Compact.into()),
            expected,
            "{source}"
        );
    }
}

/// Inputs where deleting or replacing a node exposes a boundary hazard.
const BOUNDARY_EXAMPLES: &[&str] = &[
    // Command capture.
    r"\ln\badbreak[2]x",
    r"\cos\allowbreak[n]x",
    r"\sin\mathstrut[2]x",
    r"\sin\uproot{2}[2]x",
    r"\sin\quad[2]x",
    r"\log\enspace[2](n)",
    r"\sin\nobreak(x)",
    r"\sin\quad(x)",
    r"\dd\quad[x",
    r"\qty\enspace|x|",
    r"\mqty\strut(a)",
    r"\smqty\ast",
    r"\mqty\ast",
    r"\curl\div\big[a]",
    r"\grad\Large'^2",
    r"\curl\grad\div\big[a]",
    r"\rm\curl\div\big[a]",
    r"\sine\nobreak(x)",
    r"\ln\big[2\big]x",
    // Environment capture.
    r"\begin{aligned}\big[a]\end{aligned}",
    r"\begin{aligned}\quad[a]\end{aligned}",
    r"\begin{aligned}\mathstrut[a]\end{aligned}",
    r"\begin{aligned}\big[a\end{aligned}",
    r"\begin{gathered}\quad[a]\end{gathered}",
    // Text-mode space swallowing.
    r"\text{{\ldots} and}",
    r"\mbox{\S{} b}",
    r"\text{\S\mathstrut b}",
    r"\text{\ldots\quad{} and}",
    r"\text{\AA\textbf{} and}",
];

#[test]
fn boundary_examples_round_trip() {
    for lane in lanes() {
        assert_round_trips(&lane, BOUNDARY_EXAMPLES.iter().copied());
    }
}

#[test]
fn adversarial_boundary_subset_round_trips() {
    for lane in lanes() {
        let inputs = adversarial::stratified(&lane.parse_ctx);
        assert!(!inputs.is_empty());
        assert_round_trips(&lane, inputs.iter().map(|input| input.source.as_str()));
    }
}

/// Steps that promise identical rendering must not rely on boundary
/// separation: a Render rule run alone or followed by LowerAttributes,
/// LowerAttributes, and FlattenGroups in strict mode must leave no hazard for
/// the separation step to remove.
#[test]
fn render_steps_do_not_rely_on_boundary_separation() {
    let mut failures = Vec::new();
    for lane in lanes() {
        let p = &lane.parse_ctx;
        let inputs = adversarial::stratified(p);
        let only = |lower_attributes, rewrite, flatten_groups| TransformConfig {
            lower_attributes,
            rewrite,
            finalize_ast: FinalizeAstConfig::DISABLED,
            flatten_groups,
        };
        let mut check = |label: String,
                         engine: &TransformContext,
                         config: &TransformConfig,
                         profile,
                         source: &str| {
            let Some(mut ast) = parse(p, source) else {
                return;
            };
            let separated = catch_unwind(AssertUnwindSafe(|| {
                engine.run_counting_separations(&mut ast, p, config)
            }));
            let problem = match separated {
                // Transform errors are the fast tests' concern.
                Ok(Ok(0) | Err(_)) => return,
                Ok(Ok(count)) => format!("relies on {count} separation(s)"),
                Err(_) => "panicked".to_string(),
            };
            if !pending(source, lane.name, Some(profile), None, Item::Attribution) {
                failures.push(format!("[{} {label}] {source}: {problem}", lane.name));
            }
        };

        // Each Render rule, alone, on the inputs whose separator it handles.
        for profile in PROFILES {
            let engine =
                TransformContext::from_build_config(BuildConfig::profile(profile), p).unwrap();
            for rule in engine.rewrite_plan().rules() {
                let meta = rule.meta();
                if meta.fidelity != RuleFidelity::Render {
                    continue;
                }
                let key = meta.key.to_string();
                let Ok(single) = TransformContext::from_build_config(
                    BuildConfig::profile(profile).only_rule_for_tests(meta.key),
                    p,
                ) else {
                    continue;
                };
                let config = only(
                    LowerAttributesConfig::DISABLED,
                    RewriteConfig::DEFAULT,
                    FlattenGroupsConfig::DISABLED,
                );
                // Where LowerAttributes also preserves rendering, the rule's
                // output must survive it, as `\vb{}` becoming an empty
                // `\mathbf{}` that LowerAttributes would remove.
                let with_lower = only(
                    LowerAttributesConfig::ENABLED,
                    RewriteConfig::DEFAULT,
                    FlattenGroupsConfig::DISABLED,
                );
                let preserves = matches!(profile, Profile::Authoring | Profile::Faithful);
                for input in inputs.iter().filter(|input| input.sources.contains(&key)) {
                    check(key.clone(), &single, &config, profile, &input.source);
                    if preserves {
                        let label = format!("{key} + lower-attributes");
                        check(label, &single, &with_lower, profile, &input.source);
                    }
                }
            }
        }

        // LowerAttributes where it preserves rendering, and strict FlattenGroups.
        let lower = |input: &&adversarial::Input| {
            input
                .sources
                .iter()
                .any(|source| adversarial::is_lower_attributes(source))
        };
        for profile in [Profile::Authoring, Profile::Faithful] {
            let engine =
                TransformContext::from_build_config(BuildConfig::profile(profile), p).unwrap();
            let config = only(
                LowerAttributesConfig::ENABLED,
                RewriteConfig {
                    enabled: false,
                    ..RewriteConfig::DEFAULT
                },
                FlattenGroupsConfig::DISABLED,
            );
            for input in inputs.iter().filter(lower) {
                check(
                    "lower-attributes".into(),
                    &engine,
                    &config,
                    profile,
                    &input.source,
                );
            }
            let config = only(
                LowerAttributesConfig::DISABLED,
                RewriteConfig {
                    enabled: false,
                    ..RewriteConfig::DEFAULT
                },
                FlattenGroupsConfig::STRICT,
            );
            for input in &inputs {
                check(
                    "flatten-groups".into(),
                    &engine,
                    &config,
                    profile,
                    &input.source,
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} attribution failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "run to find fixed entries in PENDING"]
fn pending_entries_still_fail() {
    let lanes = lanes();
    let mut fixed = Vec::new();
    for entry in PENDING {
        assert!(
            !entry.reason.trim().is_empty(),
            "{}: empty reason",
            entry.input
        );
        let lane = lanes
            .iter()
            .find(|lane| lane.name == entry.packages)
            .expect("pending entry names a package set");
        let still_fails =
            match entry.item {
                Item::Attribution => attribution_fails(lane, entry),
                item => all_failures(lane, entry.input).into_iter().any(
                    |(failed, profile, style, _)| {
                        failed == item && profile == entry.profile && style == entry.style
                    },
                ),
            };
        if !still_fails {
            fixed.push(format!(
                "{} {:?} {:?} {:?}: {}",
                entry.packages, entry.profile, entry.style, entry.item, entry.input
            ));
        }
    }
    assert!(
        fixed.is_empty(),
        "fixed entries to remove from PENDING:\n{}",
        fixed.join("\n")
    );
}

/// Whether the full profile pipeline separates a hazard in `entry.input`;
/// attribution entries record steps that still rely on separation there.
fn attribution_fails(lane: &Lane, entry: &Pending) -> bool {
    let profile = entry.profile.expect("attribution entries name a profile");
    let engine =
        TransformContext::from_build_config(BuildConfig::profile(profile), &lane.parse_ctx)
            .unwrap();
    let Some(mut ast) = parse(&lane.parse_ctx, entry.input) else {
        return false;
    };
    matches!(
        engine.run_counting_separations(&mut ast, &lane.parse_ctx, engine.default_config()),
        Ok(count) if count > 0
    )
}

/// Counts failures on a prepared UTF-8 corpus, one formula per line or one
/// JSON string per line for `.jsonl`. Like the round-trip report, this is a
/// measurement: it prints counts and never fails on them.
#[test]
#[ignore = "set TEXFORM_ROUNDTRIP_CORPUS to a complete single-line corpus file"]
fn corpus_round_trip_counts() {
    use std::io::BufRead;
    let path = std::env::var("TEXFORM_ROUNDTRIP_CORPUS").expect("corpus path required");
    let lanes = lanes();
    let lines: Vec<String> = std::io::BufReader::new(std::fs::File::open(&path).unwrap())
        .lines()
        .map(|line| {
            let line = line.unwrap();
            if path.ends_with(".jsonl") {
                serde_json::from_str::<String>(&line).unwrap()
            } else {
                line
            }
        })
        .collect();
    assert!(!lines.is_empty());
    for lane in &lanes {
        let mut incomplete = 0;
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for source in &lines {
            if parse(&lane.parse_ctx, source).is_none() {
                incomplete += 1;
                continue;
            }
            for (item, profile, style, message) in all_failures(lane, source) {
                let key = format!(
                    "{} {} {:?}",
                    profile.map_or("parser", round_trip::profile_name),
                    style.map_or("-", round_trip::style_name),
                    item
                );
                *counts.entry(key).or_default() += 1;
                eprintln!("[{}] {source}: {message}", lane.name);
            }
        }
        println!(
            "{}: {} inputs, {incomplete} incomplete, failures {counts:?}",
            lane.name,
            lines.len()
        );
    }
}
