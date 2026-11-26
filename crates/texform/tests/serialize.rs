mod support;

use std::collections::HashSet;

use support::kb;
use texform::{
    AllowedMode, CommandItem, CommandKind, ContentMode, Document, FragmentKind, FragmentRole,
    FragmentedLatex, KnowledgeBase, NodeKind, NodeSlot, ParseConfig, Parser, SerializationFragment,
    SerializeOptions, Style, SyntaxNode,
};

fn assert_fragment_contract(result: &FragmentedLatex) {
    let mut cursor = 0;
    for fragment in &result.fragments {
        assert!(fragment.span.start < fragment.span.end);
        assert!(cursor <= fragment.span.start);
        assert!(
            result.latex[cursor..fragment.span.start]
                .chars()
                .all(char::is_whitespace)
        );
        assert_eq!(fragment.text, result.latex[fragment.span.clone()]);
        cursor = fragment.span.end;
    }
    assert!(result.latex[cursor..].chars().all(char::is_whitespace));
}

#[test]
fn document_serializes_parsed_latex() {
    let parser = texform::Parser::builder()
        .knowledge_base(kb(&["base"]))
        .build();
    let document = parser
        .parse(r"\frac{a}{b}")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    assert_eq!(
        document.to_latex().expect("document should serialize"),
        r"\frac { a } { b }"
    );
}

#[test]
fn document_serializes_syntax_root() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![
            SyntaxNode::Char('a'),
            SyntaxNode::Char('+'),
            SyntaxNode::Char('b'),
        ],
    );
    let document = Document::from_syntax(&node).expect("syntax root should become a document");

    assert_eq!(
        document.to_latex().expect("document should serialize"),
        "a + b"
    );
    assert_eq!(
        document
            .to_latex_with(&SerializeOptions::default())
            .expect("document should serialize with options"),
        "a + b"
    );
}

#[test]
fn document_exposes_stable_fragmented_serialization_contract() {
    let parser = texform::Parser::builder()
        .knowledge_base(kb(&["base"]))
        .build();
    let document = parser
        .parse(r"\text{a\% $𝒜_i$}")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let result = document
        .to_latex_fragments()
        .expect("document should serialize with fragments");
    assert_eq!(result.latex, document.to_latex().unwrap());
    assert_fragment_contract(&result);
    assert!(result.fragments.iter().any(|fragment| {
        fragment.text == r"\%"
            && fragment.kind == FragmentKind::Character
            && fragment.mode == ContentMode::Text
    }));
    assert!(result.fragments.iter().any(|fragment| {
        fragment.text == "_"
            && fragment.kind == FragmentKind::Character
            && fragment.mode == ContentMode::Math
    }));
    let options = SerializeOptions::default();
    assert_eq!(
        document.to_latex_fragments_with(&options).unwrap().latex,
        document.to_latex_with(&options).unwrap()
    );
}

#[test]
fn serialize_options_are_constructible_from_the_facade() {
    use texform::{Parser, SerializeOptions, Style};

    let parser = Parser::builder().build();
    let document = parser
        .parse("x_i^2")
        .try_into_document()
        .expect("parse should produce a document")
        .0;
    let options = SerializeOptions {
        style: Style::Compact,
    };

    assert_eq!(
        document
            .to_latex_with(&options)
            .expect("document should serialize"),
        "x_i^2"
    );
}

#[test]
fn serialize_options_serde_rejects_unknown_nested_keys() {
    let error = serde_json::from_value::<SerializeOptions>(serde_json::json!({
        "math": { "scripts": { "order": "sup_first" } }
    }))
    .unwrap_err();
    assert!(
        error.to_string().contains("unknown field `math`"),
        "{error}"
    );
}

#[test]
fn middle_delimiter_serializes_bare_and_is_text_idempotent() {
    let parser = texform::Parser::builder()
        .knowledge_base(kb(&["base"]))
        .build();
    let parsed = parser.parse(r"\left\langle a\middle\vert b\right\rangle");
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected diagnostics: {:?}",
        parsed.diagnostics()
    );
    let document = parsed
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let latex = document.to_latex().expect("document should serialize");
    assert!(
        latex.contains(r"\middle \vert"),
        "expected a bare middle delimiter, got {latex}"
    );
    assert!(
        !latex.contains(r"{\vert}"),
        "mandatory delimiter must not be brace-wrapped, got {latex}"
    );

    let again = parser
        .parse(&latex)
        .try_into_document()
        .expect("serialized output should parse")
        .0;
    assert_eq!(
        again
            .to_latex()
            .expect("reserialized document should serialize"),
        latex
    );
}

