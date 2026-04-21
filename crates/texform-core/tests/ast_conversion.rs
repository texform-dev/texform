use texform_core::ast::{
    ArgumentKind, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind, Node, NodeId, NodeKind,
    ParentLink, Slot,
};
use texform_core::parse::{
    AllowedMode, CommandItem, CommandKind, ContextItem, DelimiterControlItem, EnvironmentItem,
    ParseContext, ParseContextBuilder,
};
use texform_interface::syntax_node::{ArgumentValue as SyntaxArgumentValue, SyntaxNode};

fn parse_with_items(src: &str, strict: bool, items: Vec<ContextItem>) -> SyntaxNode {
    let mut builder = ParseContextBuilder::new().empty();
    for item in items {
        builder = builder.insert_item(item);
    }
    let ctx = builder
        .build()
        .expect("test items should have valid xparse specs");

    let output = ctx.parse(src, strict);
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );

    output
        .result
        .unwrap_or_else(|| panic!("expected parse result"))
        .node
}

fn command_item(
    name: &str,
    kind: CommandKind,
    allowed_mode: AllowedMode,
    spec: &str,
) -> ContextItem {
    CommandItem::new(name, kind, allowed_mode, spec).into()
}

fn environment_item(
    name: &str,
    allowed_mode: AllowedMode,
    body_mode: ContentMode,
    spec: &str,
) -> ContextItem {
    EnvironmentItem::new(name, allowed_mode, body_mode, spec).into()
}

fn delimiter_item(name: &str) -> ContextItem {
    DelimiterControlItem::new(name).into()
}

fn first_root_child(ast: &Ast) -> NodeId {
    ast.children(ast.root())[0]
}

fn argument_conversion_items() -> Vec<ContextItem> {
    vec![
        command_item("delim", CommandKind::Prefix, AllowedMode::Math, "m:D"),
        command_item("hspace", CommandKind::Prefix, AllowedMode::Both, "m:L"),
        command_item(
            "romannumeral",
            CommandKind::Prefix,
            AllowedMode::Both,
            "m:I",
        ),
        command_item(
            "includegraphics",
            CommandKind::Prefix,
            AllowedMode::Both,
            "o:K m:T",
        ),
        command_item("label", CommandKind::Prefix, AllowedMode::Both, "m:N"),
        command_item(
            "qty",
            CommandKind::Prefix,
            AllowedMode::Math,
            "d<(,)><[,]><{,}><|,|>",
        ),
        command_item("pqty", CommandKind::Prefix, AllowedMode::Math, "s r{}"),
    ]
}

#[test]
fn test_conversion_preserves_unknown_command_and_active_space() {
    let syntax = parse_with_items(r"a~\mystery", false, vec![]);
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    assert_eq!(ast.kind(ast.root()), NodeKind::Root);
    match ast.node(ast.root()) {
        Node::Root { children, mode } => {
            assert_eq!(*mode, ContentMode::Math);
            assert_eq!(children.len(), 3);
        }
        other => panic!("Expected root node, got {:?}", other),
    }

    let children = ast.children(ast.root());
    assert_eq!(children.len(), 3);
    assert_eq!(ast.node(children[0]), &Node::Char('a'));
    assert_eq!(ast.node(children[1]), &Node::ActiveSpace);
    assert_eq!(
        ast.node(children[2]),
        &Node::Command {
            name: "mystery".to_string(),
            args: vec![],
            known: false,
        }
    );
}

#[test]
fn test_ast_conversion_preserves_unknown_environment_known_flag() {
    let syntax = parse_with_items(r"\begin{foo}a\end{foo}", false, vec![]);
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let children = ast.children(ast.root());
    match ast.node(children[0]) {
        Node::Environment {
            name, args, known, ..
        } => {
            assert_eq!(name, "foo");
            assert!(args.is_empty());
            assert!(!known);
        }
        other => panic!("Expected environment node, got {:?}", other),
    }
}

