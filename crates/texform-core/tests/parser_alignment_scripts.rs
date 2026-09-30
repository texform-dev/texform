use texform_core::document::Document;
use texform_core::parse::{KnowledgeBase, ParseConfig};
use texform_interface::syntax_node::{GroupKind, SyntaxNode};

fn parse(source: &str) -> Document {
    let (doc, diagnostics) = KnowledgeBase::default()
        .parse(source, &ParseConfig::STRICT)
        .try_into_document()
        .unwrap_or_else(|error| panic!("{source}: {error:?}"));
    assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    Document::from_syntax(&doc.to_syntax()).expect("complete parser output must conform");
    doc
}

fn assert_cell_start(children: &[SyntaxNode]) {
    assert_eq!(children[0], SyntaxNode::AlignmentTab);
    let SyntaxNode::Scripted { base, .. } = &children[1] else {
        panic!("expected a scripted expression after the alignment tab: {children:?}");
    };
    assert!(
        matches!(base.as_ref(), SyntaxNode::Group { kind: GroupKind::Implicit, children, .. } if children.is_empty())
    );
}

#[test]
fn scripts_after_alignment_tabs_start_a_new_cell_with_an_empty_base() {
    for source in [
        "&^2", "&_1", "&'", "&''_1", "&''^2_1", " & ^2 ", " & _1 ", " & ' '", "& ' '_1",
    ] {
        let doc = parse(source);
        let SyntaxNode::Root { children, .. } = doc.to_syntax() else {
            unreachable!()
        };
        assert_eq!(children.len(), 2, "{source}: {children:?}");
        assert_cell_start(&children);
        let latex = doc.to_latex().unwrap();
        assert_eq!(parse(&latex).to_latex().unwrap(), latex, "{source}");
    }
}

#[test]
fn alignment_environment_keeps_separators_before_empty_base_scripts() {
    let source = r"\begin{array}{ccccc} & _1 & _1 & _1 \\ & & & 1 & 1 \,_2 \\ + & & 1 & 1 & 1 \,_2 \\\hline & 1 & 0 & 1 & 0 \,_2 \end{array}";
    let doc = parse(source);
    let SyntaxNode::Root { children, .. } = doc.to_syntax() else {
        unreachable!()
    };
    let SyntaxNode::Environment { body, .. } = &children[0] else {
        unreachable!()
    };
    let SyntaxNode::Group { children, .. } = body.as_ref() else {
        unreachable!()
    };
    assert_cell_start(children);
    assert_cell_start(&children[2..]);
    assert_cell_start(&children[4..]);
    let latex = doc.to_latex().unwrap();
    assert_eq!(parse(&latex).to_latex().unwrap(), latex);
}

#[test]
fn literal_ampersand_remains_a_valid_script_base() {
    let doc = parse(r"\&^2");
    let SyntaxNode::Root { children, .. } = doc.to_syntax() else {
        unreachable!()
    };
    assert!(
        matches!(&children[0], SyntaxNode::Scripted { base, .. } if **base == SyntaxNode::Char('&'))
    );
}
