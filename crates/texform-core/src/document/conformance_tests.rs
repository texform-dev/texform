use super::*;
use crate::ast::Argument;
use crate::parse::{AllowedMode, CommandItem, DelimiterControlItem, EnvironmentItem, ParseConfig};
use ConformanceRule as Rule;

fn check_leaf(node: &Node, mode: ContentMode, kb: &KnowledgeBase) -> Check {
    check_node(node, mode, kb, |_| panic!("leaf has no children"))
}

fn rule(result: Check) -> ConformanceRule {
    result.unwrap_err().rule
}

fn command(name: &str, args: usize, known: bool) -> Node {
    Node::Command {
        name: name.into(),
        args: vec![None; args],
        known,
    }
}

fn slot(kind: ArgumentKind, value: ArgumentValue) -> ArgumentSlot {
    Some(Argument::from_value(kind, value))
}

/// Knowledge records hold `'static` argument specs; tests leak their own.
fn signature(spec: ArgSpec) -> &'static [ArgSpec] {
    Box::leak(Box::new([spec]))
}

fn parsed(source: &str, kb: &KnowledgeBase) -> crate::document::Document {
    let result = kb.parse(source, &ParseConfig::default());
    assert!(
        result.diagnostics.is_empty(),
        "{source:?}: {:?}",
        result.diagnostics
    );
    let doc = result.document.expect("complete parsed document");
    check_tree(&doc.ast, doc.ast.root(), ContentMode::Math, kb, "root")
        .unwrap_or_else(|error| panic!("{source:?}: {error}"));
    doc
}

fn leaf_content(doc: &crate::document::Document) -> String {
    let mut text = String::new();
    for id in doc.ast.find_all(doc.ast.root(), |node| {
        matches!(node, Node::Char(_) | Node::Text(_))
    }) {
        match doc.ast.node(id) {
            Node::Char(c) => text.push(*c),
            Node::Text(value) => text.push_str(value),
            _ => unreachable!(),
        }
    }
    text
}

#[test]
fn every_accepted_ascii_and_unicode_leaf_preserves_character_content() {
    let kb = KnowledgeBase::default();
    let chars = (0u8..=127)
        .map(char::from)
        .chain(['\u{2019}', '\u{a0}', '\u{2007}', '\u{3000}']);
    for c in chars {
        for mode in [ContentMode::Math, ContentMode::Text] {
            for node in [Node::Char(c), Node::Text(c.to_string())] {
                let accepted = check_leaf(&node, mode, &kb).is_ok();
                let forbidden_common = matches!(
                    c,
                    '\\' | '^' | '~' | '\u{2019}' | '\x00'..='\x1f' | '\x7f' | '\u{a0}'
                );
                let expected = match node {
                    Node::Char(_) => {
                        !forbidden_common && !(mode == ContentMode::Math && matches!(c, ' ' | '\''))
                    }
                    Node::Text(_) => {
                        mode == ContentMode::Text && !forbidden_common && !"%$&#_{}".contains(c)
                    }
                    _ => unreachable!(),
                };
                assert_eq!(accepted, expected, "{mode:?}: {node:?}");
                if !accepted {
                    continue;
                }
                let mut ast = Ast::with_root_mode(mode);
                let id = ast.new_node(node.clone());
                ast.append_child(ast.root(), id);
                let latex = crate::serialize::serialize(&ast);
                let source = if mode == ContentMode::Text {
                    format!("\\text{{{latex}}}")
                } else {
                    latex
                };
                let doc = parsed(&source, &kb);
                assert_eq!(leaf_content(&doc), c.to_string(), "{mode:?}: {node:?}");
            }
        }
    }
}

