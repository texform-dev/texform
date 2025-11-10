//! Immutable syntax tree for LaTeX parsing (Stage 1)
//!
//! This module defines the intermediate representation produced by the parser (chumsky).
//! Unlike the final AST (ast.rs), SyntaxNode uses standard Rust types (Vec, Box)
//! and is optimized for top-down traversal rather than bidirectional navigation.
//!
//! After parsing, the syntax tree is converted to the slotmap-based AST via lowering.

/// Immutable syntax tree node
///
/// Represents the structure of parsed LaTeX source code.
/// Each variant corresponds to a different syntactic construct.
#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxNode {
    /// Group: explicit {...}, implicit, delimited \left...\right, or inline math $...$
    Group {
        mode: ContentMode,
        kind: GroupKind,
        children: Vec<SyntaxNode>,
    },

    /// Prefix command: \frac{a}{b}, \sqrt[n]{x}
    ///
    /// This is the most common command type where arguments follow the command name.
    Command {
        name: String,
        starred: bool,
        args: Vec<Argument>,
    },

    /// Infix command: a \over b, {n \choose k}
    ///
    /// Only ONE infix command is allowed per group at the top level.
    /// The left and right operands are collected during parsing.
    Infix {
        name: String,
        starred: bool,
        args: Vec<Argument>, // Command's own arguments (usually empty)
        left: Box<SyntaxNode>,
        right: Box<SyntaxNode>,
    },

    /// Declarative command: \color{red} text, \bfseries text
    ///
    /// The scope extends from the command to the end of the current group.
    Declarative {
        name: String,
        starred: bool,
        args: Vec<Argument>,
        scope: Box<SyntaxNode>, // Content from command to end of group
    },

    /// Environment: \begin{env}...\end{env}
    ///
    /// Examples: \begin{matrix}...\end{matrix}, \begin{align*}...\end{align*}
    Environment {
        name: String,
        starred: bool,
        args: Vec<Argument>,
        body: Box<SyntaxNode>, // Environment body (always a Group node)
    },

    /// Scripted expression: x^2_i, a_{n-1}
    ///
    /// Subscripts and superscripts are normalized:
    /// - Order of ^ and _ is ignored (x^2_i == x_i^2)
    /// - Duplicates take the last occurrence (x^a^b -> superscript = b)
    Scripted {
        base: Box<SyntaxNode>,
        subscript: Option<Box<SyntaxNode>>,
        superscript: Option<Box<SyntaxNode>>,
    },

    /// Unknown command (non-strict mode only)
    ///
    /// Produced when a command is not found in the knowledge base
    /// and strict mode is disabled. Starred is always false for unknown commands.
    UnknownCommand { name: String, starred: bool },

    /// Text string (Text mode only)
    ///
    /// Produced in Text mode or as content of Text-mode arguments/environments.
    /// Consecutive characters and whitespace are merged into a single Text node.
    /// Multiple whitespace characters collapse into a single space.
    /// Note: In Math mode, characters remain as individual Char nodes, not Text.
    Text(String),

    /// Single character (primarily in math mode)
    ///
    /// Examples: letters (a-z, A-Z), digits (0-9), symbols (+, -, =)
    Char(char),

    /// Active character ~ (non-breaking space)
    ///
    /// In LaTeX, ~ produces a non-breaking space.
    /// This node is produced in both Math and Text modes.
    /// In Text mode, ~ is NOT merged into TextChunk; it remains as a separate node.
    ActiveSpace,
}

/// Command or environment argument
///
/// Arguments can be mandatory {...} or optional [...].
/// Each argument contains a syntax node representing its content.
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub kind: ArgumentKind,
    pub value: SyntaxNode,
}

/// Argument type: mandatory or optional
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentKind {
    /// Mandatory argument: {...}
    Mandatory,
    /// Optional argument: [...]
    Optional,
}

/// Content mode: math or text
///
/// Determines how content is parsed and interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentMode {
    /// Math mode: default mode, supports formulas, scripts, infix commands
    Math,
    /// Text mode: consecutive chars merged, no scripts, inline math via $...$
    Text,
}