#[test]
fn test_ast_conversion_copies_text_content_variant() {
    let output = ParseContext::all_packages_shared().parse(r"\text{\%}", true);
    let syntax = output
        .result
        .as_ref()
        .expect("expected parse result")
        .node
        .clone();

    match &syntax {
        SyntaxNode::Root { children, .. } => match &children[0] {
            SyntaxNode::Command { args, .. } => {
                let arg = args[0].as_ref().expect("expected text argument");
                assert_eq!(
                    arg.value,
                    SyntaxArgumentValue::TextContent(SyntaxNode::Char('%'))
                );
            }
            other => panic!("expected text command, got {:?}", other),
        },
        other => panic!("expected root node, got {:?}", other),
    }

    let ast = Ast::from_syntax_root(&syntax);
    let root = ast.root();
    let command = ast.children(root)[0];

    match ast.node(command) {
        Node::Command { args, .. } => {
            let arg = args[0].as_ref().expect("expected text argument");
            assert!(matches!(arg.value, ArgumentValue::TextContent(_)));
        }
        other => panic!("expected command node, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_scalar_argument_values() {
    let syntax = parse_with_items(
        r"\delim( \hspace1em \romannumeral12 \label{sec:intro}",
        true,
        argument_conversion_items(),
    );
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let children = ast.children(ast.root());
    assert_eq!(children.len(), 4);

    match ast.node(children[0]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "delim");
            let arg = args[0].as_ref().unwrap();
            assert_eq!(arg.kind, ArgumentKind::Mandatory);
            assert_eq!(arg.value, ArgumentValue::Delimiter(Delimiter::Char('(')));
            assert!(ast.edges(children[0]).is_empty());
        }
        other => panic!("Expected delim command, got {:?}", other),
    }

    match ast.node(children[1]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "hspace");
            let arg = args[0].as_ref().unwrap();
            assert_eq!(arg.kind, ArgumentKind::Mandatory);
            assert_eq!(arg.value, ArgumentValue::Dimension("1em".to_string()));
            assert!(ast.edges(children[1]).is_empty());
        }
        other => panic!("Expected hspace command, got {:?}", other),
    }

    match ast.node(children[2]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "romannumeral");
            let arg = args[0].as_ref().unwrap();
            assert_eq!(arg.kind, ArgumentKind::Mandatory);
            assert_eq!(arg.value, ArgumentValue::Integer("12".to_string()));
            assert!(ast.edges(children[2]).is_empty());
        }
        other => panic!("Expected romannumeral command, got {:?}", other),
    }

    match ast.node(children[3]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "label");
            let arg = args[0].as_ref().unwrap();
            assert_eq!(arg.value, ArgumentValue::CSName("sec:intro".to_string()));
            assert!(ast.edges(children[3]).is_empty());
        }
        other => panic!("Expected label command, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_content_argument_edges() {
    let syntax = parse_with_items(
        r"\includegraphics[width=1em]{file} \qty|x| \pqty*{y}",
        true,
        argument_conversion_items(),
    );
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let children = ast.children(ast.root());
    assert_eq!(children.len(), 3);

    match ast.node(children[0]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "includegraphics");
            assert_eq!(args.len(), 2);
            assert_eq!(
                args[0].as_ref().unwrap().value,
                ArgumentValue::KeyVal("width=1em".to_string())
            );

            let text = match &args[1].as_ref().unwrap().value {
                ArgumentValue::TextContent(id) => *id,
                other => panic!("Expected content argument, got {:?}", other),
            };
            assert_eq!(ast.edges(children[0]), vec![(text, Slot::Argument(1))]);
            assert_eq!(
                ast.parent(text),
                Some(ParentLink {
                    parent: children[0],
                    slot: Slot::Argument(1),
                })
            );
            assert_eq!(ast.node(text), &Node::Text("file".to_string()));
        }
        other => panic!("Expected includegraphics command, got {:?}", other),
    }

    match ast.node(children[1]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "qty");
            let arg = args[0].as_ref().unwrap();

            match &arg.kind {
                ArgumentKind::Paired { open, close } => {
                    assert_eq!(open, &Delimiter::Char('|'));
                    assert_eq!(close, &Delimiter::Char('|'));
                }
                other => panic!("Expected paired argument kind, got {:?}", other),
            }

            let content = match &arg.value {
                ArgumentValue::MathContent(id) => *id,
                other => panic!("Expected content argument, got {:?}", other),
            };
            assert_eq!(ast.node(content), &Node::Char('x'));
            assert_eq!(
                ast.parent(content),
                Some(ParentLink {
                    parent: children[1],
                    slot: Slot::Argument(0),
                })
            );
        }
        other => panic!("Expected qty command, got {:?}", other),
    }

    match ast.node(children[2]) {
        Node::Command { name, args, .. } => {
            assert_eq!(name, "pqty");
            assert_eq!(args.len(), 2);
            assert_eq!(
                args[0].as_ref().unwrap().value,
                ArgumentValue::Boolean(true)
            );

            let delimited = args[1].as_ref().unwrap();
            match &delimited.kind {
                ArgumentKind::Delimited { open, close } => {
                    assert_eq!(open, &Delimiter::Char('{'));
                    assert_eq!(close, &Delimiter::Char('}'));
                }
                other => panic!("Expected brace-delimited argument, got {:?}", other),
            }

            let content = match &delimited.value {
                ArgumentValue::MathContent(id) => *id,
                other => panic!("Expected content argument, got {:?}", other),
            };
            assert_eq!(ast.node(content), &Node::Char('y'));
            assert_eq!(
                ast.parent(content),
                Some(ParentLink {
                    parent: children[2],
                    slot: Slot::Argument(1),
                })
            );
        }
        other => panic!("Expected pqty command, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_scripted_structure() {
    let syntax = parse_with_items("x_i", true, vec![]);
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let scripted = first_root_child(&ast);
    match ast.node(scripted) {
        Node::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_eq!(ast.node(*base), &Node::Char('x'));
            assert_eq!(*superscript, None);

            let subscript = subscript.unwrap();
            assert_eq!(ast.node(subscript), &Node::Char('i'));
            assert_eq!(
                ast.parent(*base),
                Some(ParentLink {
                    parent: scripted,
                    slot: Slot::ScriptBase,
                })
            );
            assert_eq!(
                ast.parent(subscript),
                Some(ParentLink {
                    parent: scripted,
                    slot: Slot::ScriptSub,
                })
            );
        }
        other => panic!("Expected scripted node, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_infix_structure() {
    let syntax = parse_with_items(
        r"a \over b",
        true,
        vec![command_item(
            "over",
            CommandKind::Infix,
            AllowedMode::Math,
            "",
        )],
    );
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let infix = first_root_child(&ast);
    match ast.node(infix) {
        Node::Infix {
            name,
            args,
            left,
            right,
        } => {
            assert_eq!(name, "over");
            assert!(args.is_empty());
            assert_eq!(ast.node(*left), &Node::Char('a'));
            assert_eq!(ast.node(*right), &Node::Char('b'));
            assert_eq!(
                ast.parent(*left),
                Some(ParentLink {
                    parent: infix,
                    slot: Slot::InfixLeft,
                })
            );
            assert_eq!(
                ast.parent(*right),
                Some(ParentLink {
                    parent: infix,
                    slot: Slot::InfixRight,
                })
            );
        }
        other => panic!("Expected infix node, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_flat_declarative_and_environment_structure() {
    let declarative = parse_with_items(
        r"\bfseries bc",
        true,
        vec![command_item(
            "bfseries",
            CommandKind::Declarative,
            AllowedMode::Both,
            "",
        )],
    );
    let declarative_ast = Ast::from_syntax_root(&declarative);
    declarative_ast.assert_invariants();

    let decl = first_root_child(&declarative_ast);
    match declarative_ast.node(decl) {
        Node::Declarative { name, args } => {
            assert_eq!(name, "bfseries");
            assert!(args.is_empty());
            assert!(declarative_ast.edges(decl).is_empty());
        }
        other => panic!("Expected declarative node, got {:?}", other),
    }

    let environment = parse_with_items(
        r"\begin{matrix}x\end{matrix}",
        true,
        vec![environment_item(
            "matrix",
            AllowedMode::Math,
            ContentMode::Math,
            "",
        )],
    );
    let environment_ast = Ast::from_syntax_root(&environment);
    environment_ast.assert_invariants();

    let env = first_root_child(&environment_ast);
    match environment_ast.node(env) {
        Node::Environment {
            name, args, body, ..
        } => {
            assert_eq!(name, "matrix");
            assert!(args.is_empty());
            assert_eq!(
                environment_ast.parent(*body),
                Some(ParentLink {
                    parent: env,
                    slot: Slot::EnvBody,
                })
            );

            match environment_ast.node(*body) {
                Node::Group {
                    children,
                    kind,
                    mode,
                } => {
                    assert_eq!(kind, &GroupKind::Implicit);
                    assert_eq!(mode, &ContentMode::Math);
                    assert_eq!(children.len(), 1);
                    assert_eq!(environment_ast.node(children[0]), &Node::Char('x'));
                }
                other => panic!("Expected environment body group, got {:?}", other),
            }
        }
        other => panic!("Expected environment node, got {:?}", other),
    }
}

#[test]
fn test_conversion_preserves_control_delimited_groups() {
    let syntax = parse_with_items(
        r"\left\langle x \right\rangle",
        true,
        vec![delimiter_item("langle"), delimiter_item("rangle")],
    );
    let ast = Ast::from_syntax_root(&syntax);
    ast.assert_invariants();

    let group = first_root_child(&ast);
    match ast.node(group) {
        Node::Group {
            children,
            kind,
            mode,
        } => {
            assert_eq!(mode, &ContentMode::Math);
            match kind {
                GroupKind::Delimited { left, right } => {
                    assert_eq!(left, &Delimiter::Control("langle".to_string()));
                    assert_eq!(right, &Delimiter::Control("rangle".to_string()));
                }
                other => panic!("Expected delimited group, got {:?}", other),
            }
            assert_eq!(children.len(), 1);
            assert_eq!(ast.node(children[0]), &Node::Char('x'));
        }
        other => panic!("Expected converted group node, got {:?}", other),
    }
}

#[test]
#[should_panic(expected = "Ast::from_syntax_root expects SyntaxNode::Root")]
fn test_from_syntax_root_rejects_non_root_node() {
    let node = SyntaxNode::implicit_group(ContentMode::Math, vec![SyntaxNode::Char('x')]);
    let _ = Ast::from_syntax_root(&node);
}

#[test]
#[should_panic(expected = "Ast::from_syntax_root does not accept nested SyntaxNode::Root")]
fn test_from_syntax_root_rejects_nested_root_node() {
    let node = SyntaxNode::root(
        ContentMode::Math,
        vec![SyntaxNode::root(
            ContentMode::Math,
            vec![SyntaxNode::Char('x')],
        )],
    );

    let _ = Ast::from_syntax_root(&node);
}
