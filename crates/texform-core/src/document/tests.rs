use super::*;

#[cfg(test)]
impl Document {
    fn from_ast_for_test(build: impl FnOnce(&mut Ast)) -> Self {
        let mut ast = Ast::new();
        build(&mut ast);
        ast.assert_invariants();
        Document {
            ast,
            spans: SecondaryMap::new(),
            detached_modes: SecondaryMap::new(),
            has_errors: false,
            id: next_document_id(),
            knowledge_base: KnowledgeBase::default(),
        }
    }

    fn from_ast_with_errors_for_test(build: impl FnOnce(&mut Ast)) -> Self {
        let mut ast = Ast::new();
        build(&mut ast);
        ast.assert_invariants();
        let has_errors = ast.contains_error();
        Document {
            ast,
            spans: SecondaryMap::new(),
            detached_modes: SecondaryMap::new(),
            has_errors,
            id: next_document_id(),
            knowledge_base: KnowledgeBase::default(),
        }
    }
}

#[cfg(test)]
mod behavior {
    use super::*;

    #[test]
    fn new_document_is_empty_and_editable() {
        let doc = Document::new();
        assert_eq!(doc.root().kind(), NodeKind::Root);
        assert!(!doc.has_errors());
        assert!(!doc.is_read_only());
        assert_eq!(doc.errors().count(), 0);
    }

    #[test]
    fn document_ids_are_unique_across_construction_and_clone() {
        let a = Document::new();
        let b = Document::new();
        assert_ne!(a.id(), b.id());

        let c = a.clone();
        assert_ne!(a.id(), c.id());
    }

    #[test]
    fn node_ref_reads_synthesized_chars() {
        let doc = Document::from_ast_for_test(|ast| {
            let a = ast.new_node(Node::Char('a'));
            let b = ast.new_node(Node::Char('b'));
            ast.append_child(ast.root(), a);
            ast.append_child(ast.root(), b);
        });

        let mut children = doc.root().children();
        let first = children.next().unwrap();
        let second = children.next().unwrap();
        assert!(first.is_char('a'));
        assert!(second.is_char('b'));
        assert_eq!(first.next_sibling().map(|n| n.id()), Some(second.id()));
        assert_eq!(second.prev_sibling().map(|n| n.id()), Some(first.id()));
        assert_eq!(doc.root().children().count(), 2);
    }

    #[test]
    fn find_all_collects_matching_nodes() {
        let doc = Document::from_ast_for_test(|ast| {
            let a = ast.new_node(Node::Char('a'));
            let group = ast.new_node(Node::Group {
                children: vec![a],
                kind: crate::ast::GroupKind::Explicit,
                mode: ContentMode::Math,
            });
            let b = ast.new_node(Node::Char('b'));
            ast.append_child(ast.root(), group);
            ast.append_child(ast.root(), b);
        });

        let chars: Vec<_> = doc
            .find_all(doc.root(), |node| node.char().is_some())
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn edit_error_implements_display_and_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<EditError>();

        assert_eq!(
            EditError::CannotEditRoot.to_string(),
            "cannot edit the root node"
        );
        assert_eq!(
            EditError::SlotShapeMismatch { expected: "group" }.to_string(),
            "slot shape mismatch: expected group"
        );
    }