/// Delimiter type for delimited groups
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delimiter {
    /// No delimiter (corresponds to '.' in LaTeX)
    None,
    /// Single character delimiter: '(', ')', '[', ']', '|', etc.
    Char(char),
    /// Control sequence delimiter: "\langle", "\rangle", "\{", "\}", etc.
    Control(&'static str),
}

/// Group type for different grouping constructs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKind {
    /// Explicit group: {...}
    Explicit,

    /// Implicit group: wrapper for sequences that need to be treated as a single node
    ///
    /// Used when folding multiple items into one (e.g., infix operands, declarative scope).
    Implicit,

    /// Delimited group: \left delim ... \right delim
    ///
    /// Examples: \left( ... \right), \left\{ ... \right\}
    Delimited { left: Delimiter, right: Delimiter },

    /// Inline math in text mode: $...$
    ///
    /// Note: Display math \[...\] is not currently supported (future extension).
    InlineMath,
}

// ============ Helper Methods ============

impl SyntaxNode {
    /// Check if this node is a Group
    pub fn is_group(&self) -> bool {
        matches!(self, SyntaxNode::Group { .. })
    }

    /// Check if this node is a leaf (has no children)
    pub fn is_leaf(&self) -> bool {
        matches!(
            self,
            SyntaxNode::Char(_)
                | SyntaxNode::Text(_)
                | SyntaxNode::ActiveSpace
                | SyntaxNode::UnknownCommand { .. }
        )
    }

    /// Get the mode if this is a Group node
    pub fn group_mode(&self) -> Option<ContentMode> {
        match self {
            SyntaxNode::Group { mode, .. } => Some(*mode),
            _ => None,
        }
    }

    /// Create an implicit group wrapping a sequence of nodes
    pub fn implicit_group(mode: ContentMode, children: Vec<SyntaxNode>) -> Self {
        SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children,
        }
    }

    /// Create an empty implicit group
    pub fn empty_group(mode: ContentMode) -> Self {
        SyntaxNode::Group {
            mode,
            kind: GroupKind::Implicit,
            children: Vec::new(),
        }
    }
}

impl Argument {
    /// Create a mandatory argument
    pub fn mandatory(value: SyntaxNode) -> Self {
        Argument {
            kind: ArgumentKind::Mandatory,
            value,
        }
    }

    /// Create an optional argument
    pub fn optional(value: SyntaxNode) -> Self {
        Argument {
            kind: ArgumentKind::Optional,
            value,
        }
    }
}

impl ContentMode {
    /// Check if this is Math mode
    pub fn is_math(&self) -> bool {
        matches!(self, ContentMode::Math)
    }

    /// Check if this is Text mode
    pub fn is_text(&self) -> bool {
        matches!(self, ContentMode::Text)
    }
}

// ============ Display Implementations ============

impl std::fmt::Display for SyntaxNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_indent(f, 0)
    }
}