#[test]
fn unknown_names_follow_lexer_tokens_instead_of_leaf_restrictions() {
    let kb = KnowledgeBase::builder().packages(&[]).build().unwrap();
    for c in (0u8..=127)
        .map(char::from)
        .chain(['\u{2019}', '\u{a0}', '\u{2007}', '\u{3000}'])
    {
        let name = c.to_string();
        let accepted = check_leaf(&command(&name, 0, false), ContentMode::Math, &kb).is_ok();
        assert_eq!(
            accepted,
            !matches!(c, '\r' | '\n' | '%' | '$' | '&' | '#' | '_' | '{' | '}'),
            "{name:?}"
        );
        if accepted {
            let doc = parsed(&format!("\\{name}"), &kb);
            assert!(
                doc.ast
                    .find_all(
                        doc.ast.root(),
                        |node| matches!(node, Node::Command { name: actual, .. } if actual == &name)
                    )
                    .len()
                    == 1,
                "{name:?}"
            );
        }
        let accepted_env = valid_environment_name(&name);
        let expected_env =
            (c >= ' ' && c != '\x7f') && !" \\~{}$&#^_*[]'\u{2019}\u{a0}%".contains(c) || c == '*';
        assert_eq!(accepted_env, expected_env, "environment {name:?}");
        if accepted_env {
            let doc = parsed(&format!("\\begin{{{name}}}x\\end{{{name}}}"), &kb);
            assert_eq!(doc.ast.find_all(doc.ast.root(), |node| matches!(node, Node::Environment { name: actual, .. } if actual == &name)).len(), 1);
        }
    }
    for name in ["", "foo bar", "alpha1", "begin", "end", "left", "right"] {
        assert_eq!(
            rule(check_leaf(&command(name, 0, false), ContentMode::Math, &kb)),
            Rule::InvalidName,
            "{name}"
        );
    }
    for name in ["foo bar", "x[y", "x'y", "alpha1%comment"] {
        assert!(!valid_environment_name(name));
    }
}

#[test]
fn invalid_runtime_names_fail_knowledge_base_building() {
    for name in ["", "foo bar", "alpha1", "%", "\n"] {
        assert!(
            KnowledgeBase::builder()
                .item(CommandItem::new(
                    name,
                    CommandKind::Prefix,
                    AllowedMode::Both,
                    ""
                ))
                .build()
                .is_err()
        );
        if name != "%" {
            assert!(
                KnowledgeBase::builder()
                    .item(DelimiterControlItem::new(name))
                    .build()
                    .is_err()
            );
        }
    }
    for name in ["", "foo bar", "a[b", "a'b", "a%comment"] {
        assert!(
            KnowledgeBase::builder()
                .item(EnvironmentItem::new(
                    name,
                    AllowedMode::Both,
                    ContentMode::Math,
                    ""
                ))
                .build()
                .is_err()
        );
    }
}

#[test]
fn parser_labels_and_delimiter_only_records_remain_conformant() {
    let kb = KnowledgeBase::default();
    for source in [
        r"\label{sec:intro}",
        r"\ref{eq:2.2}",
        r"\eqref{alpha1}",
        r"\label{}",
        r"\text{\(}",
    ] {
        parsed(source, &kb);
    }
    let kb = KnowledgeBase::builder()
        .packages(&[])
        .item(DelimiterControlItem::new("customdelim"))
        .build()
        .unwrap();
    assert!(
        kb.lookup_command("customdelim", ContentMode::Math)
            .is_none()
    );
    parsed(r"\customdelim", &kb);
    let unknown = command("customdelim", 0, false);
    assert_eq!(
        rule(check_leaf(&unknown, ContentMode::Math, &kb)),
        Rule::KnownFlagMismatch
    );
    // In text mode the parser reads delimiter controls as unknown commands.
    check_leaf(&unknown, ContentMode::Text, &kb).unwrap();
}

#[test]
fn local_checks_detect_changed_siblings_and_staged_context_without_mutation() {
    let kb = KnowledgeBase::default();
    let doc = parsed(r"a\over b", &kb);
    let before = doc.ast.to_syntax_root();
    let Node::Root { children, .. } = doc.ast.node(doc.ast.root()) else {
        panic!()
    };
    let mut proposed = doc.ast.node(doc.ast.root()).clone();
    let Node::Root {
        children: updated, ..
    } = &mut proposed
    else {
        panic!()
    };
    updated.push(children[0]);
    assert_eq!(
        rule(check_node(&proposed, ContentMode::Math, &kb, |id| {
            (doc.ast.node(id), None)
        })),
        Rule::InfixPlacement
    );
    assert_eq!(before, doc.ast.to_syntax_root());
    let doc = parsed("x", &kb);
    let error = check_node(doc.ast.node(doc.ast.root()), ContentMode::Math, &kb, |id| {
        (doc.ast.node(id), Some(ContentMode::Text))
    })
    .unwrap_err();
    assert_eq!(
        (error.rule, error.path.as_str()),
        (Rule::ModeMismatch, "child.0")
    );
}