#[test]
fn bigl_non_ascii_fragment_spans_use_utf8_byte_offsets() {
    let parser = texform::Parser::builder()
        .knowledge_base(kb(&["base"]))
        .build();
    let document = parser
        .parse(r"\bigl( é \bigr)")
        .try_into_document()
        .expect("parse should produce a document")
        .0;

    let result = document
        .to_latex_fragments()
        .expect("document should serialize with fragments");
    assert_eq!(result.latex, document.to_latex().unwrap());
    assert_fragment_contract(&result);
    assert!(
        result
            .fragments
            .iter()
            .any(|fragment| { fragment.text == "é" && fragment.kind == FragmentKind::Character })
    );
}

fn assert_node_path_and_role_consistency(doc: &Document, result: &FragmentedLatex, label: &str) {
    assert_fragment_contract(result);
    for fragment in &result.fragments {
        let node = doc.node_at(&fragment.node_path).unwrap_or_else(|err| {
            panic!(
                "doc.node_at({:?}) failed for fragment {:?} in `{label}`: {err}",
                fragment.node_path, fragment
            )
        });
        let kind = node.kind();
        match fragment.role {
            FragmentRole::Head => {
                assert!(
                    matches!(
                        kind,
                        NodeKind::Command
                            | NodeKind::Declarative
                            | NodeKind::Infix
                            | NodeKind::Environment
                    ),
                    "{label}: {fragment:?} resolved to {kind:?}"
                );
                assert_eq!(fragment.slot, None, "{label}: {fragment:?}");
            }
            FragmentRole::Open | FragmentRole::Close => match fragment.slot {
                None => assert!(
                    matches!(kind, NodeKind::Group | NodeKind::Environment),
                    "{label}: {fragment:?} resolved to {kind:?}"
                ),
                Some(NodeSlot::Arg(_)) => assert!(
                    matches!(
                        kind,
                        NodeKind::Command
                            | NodeKind::Infix
                            | NodeKind::Declarative
                            | NodeKind::Environment
                    ),
                    "{label}: {fragment:?} resolved to {kind:?}"
                ),
                Some(NodeSlot::Subscript | NodeSlot::Superscript) => {
                    assert_eq!(kind, NodeKind::Scripted, "{label}: {fragment:?}")
                }
                Some(NodeSlot::InfixLeft | NodeSlot::InfixRight) => {
                    assert_eq!(kind, NodeKind::Infix, "{label}: {fragment:?}")
                }
                other => panic!("{label}: unexpected slot {other:?} for {fragment:?}"),
            },
            FragmentRole::Name => {
                assert_eq!(kind, NodeKind::Environment, "{label}: {fragment:?}");
                assert_eq!(fragment.slot, None, "{label}: {fragment:?}");
            }
            FragmentRole::Operator => {
                assert_eq!(kind, NodeKind::Scripted, "{label}: {fragment:?}");
                assert!(
                    matches!(
                        fragment.slot,
                        Some(NodeSlot::Subscript | NodeSlot::Superscript)
                    ),
                    "{label}: {fragment:?}"
                );
            }
            FragmentRole::Delimiter => match fragment.slot {
                None => assert_eq!(kind, NodeKind::Group, "{label}: {fragment:?}"),
                Some(NodeSlot::Arg(_)) => assert!(
                    matches!(
                        kind,
                        NodeKind::Command
                            | NodeKind::Infix
                            | NodeKind::Declarative
                            | NodeKind::Environment
                    ),
                    "{label}: {fragment:?} resolved to {kind:?}"
                ),
                other => panic!("{label}: unexpected slot {other:?} for {fragment:?}"),
            },
            FragmentRole::Value => {
                assert!(
                    matches!(
                        kind,
                        NodeKind::Command
                            | NodeKind::Infix
                            | NodeKind::Declarative
                            | NodeKind::Environment
                    ),
                    "{label}: {fragment:?} resolved to {kind:?}"
                );
                assert!(
                    matches!(fragment.slot, Some(NodeSlot::Arg(_))),
                    "{label}: {fragment:?}"
                );
            }
            FragmentRole::Leaf => {
                assert!(
                    matches!(
                        kind,
                        NodeKind::Char
                            | NodeKind::Text
                            | NodeKind::Prime
                            | NodeKind::ActiveSpace
                            | NodeKind::AlignmentTab
                            | NodeKind::Error
                    ),
                    "{label}: {fragment:?} resolved to {kind:?}"
                );
                assert_eq!(fragment.slot, None, "{label}: {fragment:?}");
            }
        }
    }
}