impl SyntaxNode {
    /// Format with indentation for pretty-printing
    fn fmt_with_indent(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let prefix = "  ".repeat(indent);
        match self {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                writeln!(f, "{}Group({:?}, {:?}) [", prefix, mode, kind)?;
                for child in children {
                    child.fmt_with_indent(f, indent + 1)?;
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Command {
                name,
                starred,
                args,
            } => {
                let star = if *starred { "*" } else { "" };
                writeln!(f, "{}Command(\\{}{}) [", prefix, name, star)?;
                for arg in args {
                    arg.fmt_with_indent(f, indent + 1)?;
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Infix {
                name,
                starred,
                args,
                left,
                right,
            } => {
                let star = if *starred { "*" } else { "" };
                writeln!(f, "{}Infix(\\{}{}) [", prefix, name, star)?;
                writeln!(f, "{}  left:", prefix)?;
                left.fmt_with_indent(f, indent + 2)?;
                writeln!(f, "{}  right:", prefix)?;
                right.fmt_with_indent(f, indent + 2)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        arg.fmt_with_indent(f, indent + 2)?;
                    }
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Declarative {
                name,
                starred,
                args,
                scope,
            } => {
                let star = if *starred { "*" } else { "" };
                writeln!(f, "{}Declarative(\\{}{}) [", prefix, name, star)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        arg.fmt_with_indent(f, indent + 2)?;
                    }
                }
                writeln!(f, "{}  scope:", prefix)?;
                scope.fmt_with_indent(f, indent + 2)?;
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Environment {
                name,
                starred,
                args,
                body,
            } => {
                let star = if *starred { "*" } else { "" };
                writeln!(f, "{}Environment({}{}) [", prefix, name, star)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        arg.fmt_with_indent(f, indent + 2)?;
                    }
                }
                writeln!(f, "{}  body:", prefix)?;
                body.fmt_with_indent(f, indent + 2)?;
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => {
                writeln!(f, "{}Scripted [", prefix)?;
                writeln!(f, "{}  base:", prefix)?;
                base.fmt_with_indent(f, indent + 2)?;
                if let Some(sub) = subscript {
                    writeln!(f, "{}  subscript:", prefix)?;
                    sub.fmt_with_indent(f, indent + 2)?;
                }
                if let Some(sup) = superscript {
                    writeln!(f, "{}  superscript:", prefix)?;
                    sup.fmt_with_indent(f, indent + 2)?;
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::UnknownCommand { name, starred } => {
                let star = if *starred { "*" } else { "" };
                writeln!(f, "{}UnknownCommand(\\{}{})", prefix, name, star)
            }
            SyntaxNode::Text(s) => writeln!(f, "{}Text(\"{}\")", prefix, s),
            SyntaxNode::Char(c) => writeln!(f, "{}Char('{}')", prefix, c),
            SyntaxNode::ActiveSpace => writeln!(f, "{}ActiveSpace", prefix),
        }
    }
}

impl Argument {
    fn fmt_with_indent(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let prefix = "  ".repeat(indent);
        writeln!(f, "{}Arg({:?}):", prefix, self.kind)?;
        self.value.fmt_with_indent(f, indent + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_node_creation() {
        // Test creating various node types
        let char_node = SyntaxNode::Char('a');
        assert!(char_node.is_leaf());

        let text_node = SyntaxNode::Text("hello".to_string());
        assert!(text_node.is_leaf());

        let group = SyntaxNode::Group {
            mode: ContentMode::Math,
            kind: GroupKind::Explicit,
            children: vec![SyntaxNode::Char('x')],
        };
        assert!(group.is_group());
        assert_eq!(group.group_mode(), Some(ContentMode::Math));
    }

    #[test]
    fn test_implicit_group_helpers() {
        let children = vec![SyntaxNode::Char('a'), SyntaxNode::Char('b')];
        let group = SyntaxNode::implicit_group(ContentMode::Math, children.clone());

        match group {
            SyntaxNode::Group {
                mode,
                kind,
                children: c,
            } => {
                assert_eq!(mode, ContentMode::Math);
                assert_eq!(kind, GroupKind::Implicit);
                assert_eq!(c.len(), 2);
            }
            _ => panic!("Expected Group"),
        }

        let empty = SyntaxNode::empty_group(ContentMode::Text);
        match empty {
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                assert_eq!(mode, ContentMode::Text);
                assert_eq!(kind, GroupKind::Implicit);
                assert!(children.is_empty());
            }
            _ => panic!("Expected Group"),
        }
    }

    #[test]
    fn test_argument_creation() {
        let node = SyntaxNode::Char('x');
        let mandatory = Argument::mandatory(node.clone());
        assert_eq!(mandatory.kind, ArgumentKind::Mandatory);

        let optional = Argument::optional(node);
        assert_eq!(optional.kind, ArgumentKind::Optional);
    }

    #[test]
    fn test_content_mode_helpers() {
        assert!(ContentMode::Math.is_math());
        assert!(!ContentMode::Math.is_text());
        assert!(ContentMode::Text.is_text());
        assert!(!ContentMode::Text.is_math());
    }

    #[test]
    fn test_command_node() {
        let cmd = SyntaxNode::Command {
            name: "frac".to_string(),
            starred: false,
            args: vec![
                Argument::mandatory(SyntaxNode::Char('a')),
                Argument::mandatory(SyntaxNode::Char('b')),
            ],
        };

        match cmd {
            SyntaxNode::Command {
                name,
                starred,
                args,
            } => {
                assert_eq!(name, "frac");
                assert!(!starred);
                assert_eq!(args.len(), 2);
            }
            _ => panic!("Expected Command"),
        }
    }

    #[test]
    fn test_infix_node() {
        let infix = SyntaxNode::Infix {
            name: "over".to_string(),
            starred: false,
            args: vec![],
            left: Box::new(SyntaxNode::Char('a')),
            right: Box::new(SyntaxNode::Char('b')),
        };

        match infix {
            SyntaxNode::Infix {
                name, left, right, ..
            } => {
                assert_eq!(name, "over");
                assert!(matches!(*left, SyntaxNode::Char('a')));
                assert!(matches!(*right, SyntaxNode::Char('b')));
            }
            _ => panic!("Expected Infix"),
        }
    }

    #[test]
    fn test_scripted_normalization_structure() {
        // Test that we can create a Scripted node with both sub and sup
        let scripted = SyntaxNode::Scripted {
            base: Box::new(SyntaxNode::Char('x')),
            subscript: Some(Box::new(SyntaxNode::Char('i'))),
            superscript: Some(Box::new(SyntaxNode::Char('2'))),
        };

        match scripted {
            SyntaxNode::Scripted {
                base,
                subscript,
                superscript,
            } => {
                assert!(matches!(*base, SyntaxNode::Char('x')));
                assert!(subscript.is_some());
                assert!(superscript.is_some());
            }
            _ => panic!("Expected Scripted"),
        }
    }

    #[test]
    fn test_group_kind_variants() {
        let explicit = GroupKind::Explicit;
        let implicit = GroupKind::Implicit;
        let delimited = GroupKind::Delimited {
            left: Delimiter::Char('('),
            right: Delimiter::Char(')'),
        };
        let inline_math = GroupKind::InlineMath;

        assert_ne!(explicit, implicit);
        assert_ne!(explicit, delimited);
        assert_ne!(explicit, inline_math);

        match delimited {
            GroupKind::Delimited { left, right } => {
                assert_eq!(left, Delimiter::Char('('));
                assert_eq!(right, Delimiter::Char(')'));
            }
            _ => panic!("Expected Delimited"),
        }
    }

    #[test]
    fn test_delimiter_variants() {
        let none = Delimiter::None;
        let char_delim = Delimiter::Char('(');
        let control_delim = Delimiter::Control("langle");

        assert_ne!(none, char_delim);
        assert_ne!(char_delim, control_delim);

        match char_delim {
            Delimiter::Char(c) => assert_eq!(c, '('),
            _ => panic!("Expected Char delimiter"),
        }

        match control_delim {
            Delimiter::Control(s) => assert_eq!(s, "langle"),
            _ => panic!("Expected Control delimiter"),
        }
    }

    #[test]
    fn test_display_simple() {
        // Test that Display implementation works without panicking
        let node = SyntaxNode::Char('a');
        let display = format!("{}", node);
        assert!(display.contains("Char"));
        assert!(display.contains('a'));

        let group = SyntaxNode::implicit_group(
            ContentMode::Math,
            vec![SyntaxNode::Char('x'), SyntaxNode::Char('y')],
        );
        let display = format!("{}", group);
        assert!(display.contains("Group"));
        assert!(display.contains("Char('x')"));
        assert!(display.contains("Char('y')"));
    }

    #[test]
    fn test_unknown_command() {
        let unknown = SyntaxNode::UnknownCommand {
            name: "foo".to_string(),
            starred: false,
        };

        assert!(unknown.is_leaf());

        match unknown {
            SyntaxNode::UnknownCommand { name, starred } => {
                assert_eq!(name, "foo");
                assert!(!starred);
            }
            _ => panic!("Expected UnknownCommand"),
        }
    }

    #[test]
    fn test_environment_structure() {
        let env = SyntaxNode::Environment {
            name: "matrix".to_string(),
            starred: false,
            args: vec![],
            body: Box::new(SyntaxNode::empty_group(ContentMode::Math)),
        };

        match env {
            SyntaxNode::Environment {
                name,
                starred,
                args,
                body,
            } => {
                assert_eq!(name, "matrix");
                assert!(!starred);
                assert!(args.is_empty());
                assert!(body.is_group());
            }
            _ => panic!("Expected Environment"),
        }
    }

    #[test]
    fn test_declarative_structure() {
        let decl = SyntaxNode::Declarative {
            name: "color".to_string(),
            starred: false,
            args: vec![Argument::mandatory(SyntaxNode::Text("red".to_string()))],
            scope: Box::new(SyntaxNode::Text("text".to_string())),
        };

        match decl {
            SyntaxNode::Declarative {
                name,
                starred,
                args,
                scope,
            } => {
                assert_eq!(name, "color");
                assert!(!starred);
                assert_eq!(args.len(), 1);
                assert!(matches!(*scope, SyntaxNode::Text(_)));
            }
            _ => panic!("Expected Declarative"),
        }
    }
}