#[test]
fn scalar_values_must_read_back_through_the_argument_parser() {
    use ArgumentValue as V;
    let kb = KnowledgeBase::default();
    let mandatory = |kind| ArgSpec {
        kind,
        ..ArgSpec::mandatory(ContentMode::Math)
    };
    let nullable = ArgSpec {
        nullable: true,
        ..mandatory(ValueKind::Dimension)
    };
    let label = ArgSpec {
        kind: ValueKind::CSName,
        ..ArgSpec::optional(ContentMode::Math)
    };
    for (spec, value, accepted) in [
        (
            mandatory(ValueKind::Dimension),
            V::Dimension("-1.5pt".into()),
            true,
        ),
        (
            mandatory(ValueKind::Dimension),
            V::Dimension("1xyz".into()),
            false,
        ),
        // The parser would normalize or drop part of these values.
        (
            mandatory(ValueKind::Dimension),
            V::Dimension("1,5pt".into()),
            false,
        ),
        (
            mandatory(ValueKind::Dimension),
            V::Dimension("1pt%comment".into()),
            false,
        ),
        (
            mandatory(ValueKind::Dimension),
            V::Dimension(String::new()),
            false,
        ),
        (nullable, V::Dimension(String::new()), true),
        (
            mandatory(ValueKind::Integer),
            V::Integer("-12".into()),
            true,
        ),
        (
            mandatory(ValueKind::Integer),
            V::Integer("1.2".into()),
            false,
        ),
        (
            mandatory(ValueKind::KeyVal),
            V::KeyVal("a={b,c}".into()),
            true,
        ),
        (mandatory(ValueKind::KeyVal), V::KeyVal("a=".into()), false),
        (
            mandatory(ValueKind::Column),
            V::Column("c|p{2pt}".into()),
            true,
        ),
        (mandatory(ValueKind::Column), V::Column("p{".into()), false),
        (
            mandatory(ValueKind::CSName),
            V::CSName("sec:intro".into()),
            true,
        ),
        (mandatory(ValueKind::CSName), V::CSName(String::new()), true),
        (
            mandatory(ValueKind::CSName),
            V::CSName("\\bad".into()),
            false,
        ),
        (label.clone(), V::CSName("a]b".into()), false),
        (label, V::CSName("{a]b}".into()), true),
    ] {
        let kind = if spec.required {
            ArgumentKind::Mandatory
        } else {
            ArgumentKind::Optional
        };
        match check_arguments(&[slot(kind, value.clone())], signature(spec), &kb) {
            Ok(()) => assert!(accepted, "{value:?}"),
            Err(error) => {
                assert!(!accepted, "{value:?}: {error}");
                assert_eq!(
                    (error.rule, error.path.as_str()),
                    (Rule::InvalidArgumentValue, "arg.0")
                );
            }
        }
    }
}

#[test]
fn local_rules_report_stable_codes() {
    use ContentMode::{Math, Text};
    let kb = KnowledgeBase::default();
    let declarative = |name: &str| Node::Declarative {
        name: name.into(),
        args: vec![],
    };
    let group = |kind| Node::Group {
        children: vec![],
        mode: Math,
        kind,
    };
    let unregistered_delimiter = GroupKind::Delimited {
        left: Delimiter::Control("notadelimiter".into()),
        right: Delimiter::None,
    };
    let error = Node::Error {
        message: "error".into(),
        snippet: "?".into(),
    };
    for (node, mode, expected) in [
        (Node::Text("two  spaces".into()), Text, Rule::InvalidText),
        (Node::AlignmentTab, Text, Rule::ModeMismatch),
        (command("alpha", 0, false), Math, Rule::KnownFlagMismatch),
        (command("alpha", 0, false), Text, Rule::ModeMismatch),
        (declarative("alpha"), Math, Rule::CommandKindMismatch),
        (declarative("unregistered"), Math, Rule::CommandKindMismatch),
        (
            command("unregistered", 1, false),
            Math,
            Rule::UnknownWithArguments,
        ),
        (command("frac", 0, true), Math, Rule::ArgumentCount),
        (
            command("frac", 2, true),
            Math,
            Rule::MissingRequiredArgument,
        ),
        (Node::Prime { count: 0 }, Math, Rule::InvalidPrimeCount),
        (Node::Prime { count: 1 }, Text, Rule::ModeMismatch),
        (error, Math, Rule::ErrorNode),
        (group(unregistered_delimiter), Math, Rule::InvalidDelimiter),
        (group(GroupKind::InlineMath), Math, Rule::ModeMismatch),
    ] {
        assert_eq!(
            rule(check_leaf(&node, mode, &kb)),
            expected,
            "{node:?} in {mode:?}"
        );
    }
    check_leaf(&group(GroupKind::InlineMath), Text, &kb).unwrap();
}