#[test]
fn fragmented_provenance_covers_all_roles_and_valid_node_paths_in_both_styles() {
    let all_packages: Vec<_> = texform::list_packages()
        .into_iter()
        .map(|package| package.name)
        .collect();
    let kb_all = KnowledgeBase::builder()
        .packages(&all_packages.iter().map(String::as_str).collect::<Vec<_>>())
        .item(CommandItem::new(
            "untilcmd",
            CommandKind::Prefix,
            AllowedMode::Math,
            r"u{\stop}",
        ))
        .build()
        .expect("custom knowledge base should build");
    let parser = Parser::builder().knowledge_base(kb_all).build();

    let formulas = [
        r"\frac 1 2",
        r"\frac{a+b}{c}",
        r"\frac{}{}",
        r"\genfrac{}{}{0pt}{}{a}{b}",
        r"\genfrac(]{0pt}{1}{a}{b}",
        r"\left( a + \middle\vert b \right]",
        r"\left. a \right.",
        r"\bigl( x \bigr)",
        r"\begin{array}{cc} a & b \\ c & d \end{array}",
        r"\begin{align*} x &= 1 \end{align*}",
        r"\text{abc $x_a^b$ \% \{ }",
        r"x_a^b",
        r"x_{ab}^{cd}",
        r"x_i^{\alpha} + x_{\mathbf{a}}^{2}",
        r"{}_a^b",
        r"f'' + f^{\prime 2} + \prime",
        r"a \over b",
        r"{a \over b} \over c",
        r"{a} \over b",
        r"a \abovewithdelims()1pt b",
        r"{\bf x} + {\displaystyle \color{red} y}",
        r"\sqrt{x} + \sqrt[3]{x} + \sqrt[{a[b]}]{x}",
        r"\buildrel a \over =",
        r"\operatorname*{arg\,max}",
        r"\bra{x}\ket{y}\braket{a|b}",
        r"\qty(a + b) + \dv*{f}{x}",
        r"\untilcmd x + y \stop",
        r"a~b + {c} + {}",
    ];

    let mut seen_roles: HashSet<&'static str> = HashSet::new();

    for source in formulas {
        let (doc, _) = parser
            .parse(source)
            .try_into_document()
            .unwrap_or_else(|err| panic!("failed to parse `{source}`: {err}"));
        for style in [Style::Spaced, Style::Compact] {
            let options = SerializeOptions { style };
            let result = doc.to_latex_fragments_with(&options).unwrap();
            assert_eq!(
                result.latex,
                doc.to_latex_with(&options).unwrap(),
                "latex mismatch for `{source}` ({style:?})"
            );
            assert_node_path_and_role_consistency(&doc, &result, source);
            for fragment in &result.fragments {
                seen_roles.insert(fragment.role.as_str());
            }
        }
    }

    // Also test lenient partial documents with Error nodes.
    let partial = parser.parse_with(
        r"\frac{a",
        &ParseConfig {
            abort_on_error: false,
            ..ParseConfig::default()
        },
    );
    let partial_doc = partial.document().expect("partial document expected");
    assert!(partial_doc.has_errors());
    for style in [Style::Spaced, Style::Compact] {
        let options = SerializeOptions { style };
        let result = partial_doc.to_latex_fragments_with(&options).unwrap();
        assert_eq!(result.latex, partial_doc.to_latex_with(&options).unwrap());
        assert_node_path_and_role_consistency(partial_doc, &result, r"\frac{a");
        for fragment in &result.fragments {
            seen_roles.insert(fragment.role.as_str());
        }
    }

    // Also test an environment whose body is an Explicit group (not unwrapped).
    let explicit_env_syntax = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::Environment {
            name: "matrix".to_string(),
            args: vec![],
            body: Box::new(SyntaxNode::Group {
                children: vec![SyntaxNode::Char('x')],
                kind: texform::GroupKind::Explicit,
                mode: ContentMode::Math,
            }),
            known: true,
        }],
    );
    let explicit_env_doc = Document::from_syntax(&explicit_env_syntax).unwrap();
    for style in [Style::Spaced, Style::Compact] {
        let options = SerializeOptions { style };
        let result = explicit_env_doc.to_latex_fragments_with(&options).unwrap();
        assert_eq!(
            result.latex,
            explicit_env_doc.to_latex_with(&options).unwrap()
        );
        assert_node_path_and_role_consistency(&explicit_env_doc, &result, "explicit env body");
    }

    // Verify all 8 FragmentRole variants were exercised and have the expected string.
    let all_roles = [
        (FragmentRole::Head, "head"),
        (FragmentRole::Open, "open"),
        (FragmentRole::Close, "close"),
        (FragmentRole::Name, "name"),
        (FragmentRole::Operator, "operator"),
        (FragmentRole::Delimiter, "delimiter"),
        (FragmentRole::Value, "value"),
        (FragmentRole::Leaf, "leaf"),
    ];
    for (role, expected_str) in all_roles {
        assert_eq!(role.as_str(), expected_str);
        assert!(
            seen_roles.contains(expected_str),
            "role `{expected_str}` was not covered by test formulas"
        );
    }
}

