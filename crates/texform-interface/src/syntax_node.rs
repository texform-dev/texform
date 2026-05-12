//! Immutable syntax tree for LaTeX parsing (Stage 1)
//!
//! This module defines the intermediate representation produced by the parser (chumsky).
//! Unlike the final AST (ast.rs), SyntaxNode uses standard Rust types (Vec, Box)
//! and is optimized for top-down traversal rather than bidirectional navigation.
//!
//! After parsing, the syntax tree is converted to the slotmap-based AST.

use serde::Serialize;

/// Command or environment argument.
///
/// Each argument contains an `ArgumentKind` + `ArgumentValue`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Argument {
    pub kind: ArgumentKind,
    pub value: ArgumentValue,
}

/// Optional slot for argument lists.
pub type ArgumentSlot = Option<Argument>;

/// Argument type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum ArgumentKind {
    /// Standard mandatory argument (`m`).
    Mandatory,
    /// Standard optional bracket argument (`o`).
    Optional,
    /// Star argument (`s`).
    Star,
    /// Braced group argument (`g` or `m{}`).
    ///
    /// Requiredness is enforced by the spec/parser rather than this enum.
    Group,
    /// Single delimited argument (`r` / `d`) with matched delimiters.
    Delimited { open: Delimiter, close: Delimiter },
    /// Paired-candidate argument (`r` / `d` with `<l,r>` pair list) with matched delimiters.
    Paired { open: Delimiter, close: Delimiter },
}

impl ArgumentKind {
    /// Create an ArgumentKind for standard forms from requiredness.
    #[inline]
    pub const fn from_required(required: bool) -> Self {
        if required {
            ArgumentKind::Mandatory
        } else {
            ArgumentKind::Optional
        }
    }
}

/// Parsed argument value.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum ArgumentValue {
    /// Parsed math-mode content subtree.
    MathContent(SyntaxNode),
    /// Parsed text-mode content subtree.
    TextContent(SyntaxNode),
    /// Delimiter argument value.
    Delimiter(Delimiter),
    /// Control-sequence name string with no escape/control sequences.
    CSName(String),
    /// Dimension argument value (raw string).
    Dimension(String),
    /// Integer argument value (raw string).
    Integer(String),
    /// Key-value list argument value (raw string).
    KeyVal(String),
    /// Parsed column template string.
    Column(String),
    /// Boolean argument value, used by star slots.
    Boolean(bool),
}

/// Content mode: math or text
///
/// Determines how content is parsed and interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum ContentMode {
    /// Math mode: default mode, supports formulas, scripts, infix commands
    Math,
    /// Text mode: consecutive chars merged, no scripts, inline math via $...$
    Text,
}

