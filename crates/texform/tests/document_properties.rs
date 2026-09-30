//! Generated construction and editing contracts through the stable facade.
use proptest::prelude::*;
use std::sync::OnceLock;
use texform::{
    Arg, ArgKindRef, ContentMode, DelimiterRef, DelimiterValue, Document, EditError, KnowledgeBase,
    NodeId, NodeKind, Parser, Profile, TransformEngine,
};

fn kb() -> &'static KnowledgeBase {
    static KB: OnceLock<KnowledgeBase> = OnceLock::new();
    KB.get_or_init(|| {
        KnowledgeBase::builder()
            .packages(&["base", "ams", "physics", "textmacros"])
            .build()
            .unwrap()
    })
}

fn parser() -> Parser {
    Parser::builder().knowledge_base(kb().clone()).build()
}

fn assert_transformable(document: &Document) {
    static ENGINES: OnceLock<Vec<TransformEngine>> = OnceLock::new();
    let engines = ENGINES.get_or_init(|| {
        [
            Profile::Authoring,
            Profile::Faithful,
            Profile::Corpus,
            Profile::Equiv,
        ]
        .into_iter()
        .map(|profile| {
            TransformEngine::builder()
                .knowledge_base(kb().clone())
                .profile(profile)
                .build()
                .unwrap()
        })
        .collect()
    });
    document.__validate_conformance().unwrap();
    for engine in engines {
        let mut copy = document.clone();
        engine.transform(&mut copy).unwrap();
        copy.__validate_conformance().unwrap();
    }
}

fn ids(document: &Document) -> Vec<NodeId> {
    std::iter::once(document.root().id())
        .chain(document.root().descendants().map(|n| n.id()))
        .collect()
}