#[test]
fn argument_slots_follow_the_signature() {
    use std::borrow::Cow;
    let kb = KnowledgeBase::default();
    let star = ArgSpec {
        kind: ValueKind::Star,
        form: ArgForm::Star,
        required: false,
        nullable: false,
        no_leading_space: false,
    };
    let integer = ArgSpec {
        kind: ValueKind::Integer,
        ..ArgSpec::mandatory(ContentMode::Math)
    };
    let paired = ArgSpec {
        form: ArgForm::Paired {
            pairs: Cow::Owned(vec![
                (DelimiterToken::Char('('), DelimiterToken::Char(')')),
                (DelimiterToken::Char('['), DelimiterToken::Char(']')),
            ]),
        },
        ..integer.clone()
    };
    let one = || ArgumentValue::Integer("1".into());
    let pair = |open, close| {
        slot(
            ArgumentKind::Paired {
                open: Delimiter::Char(open),
                close: Delimiter::Char(close),
            },
            one(),
        )
    };
    for (spec, argument, expected) in [
        (&star, None, Some(Rule::MissingRequiredArgument)),
        (
            &star,
            slot(ArgumentKind::Star, ArgumentValue::Boolean(false)),
            None,
        ),
        (
            &integer,
            slot(ArgumentKind::Optional, one()),
            Some(Rule::ArgumentForm),
        ),
        (
            &integer,
            slot(ArgumentKind::Mandatory, ArgumentValue::Boolean(true)),
            Some(Rule::ArgumentValueType),
        ),
        (&paired, pair('(', ')'), None),
        (&paired, pair('[', ']'), None),
        (&paired, pair('(', ']'), Some(Rule::PairedDelimiter)),
    ] {
        let result = check_arguments(&[argument], signature(spec.clone()), &kb);
        assert_eq!(result.err().map(|error| error.rule), expected, "{spec:?}");
    }
}

#[test]
fn parser_examples_cover_all_structural_contexts() {
    let kb = KnowledgeBase::default();
    for source in [
        r"\frac{x_i^2}{\sqrt[3]{y}}",
        r"\operatorname*{arg\,max}_{x}",
        r"\left\{a\middle|b\right\}",
        r"\begin{array}{c|c}a&b\\c&d\end{array}",
        r"\text{a $x_i$ b}",
        r"{a\over b}",
        r"x''+f^{'2}",
        r"\bf x",
    ] {
        parsed(source, &kb);
    }
}

#[test]
fn local_checks_reject_illegal_script_and_body_shapes() {
    let kb = KnowledgeBase::default();
    let mut ast = Ast::new();
    let base = ast.new_node(Node::Char('x'));
    let script = ast.new_node(Node::Char('i'));
    let scripted = ast.new_node(Node::Scripted {
        base,
        subscript: Some(script),
        superscript: None,
    });
    for (node, expected) in [
        (
            Node::Scripted {
                base,
                subscript: None,
                superscript: None,
            },
            Rule::ScriptedWithoutScript,
        ),
        (
            Node::Scripted {
                base: scripted,
                subscript: None,
                superscript: Some(script),
            },
            Rule::InvalidScriptedBase,
        ),
        (
            Node::Environment {
                name: "unknownenv".into(),
                args: vec![],
                known: false,
                body: scripted,
            },
            Rule::EnvironmentBodyMode,
        ),
    ] {
        let result = check_node(&node, ContentMode::Math, &kb, |id| (ast.node(id), None));
        assert_eq!(rule(result), expected);
    }
}

#[test]
fn escaped_brace_delimiter_records_are_valid_in_delimiter_positions() {
    let kb = KnowledgeBase::builder()
        .item(DelimiterControlItem::new("{"))
        .item(DelimiterControlItem::new("}"))
        .build()
        .unwrap();
    parsed(r"\left\{x\right\}", &kb);
    assert!(!valid_command_name("{"));
    assert!(valid_control_name("{"));
}

#[test]
fn known_text_begin_uses_the_parser_prefix_fallback() {
    let kb = KnowledgeBase::builder()
        .item(CommandItem::new(
            "begin",
            CommandKind::Prefix,
            AllowedMode::Text,
            "",
        ))
        .build()
        .unwrap();
    parsed(r"\text{\begin}", &kb);
    assert_eq!(
        rule(check_leaf(
            &command("begin", 0, false),
            ContentMode::Text,
            &KnowledgeBase::default(),
        )),
        Rule::InvalidName
    );
}