    #[test]
    fn create_and_append_children() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();

        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn insert_before_and_after() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let c = doc.create_char('c').unwrap();
        let b = doc.create_char('b').unwrap();
        let d = doc.create_char('d').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, c).unwrap();

        doc.insert_before(c, b).unwrap();
        doc.insert_after(c, d).unwrap();

        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b', 'c', 'd']);
    }

    #[test]
    fn remove_and_extract() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        let extracted = doc.extract(a).unwrap();
        assert_eq!(extracted, a);
        doc.remove(b).unwrap();

        assert_eq!(doc.root().children().count(), 0);
        doc.append_child(root, extracted).unwrap();
        assert_eq!(
            doc.root().children().next().and_then(|node| node.char()),
            Some('a')
        );
    }

    #[test]
    fn replace_with_swaps_node() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();

        doc.replace_with(a, b).unwrap();

        assert_eq!(
            doc.root().children().next().and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn set_text_and_char_and_command_name() {
        let mut doc = Document::new();
        let text = doc.in_mode(ContentMode::Text).create_text("old").unwrap();
        let ch = doc.create_char('a').unwrap();
        let cmd = doc.create_command("alpha", Vec::new()).unwrap();

        doc.set_text(text, "new").unwrap();
        doc.set_char(ch, 'b').unwrap();
        doc.set_command_name(cmd, "beta").unwrap();

        assert_eq!(doc.node(text).unwrap().text(), Some("new"));
        assert_eq!(doc.node(ch).unwrap().char(), Some('b'));
        assert_eq!(doc.node(cmd).unwrap().command_name(), Some("beta"));
    }

    #[test]
    fn cannot_edit_root() {
        let mut doc = Document::new();
        let root = doc.root().id();
        assert_eq!(doc.remove(root), Err(EditError::CannotEditRoot));
        assert_eq!(doc.extract(root), Err(EditError::CannotEditRoot));
    }

    #[test]
    fn append_to_non_container_fails() {
        let mut doc = Document::new();
        let parent = doc.create_char('a').unwrap();
        let child = doc.create_char('b').unwrap();

        assert_eq!(
            doc.append_child(parent, child),
            Err(EditError::NotAContainer)
        );
    }

    #[test]
    fn create_command_with_args_and_read_back() {
        let mut doc = Document::new();
        let numerator = doc.create_char('a').unwrap();
        let denominator = doc.create_char('b').unwrap();
        let frac = doc
            .create_command("frac", vec![Arg::Node(numerator), Arg::Node(denominator)])
            .unwrap();

        let node = doc.node(frac).unwrap();
        assert_eq!(node.command_name(), Some("frac"));
        assert_eq!(node.arg_count(), 2);
        assert_eq!(
            node.arg(0)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('a')
        );
        assert_eq!(
            node.arg(1)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn duplicate_child_is_rejected() {
        let mut doc = Document::new();
        let child = doc.create_char('x').unwrap();

        assert_eq!(
            doc.create_command("frac", vec![Arg::Node(child), Arg::Node(child)]),
            Err(EditError::DuplicateChild)
        );
    }

    #[test]
    fn failed_operations_remove_staged_sources() {
        let mut doc = KnowledgeBase::default()
            .parse(r"\begin{matrix}x\end{matrix}", &Default::default())
            .try_into_document()
            .unwrap()
            .0;
        let nodes = doc.ast.node_count();
        // Each operation parses a source input before a later check fails.
        doc.create_command("frac", ["y".into(), Arg::Star(true)])
            .unwrap_err();
        doc.create_environment_with_children("unknownenv", ["a".into()], vec!["x".into()])
            .unwrap_err();
        // Spliced sources are staged node by node before the group check.
        doc.create_group(ContentMode::Math, ["a".into(), r"b \over c".into()])
            .unwrap_err();
        assert_eq!(doc.ast.node_count(), nodes);
    }

    #[test]
    fn set_arg_replaces_content() {
        let mut doc = Document::new();
        let old = doc.create_char('a').unwrap();
        let cmd = doc.create_command("sqrt", vec![Arg::Node(old)]).unwrap();
        let new = doc.create_char('b').unwrap();

        doc.set_arg(cmd, 1, Arg::Node(new)).unwrap();

        let node = doc.node(cmd).unwrap();
        assert_eq!(
            node.arg(1)
                .and_then(|arg| arg.as_node())
                .and_then(|node| node.char()),
            Some('b')
        );
    }

    #[test]
    fn set_arg_preserves_optional_slot_kind() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "sqrt".to_string(),
                args: vec![
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Optional,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('a')),
                    }),
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Mandatory,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('x')),
                    }),
                ],
                known: true,
            }],
        };
        let mut doc = Document::from_syntax(&syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();
        let replacement = doc.create_char('b').unwrap();

        doc.set_arg(command, 0, Arg::Node(replacement)).unwrap();

        assert_eq!(doc.to_latex().unwrap(), r"\sqrt [ b ] { x }");
    }

    #[test]
    fn set_arg_preserves_no_leading_space_flag() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "probe".to_string(),
                args: vec![Some(SyntaxArgument {
                    kind: SyntaxArgumentKind::Optional,
                    no_leading_space: true,
                    value: SyntaxArgumentValue::MathContent(SyntaxNode::Char('a')),
                })],
                known: true,
            }],
        };
        let knowledge = KnowledgeBase::builder()
            .item(crate::parse::CommandItem::new(
                "probe",
                crate::parse::CommandKind::Prefix,
                crate::parse::AllowedMode::Math,
                "!o",
            ))
            .build()
            .unwrap();
        let mut doc = Document::from_syntax_with(&knowledge, &syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();
        let replacement = doc.create_char('b').unwrap();

        doc.set_arg(command, 0, Arg::Node(replacement)).unwrap();

        let roundtrip = doc.to_syntax();
        let SyntaxNode::Root { children, .. } = roundtrip else {
            panic!("expected root");
        };
        let SyntaxNode::Command { args, .. } = &children[0] else {
            panic!("expected command");
        };

        assert!(args[0].as_ref().unwrap().no_leading_space);
    }

    #[test]
    fn set_arg_preserves_star_boolean_slot_kind() {
        use texform_interface::syntax_node::{
            Argument as SyntaxArgument, ArgumentKind as SyntaxArgumentKind,
            ArgumentValue as SyntaxArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "operatorname".to_string(),
                args: vec![
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Star,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::Boolean(false),
                    }),
                    Some(SyntaxArgument {
                        kind: SyntaxArgumentKind::Mandatory,
                        no_leading_space: false,
                        value: SyntaxArgumentValue::OperatorNameContent(SyntaxNode::Char('x')),
                    }),
                ],
                known: true,
            }],
        };
        let mut doc = Document::from_syntax(&syntax).unwrap();
        let command = doc.root().children().next().unwrap().id();

        doc.set_arg(command, 0, Arg::Star(true)).unwrap();

        assert_eq!(doc.to_latex().unwrap(), r"\operatorname* {x}");
    }

    #[test]
    fn read_only_editing_checks_precede_invalid_node_checks() {
        let mut read_only = Document::from_ast_with_errors_for_test(|ast| {
            let err = ast.new_node(Node::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            });
            ast.append_child(ast.root(), err);
        });
        let mut other = Document::new();
        let foreign = other.create_char('x').unwrap();

        assert_eq!(
            read_only.append_child(foreign, foreign),
            Err(EditError::ReadOnlyDocument)
        );
        assert_eq!(
            read_only.insert_before(foreign, foreign),
            Err(EditError::ReadOnlyDocument)
        );
    }

    #[test]
    fn wrap_moves_target_inside_wrapper() {
        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let wrapper = doc.create_group(ContentMode::Math, []).unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();

        let wrapped = doc.wrap(a, wrapper).unwrap();

        let group = doc.root().children().next().unwrap();
        assert_eq!(group.id(), wrapped);
        assert_eq!(
            group.children().next().and_then(|node| node.char()),
            Some('a')
        );
    }

    #[test]
    fn unwrap_splices_group_children_into_parent() {
        let mut doc = Document::new();
        let group = doc.create_group(ContentMode::Math, []).unwrap();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        doc.append_child(group, a).unwrap();
        doc.append_child(group, b).unwrap();
        let root = doc.root().id();
        doc.append_child(root, group).unwrap();

        let children = doc.unwrap(group).unwrap();

        assert_eq!(children.len(), 2);
        let chars: Vec<_> = doc
            .root()
            .children()
            .filter_map(|node| node.char())
            .collect();
        assert_eq!(chars, vec!['a', 'b']);
    }

    #[test]
    fn error_tree_is_read_only() {
        let doc = Document::from_ast_with_errors_for_test(|ast| {
            let err = ast.new_node(Node::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            });
            ast.append_child(ast.root(), err);
        });
        assert!(doc.has_errors());
        assert!(doc.is_read_only());
        assert_eq!(doc.errors().count(), 1);

        let mut doc = doc;
        assert_eq!(doc.create_char('z'), Err(EditError::ReadOnlyDocument));
    }

    #[test]
    fn from_syntax_round_trips_clean_tree() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert!(!doc.has_errors());

        assert_eq!(doc.to_syntax(), syntax);
    }

    #[test]
    fn from_syntax_rejects_zero_count_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Prime { count: 0 }],
        };

        assert!(matches!(
            Document::from_syntax(&syntax).expect_err("expected invalid prime count"),
            FromSyntaxError::Conformance(ConformanceError {
                rule: ConformanceRule::InvalidPrimeCount,
                ..
            })
        ));
    }

    #[test]
    fn from_syntax_rejects_text_mode_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Text,
            children: vec![SyntaxNode::Prime { count: 1 }],
        };

        assert!(matches!(
            Document::from_syntax(&syntax).expect_err("expected text-mode prime rejection"),
            FromSyntaxError::Conformance(ConformanceError {
                rule: ConformanceRule::ModeMismatch,
                ..
            })
        ));
    }

    #[test]
    fn from_syntax_rejects_text_mode_scripted_prime() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Text,
            children: vec![SyntaxNode::Scripted {
                base: Box::new(SyntaxNode::Prime { count: 1 }),
                subscript: None,
                superscript: None,
            }],
        };

        assert!(matches!(
            Document::from_syntax(&syntax)
                .expect_err("expected text-mode scripted prime rejection"),
            FromSyntaxError::Conformance(ConformanceError {
                rule: ConformanceRule::ModeMismatch,
                ..
            })
        ));
    }

    #[test]
    fn from_syntax_rejects_text_content_scripted_prime() {
        use texform_interface::syntax_node::{
            Argument, ArgumentKind, ArgumentValue, ContentMode as M, SyntaxNode,
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Command {
                name: "text".to_string(),
                args: vec![Some(Argument {
                    kind: ArgumentKind::Mandatory,
                    no_leading_space: false,
                    value: ArgumentValue::TextContent(SyntaxNode::Scripted {
                        base: Box::new(SyntaxNode::Prime { count: 1 }),
                        subscript: None,
                        superscript: None,
                    }),
                })],
                known: true,
            }],
        };

        assert!(matches!(
            Document::from_syntax(&syntax)
                .expect_err("expected text-content scripted prime rejection"),
            FromSyntaxError::Conformance(ConformanceError {
                rule: ConformanceRule::ModeMismatch,
                ..
            })
        ));
    }

    #[test]
    fn from_syntax_marks_error_tree_read_only() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Error {
                message: "bad".to_string(),
                snippet: "x".to_string(),
            }],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert!(doc.has_errors());
        assert!(doc.is_read_only());
    }

    #[test]
    fn from_syntax_alone_has_no_spans() {
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a')],
        };
        let doc = Document::from_syntax(&syntax).unwrap();
        assert_eq!(doc.root().children().next().unwrap().span(), None);
    }

    #[test]
    fn span_mapping_aligns_paths_to_node_ids() {
        use chumsky::prelude::*;
        use texform_interface::syntax_node::{ContentMode as M, SyntaxNode};

        let leaf = |start: usize, end: usize| SpanTree {
            span: SimpleSpan::new((), start..end),
            kids: Vec::new(),
        };

        let syntax = SyntaxNode::Root {
            mode: M::Math,
            children: vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')],
        };
        let span_tree = SpanTree {
            span: SimpleSpan::new((), 0..2),
            kids: vec![leaf(0, 1), leaf(1, 2)],
        };

        let doc = Document::from_syntax_with_spans(&KnowledgeBase::default(), &syntax, &span_tree);
        let mut kids = doc.root().children();
        let a = kids.next().unwrap();
        let b = kids.next().unwrap();
        assert_eq!(a.span(), Some(Span { start: 0, end: 1 }));
        assert_eq!(b.span(), Some(Span { start: 1, end: 2 }));
        assert_eq!(doc.root().span(), Some(Span { start: 0, end: 2 }));
    }

    #[test]
    fn to_latex_default_and_with_options() {
        use crate::serialize::SerializeOptions;

        let mut doc = Document::new();
        let a = doc.create_char('a').unwrap();
        let b = doc.create_char('b').unwrap();
        let root = doc.root().id();
        doc.append_child(root, a).unwrap();
        doc.append_child(root, b).unwrap();

        assert_eq!(doc.to_latex().unwrap(), "a b");
        assert_eq!(format!("{doc}"), "a b");

        let opts = SerializeOptions::default();
        assert_eq!(doc.to_latex_with(&opts).unwrap(), "a b");
    }
}