fn format_slot(slot: Option<NodeSlot>) -> String {
    match slot {
        None => "-".to_string(),
        Some(NodeSlot::Child(i)) => format!("child.{i}"),
        Some(NodeSlot::Arg(i)) => format!("arg.{i}"),
        Some(other) => other.as_str().to_string(),
    }
}

fn format_fragment_lines(fragments: &[SerializationFragment]) -> String {
    fragments
        .iter()
        .map(|f| {
            format!(
                "{} {} {} {}",
                f.text,
                f.role.as_str(),
                f.node_path,
                format_slot(f.slot)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_snapshot(expected: &str) -> String {
    expected
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn fragmented_paths_and_slots_match_exact_expected_structure() {
    let parser = Parser::builder().build();
    let assert_full = |source: &str, style: Style, expected: &str| {
        let (doc, _) = parser.parse(source).try_into_document().unwrap();
        let res = doc.to_latex_fragments_with(&style.into()).unwrap();
        assert_eq!(
            format_fragment_lines(&res.fragments),
            normalize_snapshot(expected),
            "fragment mismatch for `{source}` ({style:?})"
        );
        if style == Style::Compact {
            assert_node_path_and_role_consistency(&doc, &res, &format!("{source} compact"));
        }
    };
    let assert_prefix = |source: &str, count: usize, expected: &str| {
        let (doc, _) = parser.parse(source).try_into_document().unwrap();
        let res = doc.to_latex_fragments().unwrap();
        assert_eq!(
            format_fragment_lines(&res.fragments[..count]),
            normalize_snapshot(expected),
            "fragment prefix mismatch for `{source}`"
        );
    };

    // 1. `\sqrt{x}`: omitted optional slot 0 means `{` / `}` belong to `Arg(1)`.
    assert_full(
        r"\sqrt{x}",
        Style::Spaced,
        r"
        \sqrt  head   root.child.0                -
        {      open   root.child.0                arg.1
        x      leaf   root.child.0.arg.1.content  -
        }      close  root.child.0                arg.1
        ",
    );

    // 2. `\sqrt[3]{x}`: optional `[3]` on `Arg(0)`, mandatory `{x}` on `Arg(1)`.
    assert_full(
        r"\sqrt[3]{x}",
        Style::Spaced,
        r"
        \sqrt  head   root.child.0                -
        [      open   root.child.0                arg.0
        3      leaf   root.child.0.arg.0.content  -
        ]      close  root.child.0                arg.0
        {      open   root.child.0                arg.1
        x      leaf   root.child.0.arg.1.content  -
        }      close  root.child.0                arg.1
        ",
    );

    // 3. `\genfrac(]{0pt}{1}{a}{b}`: exact Arg(0)..Arg(5) attribution.
    assert_full(
        r"\genfrac(]{0pt}{1}{a}{b}",
        Style::Spaced,
        r"
        \genfrac  head       root.child.0                -
        (         delimiter  root.child.0                arg.0
        ]         delimiter  root.child.0                arg.1
        {         open       root.child.0                arg.2
        0pt       value      root.child.0                arg.2
        }         close      root.child.0                arg.2
        {         open       root.child.0                arg.3
        1         value      root.child.0                arg.3
        }         close      root.child.0                arg.3
        {         open       root.child.0                arg.4
        a         leaf       root.child.0.arg.4.content  -
        }         close      root.child.0                arg.4
        {         open       root.child.0                arg.5
        b         leaf       root.child.0.arg.5.content  -
        }         close      root.child.0                arg.5
        ",
    );

    // 4. `\genfrac{}{}{0pt}{}{a}{b}`: empty delimiter `{}` on Arg(0) and Arg(1).
    assert_prefix(
        r"\genfrac{}{}{0pt}{}{a}{b}",
        5,
        r"
        \genfrac  head   root.child.0  -
        {         open   root.child.0  arg.0
        }         close  root.child.0  arg.0
        {         open   root.child.0  arg.1
        }         close  root.child.0  arg.1
        ",
    );

    // 5. `x_{ab}^{c}`: Subscript and Superscript operators and braces.
    assert_full(
        r"x_{ab}^{c}",
        Style::Spaced,
        r"
        x  leaf      root.child.0.base         -
        _  operator  root.child.0              subscript
        {  open      root.child.0              subscript
        a  leaf      root.child.0.sub.child.0  -
        b  leaf      root.child.0.sub.child.1  -
        }  close     root.child.0              subscript
        ^  operator  root.child.0              superscript
        {  open      root.child.0              superscript
        c  leaf      root.child.0.sup.child.0  -
        }  close     root.child.0              superscript
        ",
    );

    // 6. Prime shorthand `f''`: `''` is emitted from the Prime child at `.sup` with `slot: None`.
    assert_full(
        "f''",
        Style::Spaced,
        r"
        f   leaf  root.child.0.base  -
        ''  leaf  root.child.0.sup   -
        ",
    );

    // 7. Braced infix operands `{a} \over {b}` -> `InfixLeft` / `InfixRight`.
    assert_full(
        r"{a} \over {b}",
        Style::Spaced,
        r"
        {      open   root.child.0                infix_left
        a      leaf   root.child.0.left.child.0   -
        }      close  root.child.0                infix_left
        \over  head   root.child.0                -
        {      open   root.child.0                infix_right
        b      leaf   root.child.0.right.child.0  -
        }      close  root.child.0                infix_right
        ",
    );

    // 8. Environment: `{array}` has `slot: None`; `array` argspec has optional alignment at Arg(0)
    // and mandatory column spec `{cc}` at Arg(1).
    assert_full(
        r"\begin{array}{cc}a&b\end{array}",
        Style::Spaced,
        r"
        \begin  head   root.child.0               -
        {       open   root.child.0               -
        array   name   root.child.0               -
        }       close  root.child.0               -
        {       open   root.child.0               arg.1
        cc      value  root.child.0               arg.1
        }       close  root.child.0               arg.1
        a       leaf   root.child.0.body.child.0  -
        &       leaf   root.child.0.body.child.1  -
        b       leaf   root.child.0.body.child.2  -
        \end    head   root.child.0               -
        {       open   root.child.0               -
        array   name   root.child.0               -
        }       close  root.child.0               -
        ",
    );

    // 9. `\buildrel a \over =`: `\over` terminator carries `Some(NodeSlot::Arg(0))`.
    assert_full(
        r"\buildrel a \over =",
        Style::Spaced,
        r"
        \buildrel  head   root.child.0                -
        {          open   root.child.0                arg.0
        a          leaf   root.child.0.arg.0.content  -
        }          close  root.child.0                arg.0
        \over      close  root.child.0                arg.0
        {          open   root.child.0                arg.1
        =          leaf   root.child.0.arg.1.content  -
        }          close  root.child.0                arg.1
        ",
    );

    // 10. `\operatorname*{lim}`: star `*` carries `Value` and `Some(NodeSlot::Arg(0))`.
    assert_prefix(
        r"\operatorname*{lim}",
        3,
        r"
        \operatorname  head   root.child.0  -
        *              value  root.child.0  arg.0
        {              open   root.child.0  arg.1
        ",
    );

    // 11. Compact bare script unwrapping single-child implicit group vs direct char.
    assert_full(
        r"x_{i}^2",
        Style::Compact,
        r"
        x  leaf      root.child.0.base         -
        _  operator  root.child.0              subscript
        i  leaf      root.child.0.sub.child.0  -
        ^  operator  root.child.0              superscript
        2  leaf      root.child.0.sup          -
        ",
    );
}
