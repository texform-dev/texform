use serde_json::json;
use texform_core::document::{ColumnarTree, Document};
use texform_core::parse::{AllowedMode, ContentMode, EnvironmentItem, KnowledgeBase, ParseConfig};
use texform_interface::syntax_node::{Argument, ArgumentKind, ArgumentValue, SyntaxNode};

fn parse(knowledge: &KnowledgeBase, source: &str) -> Document {
    let result = knowledge.parse(source, &ParseConfig::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.document.unwrap()
}

/// Row of the first `value` in a column.
fn row<T: PartialEq + std::fmt::Debug>(column: &[T], value: T) -> usize {
    column
        .iter()
        .position(|cell| *cell == value)
        .unwrap_or_else(|| panic!("no row with {value:?}"))
}

fn check_columns(tables: &ColumnarTree) {
    let value = serde_json::to_value(tables).unwrap();
    assert_eq!(value["nodes"].as_object().unwrap().len(), 12);
    assert_eq!(value["args"].as_object().unwrap().len(), 9);
    for (table, count) in [
        ("nodes", tables.nodes.kind.len()),
        ("args", tables.args.owner.len()),
    ] {
        for column in value[table].as_object().unwrap().values() {
            assert_eq!(column.as_array().unwrap().len(), count);
        }
    }
    for (row, parent) in tables.nodes.parent.iter().enumerate().skip(1) {
        assert!(*parent >= 0 && (*parent as usize) < row);
        assert_eq!(
            tables.nodes.depth[row],
            tables.nodes.depth[*parent as usize] + 1
        );
    }
    for (argument, content) in tables.args.content.iter().enumerate() {
        if *content >= 0 {
            let node = *content as usize;
            assert_eq!(
                tables.nodes.parent[node],
                tables.args.owner[argument] as i64
            );
            assert_eq!(tables.nodes.slot[node], Some("arg"));
            assert_eq!(
                tables.nodes.slot_index[node],
                tables.args.index[argument] as i64
            );
        }
    }
}

#[test]
fn root_snapshot_excludes_detached_nodes_and_uses_sentinel_indices() {
    let mut doc = Document::new();
    doc.create_char('x').unwrap();
    assert_eq!(
        serde_json::to_value(doc.to_columnar()).unwrap(),
        json!({
            "nodes": {"kind":["Root"],"parent":[-1],"slot":[null],"slot_index":[-1],
                "depth":[0],"mode":["math"],"name":[null],"value":[null],"known":[null],
                "group_kind":[null],"left":[null],"right":[null]},
            "args": {"owner":[],"index":[],"form":[],"present":[],"value_kind":[],
                "value":[],"content":[],"open":[],"close":[]}
        })
    );
}

#[test]
fn tables_preserve_absent_slots_star_operator_names_and_scalar_delimiters() {
    let doc = parse(
        &KnowledgeBase::default(),
        r"\sqrt{x}+\operatorname{sn}+\big(+\text{hi}+\begin{array}{c}a\end{array}",
    );
    let tables = doc.to_columnar();
    check_columns(&tables);
    let name = |name: &str| row(&tables.nodes.name, Some(name.to_owned()));
    // Absent command and environment slots take their form from the signature.
    for owner in [name("sqrt"), name("array")] {
        let optional = row(&tables.args.owner, owner);
        assert_eq!(tables.args.form[optional], Some("optional"));
        assert!(!tables.args.present[optional]);
        assert_eq!(tables.args.value_kind[optional], None);
        assert_eq!(tables.args.content[optional], -1);
    }
    let star = row(&tables.args.form, Some("star"));
    assert!(tables.args.present[star]);
    assert_eq!(tables.args.value_kind[star], Some("boolean"));
    assert_eq!(tables.args.value[star].as_deref(), Some("false"));
    for (kind, mode) in [("operator_name", "math"), ("text", "text")] {
        let content = tables.args.content[row(&tables.args.value_kind, Some(kind))];
        assert_eq!(tables.nodes.mode[content as usize], mode);
    }
    let delimiter = row(&tables.args.value_kind, Some("delimiter"));
    assert_eq!(tables.args.value[delimiter].as_deref(), Some("("));
    assert_eq!(tables.args.content[delimiter], -1);
}

#[test]
fn context_modes_follow_environment_bodies_and_inline_math_boundaries() {
    let knowledge = KnowledgeBase::builder()
        .item(EnvironmentItem::new(
            "textenv",
            AllowedMode::Math,
            ContentMode::Text,
            "",
        ))
        .build()
        .unwrap();
    let doc = parse(&knowledge, r"\begin{textenv}hi $x$\end{textenv}");
    let tables = doc.to_columnar();
    check_columns(&tables);
    assert_eq!(
        tables.nodes.mode[row(&tables.nodes.slot, Some("env_body"))],
        "text"
    );
    let inline = row(&tables.nodes.group_kind, Some("inline_math"));
    assert_eq!(tables.nodes.mode[inline], "text");
    assert_eq!(tables.nodes.mode[inline + 1], "math");
}

#[test]
fn paired_arguments_export_selected_boundaries_and_infix_preorder() {
    let knowledge = KnowledgeBase::builder()
        .packages(&["base", "ams", "physics"])
        .build()
        .unwrap();
    let doc = parse(&knowledge, r"{a\over b}+\qty[x]+\left\langle y\right.");
    let tables = doc.to_columnar();
    check_columns(&tables);
    let paired = tables
        .args
        .form
        .iter()
        .zip(&tables.args.present)
        .position(|(form, present)| *form == Some("paired") && *present)
        .unwrap();
    assert_eq!(tables.args.open[paired].as_deref(), Some("["));
    assert_eq!(tables.args.close[paired].as_deref(), Some("]"));
    let infix = row(&tables.nodes.kind, "Infix");
    assert_eq!(tables.nodes.slot[infix + 1], Some("infix_left"));
    let right = row(&tables.nodes.slot, Some("infix_right"));
    assert_eq!(tables.nodes.parent[right], infix as i64);
    let group = row(&tables.nodes.group_kind, Some("delimited"));
    assert_eq!(tables.nodes.left[group].as_deref(), Some("\\langle"));
    assert_eq!(tables.nodes.right[group].as_deref(), Some("."));
}

#[test]
fn until_arguments_export_only_their_terminator() {
    let doc = parse(&KnowledgeBase::default(), r"\root n\of x");
    let tables = doc.to_columnar();
    check_columns(&tables);
    let until = row(&tables.args.form, Some("until"));
    assert_eq!(tables.args.open[until], None);
    assert_eq!(tables.args.close[until].as_deref(), Some(r"\of"));
}

#[test]
fn incomplete_documents_export_unknown_empty_slots_and_all_scalar_payloads() {
    let values = [
        ArgumentValue::CSName("eq:one".into()),
        ArgumentValue::Dimension("1pt".into()),
        ArgumentValue::Integer("-2".into()),
        ArgumentValue::KeyVal("a=b".into()),
        ArgumentValue::Column("lc".into()),
        ArgumentValue::Boolean(true),
    ];
    let mut args = vec![None];
    args.extend(values.into_iter().map(|value| {
        Some(Argument {
            kind: ArgumentKind::Mandatory,
            no_leading_space: false,
            value,
        })
    }));
    let syntax = SyntaxNode::Root {
        mode: ContentMode::Math,
        children: vec![
            SyntaxNode::Command {
                name: "missing".into(),
                known: false,
                args,
            },
            SyntaxNode::Error {
                message: "broken source".into(),
                snippet: "???".into(),
            },
        ],
    };
    let doc = Document::from_syntax(&syntax).unwrap();
    assert!(doc.has_errors());
    let tables = doc.to_columnar();
    check_columns(&tables);
    assert_eq!(tables.nodes.value[2].as_deref(), Some("???"));
    assert_eq!(tables.nodes.known[1], Some(false));
    assert_eq!(tables.args.form[0], None);
    assert_eq!(
        tables.args.value_kind,
        [
            None,
            Some("cs_name"),
            Some("dimension"),
            Some("integer"),
            Some("key_val"),
            Some("column"),
            Some("boolean")
        ]
    );
    assert_eq!(
        tables.args.value,
        [
            None,
            Some("eq:one".into()),
            Some("1pt".into()),
            Some("-2".into()),
            Some("a=b".into()),
            Some("lc".into()),
            Some("true".into())
        ]
    );
}

#[test]
fn scripted_rows_preserve_base_subscript_superscript_and_prime_count() {
    let doc = parse(&KnowledgeBase::default(), "f_i''");
    let tables = doc.to_columnar();
    check_columns(&tables);
    let scripted = row(&tables.nodes.kind, "Scripted");
    let slots: Vec<_> = tables
        .nodes
        .parent
        .iter()
        .enumerate()
        .filter(|(_, parent)| **parent == scripted as i64)
        .map(|(row, _)| tables.nodes.slot[row])
        .collect();
    assert_eq!(
        slots,
        [Some("script_base"), Some("subscript"), Some("superscript")]
    );
    let prime = row(&tables.nodes.kind, "Prime");
    assert_eq!(tables.nodes.value[prime].as_deref(), Some("2"));
}