// Include identity and parent links, which a syntax-only rollback check cannot observe.
fn observe(document: &Document, roots: &[NodeId]) -> String {
    let mut result = String::new();
    for &id in roots {
        match document.node(id) {
            Err(error) => result.push_str(&format!("{id:?}:{error:?};")),
            Ok(node) => {
                for node in std::iter::once(node).chain(node.descendants()) {
                    result.push_str(&format!(
                        "{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?};",
                        node.id(),
                        node.parent().map(|n| n.id()),
                        node.kind(),
                        node.char(),
                        node.text(),
                        node.command_name(),
                        node.env_name(),
                        node.group_kind(),
                        node.prime_count(),
                        node.arg_slots().collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    result
}

fn leaf() -> impl Strategy<Value = char> {
    prop_oneof![
        proptest::char::range('a', 'z'),
        proptest::char::range('0', '9'),
        proptest::sample::select(vec![
            '+', '-', '=', 'α', '中', '%', '{', '}', '$', '_', '^', '\\', '\u{a0}', '\u{2019}'
        ])
    ]
}

proptest! {
    #[test]
    fn public_construction_stays_conformant(
        text_mode in any::<bool>(),
        steps in prop::collection::vec((0u8..14, leaf(), 0usize..20), 1..40),
    ) {
        let mode = if text_mode { ContentMode::Text } else { ContentMode::Math };
        let mut document = Document::with_knowledge_base(kb(), mode);
        let mut detached = Vec::new();
        for (operation, ch, index) in steps {
            let before = document.to_syntax();
            let old_nodes = observe(&document, &detached);
            let source = ch.to_string();
            let input = detached.get(index % detached.len().max(1)).copied().map(Arg::Node).unwrap_or_else(|| Arg::Source(source.clone()));
            let result = match operation {
                0 => document.create_char(ch),
                1 => document.create_text(source),
                2 => document.create_prime(index % 4),
                3 => document.create_active_space(),
                4 => document.create_group(mode, [input]),
                5 => document.parse_fragment(&source, Some(mode)),
                6 => document.create_inline_math([input]),
                7 => document.create_delimited_group(DelimiterValue::Char('('), DelimiterValue::Char(')'), [input]),
                8 => document.create_scripted(input, Some(Arg::Source("i".into())), Some(Arg::Source("2".into()))),
                9 => document.create_command("frac", [input, Arg::Source("y".into())]),
                10 => document.create_command("sqrt", [Arg::Absent, input]),
                11 => document.create_command("text", [input]),
                12 => document.create_environment_with_children("matrix", [], vec![input]),
                _ => document.create_infix("over", input, Arg::Source("z".into()), []),
            };
            match result {
                Ok(node) => {
                    detached.retain(|id| document.node(*id).is_ok_and(|n| n.parent().is_none()));
                    detached.push(node);
                    // Keep some roots detached so later constructors compose deeper trees.
                    if index % 3 == 0 {
                        document.__validate_conformance().unwrap();
                        continue;
                    }
                    let syntax = document.to_syntax();
                    let candidate = observe(&document, &detached);
                    if document.append_child(document.root().id(), node).is_err() {
                        prop_assert_eq!(document.to_syntax(), syntax);
                        prop_assert_eq!(observe(&document, &detached), candidate);
                    } else {
                        detached.retain(|id| *id != node);
                    }
                }
                Err(_) => {
                    prop_assert_eq!(document.to_syntax(), before);
                    prop_assert_eq!(observe(&document, &detached), old_nodes);
                }
            }
            document.__validate_conformance().unwrap();
        }
        assert_transformable(&document);
        // This is a serialization probe, not a promise of structural round trips.
        let latex = document.to_latex().unwrap();
        let probe = if text_mode { format!(r"\text{{{latex}}}") } else { latex.clone() };
        let parsed = parser().parse(&probe);
        prop_assert!(!parsed.has_errors(), "serialization probe: {latex:?}");
    }

    #[test]
    fn regression_edits_preserve_conformance_and_roll_back_failures(
        fixture in 0usize..(REGRESSION.len() + STRUCTURAL_FIXTURES.len()),
        steps in prop::collection::vec((0u8..20, any::<usize>(), leaf(), 0usize..5), 1..35),
    ) {
        let (fixture_id, source) = REGRESSION.iter().chain(STRUCTURAL_FIXTURES).copied().nth(fixture).unwrap();
        let mut document = parser().parse(source).try_into_document().unwrap().0;
        for (operation, selector, ch, index) in steps {
            let reachable = ids(&document);
            // Half the operations target an appropriate kind; the other half also exercise wrong-kind errors.
            let compatible: Vec<_> = reachable.iter().copied().filter(|id| {
                let node = document.node(*id).unwrap();
                match operation {
                    0 | 3 | 9 => matches!(node.kind(), NodeKind::Root | NodeKind::Group),
                    6 => node.kind() == NodeKind::Group,
                    10 => node.kind() == NodeKind::Char,
                    11 => node.kind() == NodeKind::Text,
                    12 => node.command_name().is_some(),
                    13 => node.env_name().is_some(),
                    14 => node.arg_count() > 0,
                    17 => matches!(node.group_kind(), Some(texform::GroupKindRef::Delimited { .. })),
                    18 => matches!(node.arg_kind(0), Some(ArgKindRef::Paired { .. })),
                    19 => node.kind() == NodeKind::Prime,
                    _ => node.id() != document.root().id(),
                }
            }).collect();
            let typed = selector % 2 == 0 && !compatible.is_empty();
            let targets = if typed { &compatible } else { &reachable };
            let target = targets[(selector / 2) % targets.len()];
            let index = if typed && matches!(operation, 14 | 18) { 0 } else { index };
            let children = if operation == 5 { vec![] } else { vec![Arg::Source("z+1".into())] };
            let candidate = document.create_group(ContentMode::Math, children).unwrap();
            let syntax = document.to_syntax();
            let identities = ids(&document);
            let state = observe(&document, &[document.root().id(), candidate]);
            let result: Result<(), EditError> = match operation {
                0 => document.append_child(target, candidate),
                1 => document.insert_before(target, candidate),
                2 => document.insert_after(target, candidate),
                3 => document.insert_child(target, index, candidate),
                4 => document.replace_with(target, candidate),
                5 => document.wrap(target, candidate).map(|_| ()),
                6 => document.unwrap(target).map(|_| ()),
                7 => document.extract(target).map(|_| ()),
                8 => document.remove(target),
                9 => document.clear(target),
                10 => document.set_char(target, ch),
                11 => document.set_text(target, ch.to_string()),
                12 => document.set_command_name(target, if index % 2 == 0 { "frac" } else { "alpha" }),
                13 => document.set_env_name(target, if index % 2 == 0 { "matrix" } else { "array" }),
                14 => document.set_arg(target, index, Arg::Node(candidate)),
                15 => document.set_subscript(target, Some(Arg::Node(candidate))).map(|_| ()),
                16 => document.set_superscript(target, if index % 2 == 0 { None } else { Some(Arg::Node(candidate)) }).map(|_| ()),
                17 => document.set_delimiters(target, "(", ")"),
                18 => document.set_arg_delimiters(target, index, "[", "]"),
                _ => document.set_prime_count(target, index),
            };
            if result.is_err() {
                prop_assert_eq!(document.to_syntax(), syntax, "fixture {}", fixture_id);
                prop_assert_eq!(ids(&document), identities, "fixture {}", fixture_id);
                prop_assert_eq!(observe(&document, &[document.root().id(), candidate]), state, "fixture {}", fixture_id);
            }
            document.__validate_conformance().unwrap();
        }
        assert_transformable(&document);
    }
}

fn boundaries(document: &Document, command: NodeId) -> (char, char) {
    match document.node(command).unwrap().arg_kind(0).unwrap() {
        ArgKindRef::Paired {
            open: DelimiterRef::Char(open),
            close: DelimiterRef::Char(close),
        } => (open, close),
        other => panic!("expected character boundaries, got {other:?}"),
    }
}

#[test]
fn paired_candidates_round_trip_and_edit_boundaries_independently() {
    let pairs = [('(', ')'), ('[', ']'), ('{', '}'), ('|', '|')];
    for (open, close) in pairs {
        let mut document = Document::with_knowledge_base(kb(), ContentMode::Math);
        let command = document
            .create_command(
                "qty",
                [Arg::paired("x", open.to_string(), close.to_string()).unwrap()],
            )
            .unwrap();
        document
            .append_child(document.root().id(), command)
            .unwrap();
        assert_eq!(boundaries(&document, command), (open, close));
        let latex = document.to_latex().unwrap();
        let reparsed = parser().parse(&latex).try_into_document().unwrap().0;
        assert_eq!(
            boundaries(
                &reparsed,
                reparsed.find_commands("qty").next().unwrap().id()
            ),
            (open, close)
        );
        document
            .set_arg(command, 0, Arg::Source("y+z".into()))
            .unwrap();
        assert_eq!(boundaries(&document, command), (open, close));
        let content = document
            .node(command)
            .unwrap()
            .arg(0)
            .unwrap()
            .as_node()
            .unwrap()
            .id();
        let content_state = observe(&document, &[content]);
        for (new_open, new_close) in pairs {
            document
                .set_arg_delimiters(command, 0, new_open.to_string(), new_close.to_string())
                .unwrap();
            assert_eq!(boundaries(&document, command), (new_open, new_close));
            assert_eq!(observe(&document, &[content]), content_state);
            document.__validate_conformance().unwrap();
        }
        for (bad_open, bad_close) in [("(", "]"), (".", "."), ("\\langle", "\\rangle")] {
            let before = document.to_syntax();
            let state = observe(&document, &[document.root().id()]);
            assert!(
                document
                    .set_arg_delimiters(command, 0, bad_open, bad_close)
                    .is_err()
            );
            assert!(
                document
                    .create_command("qty", [Arg::paired("z", bad_open, bad_close).unwrap()])
                    .is_err()
            );
            assert_eq!(document.to_syntax(), before);
            assert_eq!(observe(&document, &[document.root().id()]), state);
        }
        assert_transformable(&document);
    }
}

// Exact records from regression/data/linxy.parquet, retaining formula_id for diagnosis.
const REGRESSION: &[(&str, &str)] = &[
    (
        "e8d09faf7482",
        r"d s ^ { 2 } = ( 1 - { \frac { q c o s \theta } { r } } ) ^ { \frac { 2 } { 1 + \alpha ^ { 2 } } } \lbrace d r ^ { 2 } + r ^ { 2 } d \theta ^ { 2 } + r ^ { 2 } s i n ^ { 2 } \theta d \varphi ^ { 2 } \rbrace - { \frac { d t ^ { 2 } } { ( 1 - { \frac { q c o s \theta } { r } } ) ^ { \frac { 2 } { 1 + \alpha ^ { 2 } } } } } .",
    ),
    (
        "1f289469262c",
        r"\widetilde \gamma _ { \mathrm { h o p f } } \simeq \sum _ { n > 0 } \widetilde { G } _ { n } { \frac { ( - a ) ^ { n } } { 2 ^ { 2 n - 1 } } }",
    ),
    (
        "58b533efd2be",
        r"\rho _ { L } ( q ) = \sum _ { m = 1 } ^ { L } \ P _ { L } ( m ) \ { \frac { 1 } { q ^ { m - 1 } } } .",
    ),
    (
        "2cd93f090ed4",
        r"{ \frac { d ^ { 2 } \varphi } { d \tau ^ { 2 } } } = \sin \varphi \ ,",
    ),
    (
        "788e138b377e",
        r"| t _ { i , j } | ^ { 2 } \leq t _ { i , i } t _ { j , j } , t _ { i , i } > 0 \forall i , j",
    ),
    (
        "b50c400c67a2",
        r"s + t + u = - \frac { 8 } { \alpha ^ { \prime } }",
    ),
    (
        "6250ee8eb03c",
        r"m _ { \phi _ { 0 } } ^ { 2 } \sim { \frac { m ^ { 2 } v ^ { 2 } } { \Lambda ^ { 2 } } } .",
    ),
    (
        "dcfa8486638a",
        r"( \partial b ) ^ { * } = - q ^ { 2 } ( D ^ { - 1 } ) \left( - q ^ { 3 } c d ( \partial a ) + q a d ( \partial c ) + q ^ { 2 } c ^ { 2 } ( \partial b ) - a c ( \partial d ) \right) ,",
    ),
    (
        "79cec153761a",
        r"d _ { k } > \frac { 9 } { ( k + 2 ) ^ { 2 } } ( \bar { \varphi } \varphi ) _ { 0 } ^ { \frac { 4 } { 3 } } ( \bar { F } F ) _ { 0 } ^ { - 1 } d _ { k - 1 } \ .",
    ),
    (
        "e14705be6722",
        r"\varepsilon ^ { \mu \nu \alpha \beta } \partial _ { \nu } B _ { \alpha \beta } = 0 .",
    ),
    (
        "8e22d3f74453",
        r"\mathrm { I ) } \quad { \cal L } = \partial _ { z } \varphi \partial _ { \bar { z } } \varphi - 4 \varphi ^ { 2 } + 2 \varphi ^ { 4 } \quad \mathrm { a n d } \quad \mathrm { I I ) } \quad { \cal L } = \partial _ { z } \varphi \partial _ { \bar { z } } \varphi + 2 \varphi ^ { 2 } - 2 \varphi ^ { 4 } .",
    ),
    (
        "6fe065b70621",
        r"{ \cal W } \equiv { \frac { 1 } { 4 \rho ^ { 2 } } } \Big [ \cosh ( 2 \varphi _ { 2 } ) ( \rho ^ { 6 } - 2 ) - ( 3 \rho ^ { 6 } + 2 ) \Big ] , \qquad \rho \equiv e ^ { { \frac { 1 } { \sqrt { 6 } } } \varphi _ { 1 } } .",
    ),
];

// Complement the real corpus sample with structures that are sparse in its first records.
const STRUCTURAL_FIXTURES: &[(&str, &str)] = &[
    ("environment", r"\begin{matrix}a&b\\c&d\end{matrix}"),
    (
        "column-environment",
        r"\begin{array}{cc}a&b\\c&d\end{array}",
    ),
    ("text-inline-math", r"\text{before $x_i^2$ after}"),
    ("infix", r"{a\over b}+{c\choose d}"),
    ("paired", r"\qty(x)+\qty[y]+\qty{z}+\qty|w|"),
    ("delimiters-primes", r"\left(x_i+y\right)+z''"),
];