/// Delimiter type for delimited groups
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum Delimiter {
    /// No delimiter (corresponds to '.' in LaTeX)
    None,
    /// Single character delimiter: '(', ')', '[', ']', '|', etc.
    Char(char),
    /// Control sequence delimiter: "\langle", "\rangle", "\{", "\}", etc.
    Control(&'static str),
}

/// Group type for different grouping constructs
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum GroupKind {
    /// Explicit group: {...}
    Explicit,

    /// Implicit group: wrapper for sequences that need to be treated as a single node
    ///
    /// Used when folding multiple items into one (e.g., infix operands).
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

/// Immutable syntax tree node
///
/// Represents the structure of parsed LaTeX source code.
/// Each variant corresponds to a different syntactic construct.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub enum SyntaxNode {
    /// Parse-tree root node produced by the top-level parser.
    ///
    /// A `Root` never nests inside another `SyntaxNode`; it marks the entry
    /// point of a parsed formula and carries the top-level content mode.
    Root {
        mode: ContentMode,
        children: Vec<SyntaxNode>,
    },

    /// Group: explicit {...}, implicit, delimited \left...\right, or inline math $...$
    Group {
        mode: ContentMode,
        kind: GroupKind, // TODO: Move boundary info into Group, remove kind.
        children: Vec<SyntaxNode>,
    },

    /// Prefix command: \frac{a}{b}, \sqrt[n]{x}
    ///
    /// This is the most common command type where arguments follow the command name.
    Command {
        name: String,
        args: Vec<ArgumentSlot>,
        known: bool,
    },

    /// Infix command: a \over b, {n \choose k}
    ///
    /// Only ONE infix command is allowed per group at the top level.
    /// The left and right operands are collected during parsing.
    Infix {
        name: String,
        args: Vec<ArgumentSlot>, // Command's own arguments (usually empty)
        left: Box<SyntaxNode>,
        right: Box<SyntaxNode>,
    },

    /// Declarative command: \color{red}, \bfseries
    Declarative {
        name: String,
        args: Vec<ArgumentSlot>,
    },

    /// Environment: \begin{env}...\end{env}
    ///
    /// Examples: \begin{matrix}...\end{matrix}, \begin{align*}...\end{align*}
    Environment {
        name: String,
        args: Vec<ArgumentSlot>,
        known: bool,
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

    /// Parser-produced error placeholder.
    ///
    /// This keeps recovery output inspectable while making downstream
    /// conversion and serialization fail fast if the caller tries to treat it
    /// as a valid syntax tree.
    Error { message: String, snippet: String },

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
    ///
    /// TODO: Decide whether this needs to remain a distinct node type.
    ActiveSpace,
}

// ============ Helper Methods ============

impl SyntaxNode {
    /// Check if this node is a content container (`Group` or parse-tree `Root`).
    pub fn is_group(&self) -> bool {
        matches!(self, SyntaxNode::Root { .. } | SyntaxNode::Group { .. })
    }

    /// Check if this node is a leaf (has no children)
    pub fn is_leaf(&self) -> bool {
        matches!(
            self,
            SyntaxNode::Char(_)
                | SyntaxNode::Text(_)
                | SyntaxNode::ActiveSpace
                | SyntaxNode::Error { .. }
        ) || matches!(self, SyntaxNode::Command { args, .. } if args.iter().all(|slot| {
            slot.as_ref().is_none_or(|arg| {
                !matches!(
                    arg.value,
                    ArgumentValue::MathContent(_) | ArgumentValue::TextContent(_)
                )
            })
        })) || matches!(self, SyntaxNode::Declarative { args, .. } if args.iter().all(|slot| {
            slot.as_ref().is_none_or(|arg| {
                !matches!(
                    arg.value,
                    ArgumentValue::MathContent(_) | ArgumentValue::TextContent(_)
                )
            })
        }))
    }

    /// Get the content mode if this is a content container (`Group` or `Root`).
    pub fn group_mode(&self) -> Option<ContentMode> {
        match self {
            SyntaxNode::Root { mode, .. } | SyntaxNode::Group { mode, .. } => Some(*mode),
            _ => None,
        }
    }

    /// Create a parse-tree root node wrapping a sequence of top-level children.
    pub fn root(mode: ContentMode, children: Vec<SyntaxNode>) -> Self {
        SyntaxNode::Root { mode, children }
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
    /// Create an argument from a kind and value.
    pub fn from_value(kind: ArgumentKind, value: ArgumentValue) -> Self {
        Argument { kind, value }
    }
}

impl ContentMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            ContentMode::Math => "math",
            ContentMode::Text => "text",
        }
    }
}

