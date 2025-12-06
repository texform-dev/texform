//! Knowledge base for LaTeX command metadata
//!
//! This module defines command metadata used during parsing.
//! It contains only a minimal set of commands for parser development (Milestone 1-5).
//! Full command definitions will be added later in packages (amsmath.rs, text.rs, etc.).

use crate::syntax_node::{ArgumentKind, ContentMode};

/// Command type in knowledge base (determines AST node type)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Prefix command → creates Command node
    /// Arguments follow the command
    Prefix,

    /// Infix command → creates InfixCommand node
    /// Left and right operands collected from context
    Infix,

    /// Declarative command → creates DeclarativeCommand node
    /// Scope collected from context (command to end of group)
    Declarative,
}

/// Argument specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    /// Argument type (Mandatory or Optional)
    pub kind: ArgumentKind,

    /// Content mode for this argument (Math or Text)
    pub mode: ContentMode,
}

impl ArgSpec {
    /// Create a mandatory argument spec
    pub const fn mandatory(mode: ContentMode) -> Self {
        ArgSpec {
            kind: ArgumentKind::Mandatory,
            mode,
        }
    }

    /// Create an optional argument spec
    pub const fn optional(mode: ContentMode) -> Self {
        ArgSpec {
            kind: ArgumentKind::Optional,
            mode,
        }
    }
}

/// Command metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandMeta {
    /// Command name (without backslash)
    pub name: &'static str,

    /// Command type (determines which AST node type to create)
    pub kind: CommandKind,

    /// Whether command supports starred variant (e.g., \section*)
    pub has_star_variant: bool,

    /// Argument specifications
    /// - For Prefix: all arguments
    /// - For Infix: command's own args (usually empty), left/right collected separately
    /// - For Declarative: command's own args, scope collected separately
    pub args: &'static [ArgSpec],
}

/// Environment metadata in knowledge base
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvMeta {
    /// Environment name (without \begin/\end)
    pub name: &'static str,

    /// Whether environment supports starred variant
    pub has_star_variant: bool,

    /// Argument specifications
    pub args: &'static [ArgSpec],

    /// Content mode for environment body
    pub body_mode: ContentMode,
}

// ============ Minimal Command Database (for parser development) ============
//
// NOTE: These are TEMPORARY definitions for developing the parser (Milestone 1-5).
// Full command definitions will be organized in packages later.

/// Lookup command metadata by name
///
/// Returns None if command is not in the knowledge base.
pub fn lookup_command(name: &str) -> Option<&'static CommandMeta> {
    COMMANDS.iter().find(|cmd| cmd.name == name)
}

/// Lookup environment metadata by name
///
/// Returns None if environment is not in the knowledge base.
pub fn lookup_env(name: &str) -> Option<&'static EnvMeta> {
    ENVIRONMENTS.iter().find(|env| env.name == name)
}

/// Check if command is blacklisted
///
/// Returns Some(reason) if blacklisted, None otherwise.
pub fn is_blacklisted(name: &str) -> Option<&'static str> {
    BLACKLIST
        .iter()
        .find(|(cmd, _)| *cmd == name)
        .map(|(_, reason)| *reason)
}

/// Check if control sequence acts as a delimiter usable by \left...\right
pub fn is_delimiter_control(name: &str) -> bool {
    DELIMITER_CONTROLS.contains(&name)
}

// ============ Temporary Command Definitions ============

static COMMANDS: &[CommandMeta] = &[
    // ---- Prefix commands ----
    // \frac{numerator}{denominator}
    CommandMeta {
        name: "frac",
        kind: CommandKind::Prefix,
        has_star_variant: false,
        args: &[
            ArgSpec::mandatory(ContentMode::Math),
            ArgSpec::mandatory(ContentMode::Math),
        ],
    },
    // \sqrt[n]{x}
    CommandMeta {
        name: "sqrt",
        kind: CommandKind::Prefix,
        has_star_variant: false,
        args: &[
            ArgSpec::optional(ContentMode::Math),
            ArgSpec::mandatory(ContentMode::Math),
        ],
    },
    // \text{content}
    CommandMeta {
        name: "text",
        kind: CommandKind::Prefix,
        has_star_variant: false,
        args: &[ArgSpec::mandatory(ContentMode::Text)],
    },
    // ---- Infix commands ----
    // a \over b
    CommandMeta {
        name: "over",
        kind: CommandKind::Infix,
        has_star_variant: false,
        args: &[], // left/right collected from context
    },
    // {n \choose k}
    CommandMeta {
        name: "choose",
        kind: CommandKind::Infix,
        has_star_variant: false,
        args: &[], // left/right collected from context
    },
    // ---- Declarative commands ----
    // \bfseries text
    CommandMeta {
        name: "bfseries",
        kind: CommandKind::Declarative,
        has_star_variant: false,
        args: &[], // scope collected from context
    },
    // \color{color} text
    CommandMeta {
        name: "color",
        kind: CommandKind::Declarative,
        has_star_variant: false,
        args: &[ArgSpec::mandatory(ContentMode::Text)], // scope collected separately
    },
];

static ENVIRONMENTS: &[EnvMeta] = &[
    // \begin{matrix}...\end{matrix}
    EnvMeta {
        name: "matrix",
        has_star_variant: false,
        args: &[],
        body_mode: ContentMode::Math,
    },
];

static BLACKLIST: &[(&str, &str)] = &[
    ("ifnum", "Control flow not supported"),
    ("csname", "Dynamic command names not supported"),
];

static DELIMITER_CONTROLS: &[&str] = &[
    "langle",
    "rangle",
    "{",
    "}",
    "lfloor",
    "rfloor",
    "lceil",
    "rceil",
    "lvert",
    "rvert",
    "lVert",
    "rVert",
    "lgroup",
    "rgroup",
    "lmoustache",
    "rmoustache",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_command() {
        // Test prefix commands
        let frac = lookup_command("frac").unwrap();
        assert_eq!(frac.name, "frac");
        assert_eq!(frac.kind, CommandKind::Prefix);
        assert!(!frac.has_star_variant);
        assert_eq!(frac.args.len(), 2);

        let sqrt = lookup_command("sqrt").unwrap();
        assert_eq!(sqrt.name, "sqrt");
        assert_eq!(sqrt.args.len(), 2);
        assert_eq!(sqrt.args[0].kind, ArgumentKind::Optional);
        assert_eq!(sqrt.args[1].kind, ArgumentKind::Mandatory);

        // Test infix commands
        let over = lookup_command("over").unwrap();
        assert_eq!(over.kind, CommandKind::Infix);
        assert!(over.args.is_empty());

        // Test declarative commands
        let color = lookup_command("color").unwrap();
        assert_eq!(color.kind, CommandKind::Declarative);
        assert_eq!(color.args.len(), 1);

        // Test unknown command
        assert!(lookup_command("unknown").is_none());
    }

    #[test]
    fn test_lookup_env() {
        let matrix = lookup_env("matrix").unwrap();
        assert_eq!(matrix.name, "matrix");
        assert_eq!(matrix.body_mode, ContentMode::Math);

        assert!(lookup_env("unknown").is_none());
    }

    #[test]
    fn test_blacklist() {
        assert_eq!(is_blacklisted("ifnum"), Some("Control flow not supported"));
        assert_eq!(
            is_blacklisted("csname"),
            Some("Dynamic command names not supported")
        );
        assert_eq!(is_blacklisted("frac"), None);
    }

    #[test]
    fn test_arg_spec_helpers() {
        let mandatory_math = ArgSpec::mandatory(ContentMode::Math);
        assert_eq!(mandatory_math.kind, ArgumentKind::Mandatory);
        assert_eq!(mandatory_math.mode, ContentMode::Math);

        let optional_text = ArgSpec::optional(ContentMode::Text);
        assert_eq!(optional_text.kind, ArgumentKind::Optional);
        assert_eq!(optional_text.mode, ContentMode::Text);
    }

    #[test]
    fn test_delimiter_controls() {
        assert!(is_delimiter_control("langle"));
        assert!(is_delimiter_control("rvert"));
        assert!(!is_delimiter_control("notadelim"));
    }
}