impl std::fmt::Display for ContentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
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
            SyntaxNode::Root { mode, children } => {
                writeln!(f, "{}Root({:?}) [", prefix, mode)?;
                Self::fmt_group_children_with_indent(f, children, indent + 1)?;
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Group {
                mode,
                kind,
                children,
            } => {
                writeln!(f, "{}Group({:?}, {:?}) [", prefix, mode, kind)?;
                Self::fmt_group_children_with_indent(f, children, indent + 1)?;
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Command { name, args, known } => {
                writeln!(f, "{}Command(\\{}, known={}) [", prefix, name, known)?;
                for arg in args {
                    fmt_argument_slot(f, arg, indent + 1)?;
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Infix {
                name,
                args,
                left,
                right,
            } => {
                writeln!(f, "{}Infix(\\{}) [", prefix, name)?;
                writeln!(f, "{}  left:", prefix)?;
                left.fmt_with_indent(f, indent + 2)?;
                writeln!(f, "{}  right:", prefix)?;
                right.fmt_with_indent(f, indent + 2)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        fmt_argument_slot(f, arg, indent + 2)?;
                    }
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Declarative { name, args } => {
                writeln!(f, "{}Declarative(\\{}) [", prefix, name)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        fmt_argument_slot(f, arg, indent + 2)?;
                    }
                }
                writeln!(f, "{}]", prefix)
            }
            SyntaxNode::Environment {
                name,
                args,
                known,
                body,
            } => {
                writeln!(f, "{}Environment({}, known={}) [", prefix, name, known)?;
                if !args.is_empty() {
                    writeln!(f, "{}  args:", prefix)?;
                    for arg in args {
                        fmt_argument_slot(f, arg, indent + 2)?;
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
            SyntaxNode::Error { message, snippet } => {
                writeln!(
                    f,
                    "{}Error(message: {}, snippet: {})",
                    prefix, message, snippet
                )
            }
            SyntaxNode::Text(s) => writeln!(f, "{}Text(\"{}\")", prefix, s),
            SyntaxNode::Char(c) => writeln!(f, "{}Char('{}')", prefix, c),
            SyntaxNode::ActiveSpace => writeln!(f, "{}ActiveSpace", prefix),
        }
    }

    fn fmt_group_children_with_indent(
        f: &mut std::fmt::Formatter<'_>,
        children: &[SyntaxNode],
        indent: usize,
    ) -> std::fmt::Result {
        let prefix = "  ".repeat(indent);
        let mut i = 0;

        while i < children.len() {
            if let SyntaxNode::Char(_) = children[i] {
                let mut merged = String::new();
                while i < children.len() {
                    match &children[i] {
                        SyntaxNode::Char(c) => {
                            merged.push(*c);
                            i += 1;
                        }
                        _ => break,
                    }
                }

                if merged.chars().count() == 1 {
                    writeln!(f, "{}Char('{}')", prefix, merged.chars().next().unwrap())?;
                } else {
                    writeln!(f, "{}Chars({:?})", prefix, merged)?;
                }
                continue;
            }

            children[i].fmt_with_indent(f, indent)?;
            i += 1;
        }

        Ok(())
    }
}

impl Argument {
    fn fmt_with_indent(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let prefix = "  ".repeat(indent);
        writeln!(f, "{}Arg({:?}):", prefix, self.kind)?;
        self.value.fmt_with_indent(f, indent + 1)
    }
}

impl ArgumentValue {
    fn fmt_with_indent(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let prefix = "  ".repeat(indent);
        match self {
            ArgumentValue::MathContent(node) | ArgumentValue::TextContent(node) => {
                node.fmt_with_indent(f, indent)
            }
            ArgumentValue::Delimiter(delim) => writeln!(f, "{}Delimiter({:?})", prefix, delim),
            ArgumentValue::CSName(value) => writeln!(f, "{}CSName(\"{}\")", prefix, value),
            ArgumentValue::Dimension(value) => writeln!(f, "{}Dimension(\"{}\")", prefix, value),
            ArgumentValue::Integer(value) => writeln!(f, "{}Integer(\"{}\")", prefix, value),
            ArgumentValue::KeyVal(value) => writeln!(f, "{}KeyVal(\"{}\")", prefix, value),
            ArgumentValue::Column(value) => writeln!(f, "{}Column(\"{}\")", prefix, value),
            ArgumentValue::Boolean(value) => writeln!(f, "{}Boolean({})", prefix, value),
        }
    }
}

fn fmt_argument_slot(
    f: &mut std::fmt::Formatter<'_>,
    slot: &ArgumentSlot,
    indent: usize,
) -> std::fmt::Result {
    let prefix = "  ".repeat(indent);
    match slot {
        Some(argument) => argument.fmt_with_indent(f, indent),
        None => writeln!(f, "{}Arg(None)", prefix),
    }
}

// Tests in tests/syntax_node.rs
