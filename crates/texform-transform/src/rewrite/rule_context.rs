//! Rule context and typed node views for rule matching.
//!
//! This module provides [`RuleContext`], the runtime context object passed to
//! [`RewriteRule::apply()`](super::rule::RewriteRule::apply) during AST
//! transformation. It bundles mutable AST access with knowledge-base lookups,
//! validation helpers, and statistics tracking.
//!
//! It also defines a family of read-only *view* structs ([`CommandView`],
//! [`InfixView`], [`DeclarativeView`], [`EnvironmentView`]) that the
//! `match_*` helpers extract from AST nodes. Rules operate on these views
//! instead of pattern-matching raw [`Node`] variants directly, which keeps
//! rule implementations concise and type-safe.

use std::ops::Deref;

use crate::ast::{ArgumentKind, ArgumentSlot, ArgumentValue, Ast, Node, NodeId};
use crate::knowledge::{KnowledgeBase, lookup_command_node_name, lookup_environment_node_name};
use crate::parse::ContentMode;
use crate::rewrite::rule::RuleKey;
use crate::rewrite::{RewriteReport, RuleError};
use texform_knowledge::specs::{
    ActiveCharacterRecord, ActiveCommandRecord, ActiveEnvironmentRecord, BuiltinCommandRecord,
    BuiltinEnvironmentRecord,
};

/// A read-only view of a prefix command node for use in rule matching.
#[derive(Clone, Copy)]
pub struct CommandView<'a> {
    /// The command name without the leading backslash.
    pub name: &'a str,
    /// The explicit argument slots parsed for this command.
    pub args: &'a [ArgumentSlot],
}

impl CommandView<'_> {
    /// Returns the command subject used in transform diagnostics, such as `\frac`.
    pub fn subject(&self) -> String {
        format!(r"\{}", self.name)
    }
}

/// A read-only view of an infix command node for use in rule matching.
#[derive(Clone, Copy)]
pub struct InfixView<'a> {
    /// The command name without the leading backslash.
    pub name: &'a str,
    /// The explicit argument slots parsed for this command.
    pub args: &'a [ArgumentSlot],
    /// The left operand subtree collected by the parser.
    pub left: NodeId,
    /// The right operand subtree collected by the parser.
    pub right: NodeId,
}

impl InfixView<'_> {
    /// Returns the infix command subject used in transform diagnostics, such as `\over`.
    pub fn subject(&self) -> String {
        format!(r"\{}", self.name)
    }
}

/// A read-only view of a declarative command node for use in rule matching.
#[derive(Clone, Copy)]
pub struct DeclarativeView<'a> {
    /// The command name without the leading backslash.
    pub name: &'a str,
    /// The explicit argument slots parsed for this command.
    pub args: &'a [ArgumentSlot],
}

/// A read-only view of an environment node for use in rule matching.
#[derive(Clone, Copy)]
pub struct EnvironmentView<'a> {
    /// The environment name (as it appears between `\begin{…}` and `\end{…}`).
    pub name: &'a str,
    /// The explicit argument slots parsed for this environment.
    pub args: &'a [ArgumentSlot],
    /// The body subtree between `\begin` and `\end`.
    pub body: NodeId,
}

/// The runtime context object passed to [`RewriteRule::apply()`](super::rule::RewriteRule::apply).
///
/// It bundles mutable AST access with knowledge-base lookups, node-shape
/// validation helpers, and statistics tracking. Rules receive a mutable
/// reference to this context and use it both to inspect the current tree
/// and to record replacement nodes.
///
/// `ast` is intentionally public because many transforms need unrestricted
/// structural mutation, not just a narrow helper surface. The tradeoff is that
/// rules can also violate AST invariants if they misuse low-level operations,
/// so debug builds re-run [`Ast::assert_invariants()`](crate::ast::Ast::assert_invariants)
/// after every successful rewrite. Knowledge-base access, transform-context
/// queries, and report mutation stay mediated through methods because those interactions are
/// semantic rather than structural.
pub struct RuleContext<'a> {
    /// Mutable access to the AST being transformed.
    ///
    /// This field stays public so rules can perform bespoke tree surgery when
    /// helper functions are not expressive enough.
    pub ast: &'a mut Ast,
    math_kb: &'a KnowledgeBase,
    text_kb: &'a KnowledgeBase,
    report: &'a mut RewriteReport,
}

/// A read-only scoped context bound to a rule key for diagnostics and slot extraction.
pub struct RuleScopedContext<'cx, 'ctx> {
    cx: &'cx RuleContext<'ctx>,
    rule: RuleKey,
}

impl<'cx, 'ctx> Deref for RuleScopedContext<'cx, 'ctx> {
    type Target = RuleContext<'ctx>;

    fn deref(&self) -> &Self::Target {
        self.cx
    }
}

impl RuleScopedContext<'_, '_> {
    /// Creates an [`InvalidNodeShape`](RuleError::InvalidNodeShape) error for the bound rule.
    pub fn invalid_shape(&self, message: impl Into<String>) -> RuleError {
        self.cx.invalid_shape(self.rule, message)
    }

    /// Creates a [`MissingMetadata`](RuleError::MissingMetadata) error for the bound rule.
    pub fn missing_metadata(&self, name: impl Into<String>) -> RuleError {
        self.cx.missing_metadata(self.rule, name)
    }

    /// Returns `Ok(())` when `condition` is true, or an invalid-shape error otherwise.
    pub fn ensure_shape(
        &self,
        condition: bool,
        message: impl Into<String>,
    ) -> Result<(), RuleError> {
        self.cx.ensure_shape(condition, self.rule, message)
    }

    /// Asserts that `args` has exactly `expected` slots, returning an error that names `subject` on mismatch.
    pub fn expect_arg_len(
        &self,
        args: &[ArgumentSlot],
        expected: usize,
        subject: &str,
    ) -> Result<(), RuleError> {
        self.cx.expect_arg_len(self.rule, args, expected, subject)
    }

    /// Shorthand for [`expect_arg_len`](Self::expect_arg_len) with `expected = 0`.
    pub fn expect_no_args(&self, args: &[ArgumentSlot], subject: &str) -> Result<(), RuleError> {
        self.cx.expect_no_args(self.rule, args, subject)
    }

    /// Extracts a boolean star argument from a parsed star slot.
    pub fn star_arg_value(&self, slot: &ArgumentSlot, subject: &str) -> Result<bool, RuleError> {
        match slot {
            Some(arg) if arg.kind == ArgumentKind::Star => match arg.value {
                ArgumentValue::Boolean(value) => Ok(value),
                _ => {
                    Err(self
                        .invalid_shape(format!("{subject} star slot should carry a boolean value")))
                }
            },
            _ => Err(self.invalid_shape(format!("{subject} should carry a star slot"))),
        }
    }

    /// Extracts an optional math-content argument.
    pub fn optional_math_content(
        &self,
        slot: &ArgumentSlot,
        subject: &str,
        label: &str,
    ) -> Result<Option<NodeId>, RuleError> {
        match slot {
            None => Ok(None),
            Some(arg) if arg.kind == ArgumentKind::Optional => match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(Some(node_id)),
                _ => Err(self.invalid_shape(format!("{subject} {label} should be math content"))),
            },
            _ => Err(self.invalid_shape(format!(
                "{subject} {label} should be an optional math argument"
            ))),
        }
    }

    /// Extracts an optional braced-group math-content argument.
    pub fn optional_group_math_content(
        &self,
        slot: &ArgumentSlot,
        subject: &str,
        label: &str,
    ) -> Result<Option<NodeId>, RuleError> {
        match slot {
            None => Ok(None),
            Some(arg) if arg.kind == ArgumentKind::Group => match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(Some(node_id)),
                _ => Err(self
                    .invalid_shape(format!("{subject} optional {label} should be math content"))),
            },
            _ => Err(self.invalid_shape(format!(
                "{subject} optional {label} should be a braced group"
            ))),
        }
    }

    /// Extracts a mandatory math-content argument.
    pub fn mandatory_math_content(
        &self,
        slot: &ArgumentSlot,
        subject: &str,
        label: &str,
    ) -> Result<NodeId, RuleError> {
        match slot {
            Some(arg) if arg.kind == ArgumentKind::Mandatory => match arg.value {
                ArgumentValue::MathContent(node_id) => Ok(node_id),
                _ => Err(self.invalid_shape(format!("{subject} {label} should be math content"))),
            },
            _ => Err(self.invalid_shape(format!(
                "{subject} {label} should be a mandatory math argument"
            ))),
        }
    }

    /// Extracts a math-content argument that may be either mandatory or a braced group.
    pub fn mandatory_or_group_math_content(
        &self,
        slot: &ArgumentSlot,
        subject: &str,
        label: &str,
    ) -> Result<NodeId, RuleError> {
        match slot {
            Some(arg) if matches!(arg.kind, ArgumentKind::Mandatory | ArgumentKind::Group) => {
                match arg.value {
                    ArgumentValue::MathContent(node_id) => Ok(node_id),
                    _ => {
                        Err(self.invalid_shape(format!("{subject} {label} should be math content")))
                    }
                }
            }
            _ => Err(self.invalid_shape(format!("{subject} {label} should be math content"))),
        }
    }
}

impl<'a> RuleContext<'a> {
    pub fn new(
        ast: &'a mut Ast,
        math_kb: &'a KnowledgeBase,
        text_kb: &'a KnowledgeBase,
        report: &'a mut RewriteReport,
    ) -> Self {
        Self {
            ast,
            math_kb,
            text_kb,
            report,
        }
    }

    fn kb_for(&self, mode: ContentMode) -> &'a KnowledgeBase {
        match mode {
            ContentMode::Math => self.math_kb,
            ContentMode::Text => self.text_kb,
        }
    }

    /// Returns a lightweight context that binds diagnostics and slot extraction to one rule.
    pub fn for_rule(&self, rule: RuleKey) -> RuleScopedContext<'_, 'a> {
        RuleScopedContext { cx: self, rule }
    }

    /// Looks up the active command record for the node at `node_id` by extracting its name from the AST.
    pub fn active_command(&self, node_id: NodeId) -> Option<&ActiveCommandRecord> {
        let name = lookup_command_node_name(self.ast.node(node_id))?;
        self.lookup_command(name, ContentMode::Math)
            .or_else(|| self.lookup_command(name, ContentMode::Text))
    }

    /// Looks up the active environment record for the node at `node_id` by extracting its name from the AST.
    pub fn active_env(&self, node_id: NodeId) -> Option<&ActiveEnvironmentRecord> {
        let name = lookup_environment_node_name(self.ast.node(node_id))?;
        self.lookup_env(name, ContentMode::Math)
            .or_else(|| self.lookup_env(name, ContentMode::Text))
    }

    /// Looks up a command record by name directly in the selected knowledge-base lane.
    pub fn lookup_command(&self, name: &str, mode: ContentMode) -> Option<&ActiveCommandRecord> {
        self.kb_for(mode).lookup_command(name)
    }

    /// Looks up a character record by name directly in the selected knowledge-base lane.
    pub fn lookup_character(
        &self,
        name: &str,
        mode: ContentMode,
    ) -> Option<&ActiveCharacterRecord> {
        self.kb_for(mode).lookup_character(name)
    }

    /// Looks up an environment record by name directly in the selected knowledge-base lane.
    pub fn lookup_env(&self, name: &str, mode: ContentMode) -> Option<&ActiveEnvironmentRecord> {
        self.kb_for(mode).lookup_env(name)
    }

    /// Records that a rule was successfully applied, incrementing its count in the report.
    pub fn mark_rule_applied(&mut self, key: RuleKey) {
        self.report.mark_rule_applied(key);
    }

    /// Records that a rule was attempted after consumed target matching but made no change.
    pub fn mark_rule_skipped(&mut self, key: RuleKey) {
        self.report.mark_rule_skipped(key);
    }

    /// Returns the AST node for the given identifier.
    pub fn node(&self, node_id: NodeId) -> &Node {
        self.ast.node(node_id)
    }

    /// Creates an [`InvalidNodeShape`](RuleError::InvalidNodeShape) error for the given rule.
    pub fn invalid_shape(&self, _rule: RuleKey, message: impl Into<String>) -> RuleError {
        RuleError::InvalidNodeShape {
            message: message.into(),
        }
    }

    /// Creates a [`MissingMetadata`](RuleError::MissingMetadata) error for the given rule.
    pub fn missing_metadata(&self, _rule: RuleKey, name: impl Into<String>) -> RuleError {
        RuleError::MissingMetadata { name: name.into() }
    }

    /// Returns `Ok(())` when `condition` is true, or an [`InvalidNodeShape`](RuleError::InvalidNodeShape) error otherwise.
    pub fn ensure_shape(
        &self,
        condition: bool,
        rule: RuleKey,
        message: impl Into<String>,
    ) -> Result<(), RuleError> {
        if condition {
            Ok(())
        } else {
            Err(self.invalid_shape(rule, message))
        }
    }

    /// Asserts that `args` has exactly `expected` slots, returning an error that names `subject` on mismatch.
    pub fn expect_arg_len(
        &self,
        rule: RuleKey,
        args: &[ArgumentSlot],
        expected: usize,
        subject: &str,
    ) -> Result<(), RuleError> {
        self.ensure_shape(
            args.len() == expected,
            rule,
            format!(
                "{subject} should carry exactly {expected} explicit argument slots, got {}",
                args.len()
            ),
        )
    }

    /// Shorthand for [`expect_arg_len`](Self::expect_arg_len) with `expected = 0`.
    pub fn expect_no_args(
        &self,
        rule: RuleKey,
        args: &[ArgumentSlot],
        subject: &str,
    ) -> Result<(), RuleError> {
        self.expect_arg_len(rule, args, 0, subject)
    }

    /// Tries to extract a [`CommandView`] from the node, returning `None` if it is not a matching prefix command.
    pub fn match_command(
        &self,
        node_id: NodeId,
        record: &'static BuiltinCommandRecord,
    ) -> Option<CommandView<'_>> {
        match self.ast.node(node_id) {
            Node::Command { name, args, .. } if name == record.name => Some(CommandView {
                name: name.as_str(),
                args: args.as_slice(),
            }),
            _ => None,
        }
    }

    /// Tries to extract an [`InfixView`] from the node, returning `None` if it is not a matching infix command.
    pub fn match_infix(
        &self,
        node_id: NodeId,
        record: &'static BuiltinCommandRecord,
    ) -> Option<InfixView<'_>> {
        match self.ast.node(node_id) {
            Node::Infix {
                name,
                args,
                left,
                right,
            } if name == record.name => Some(InfixView {
                name: name.as_str(),
                args: args.as_slice(),
                left: *left,
                right: *right,
            }),
            _ => None,
        }
    }

    /// Tries to extract a [`DeclarativeView`] from the node, returning `None` if it is not a matching declarative command.
    pub fn match_declarative(
        &self,
        node_id: NodeId,
        record: &'static BuiltinCommandRecord,
    ) -> Option<DeclarativeView<'_>> {
        match self.ast.node(node_id) {
            Node::Declarative { name, args } if name == record.name => Some(DeclarativeView {
                name: name.as_str(),
                args: args.as_slice(),
            }),
            _ => None,
        }
    }

    /// Tries to extract an [`EnvironmentView`] from the node, returning `None` if it is not a matching environment.
    pub fn match_environment(
        &self,
        node_id: NodeId,
        record: &'static BuiltinEnvironmentRecord,
    ) -> Option<EnvironmentView<'_>> {
        match self.ast.node(node_id) {
            Node::Environment {
                name, args, body, ..
            } if name == record.name => Some(EnvironmentView {
                name: name.as_str(),
                args: args.as_slice(),
                body: *body,
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Argument;
    use crate::parse::ParseContext;
    use crate::rewrite::{PackageName, RewriteReport, RuleKey};

    const TEST_RULE: RuleKey = RuleKey {
        package: PackageName::Base,
        name: "rule-context-test",
    };

    #[test]
    fn extracts_common_prefix_argument_shapes() {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut report = RewriteReport::default();
        let mut ast = Ast::new();
        let required = ast.new_node(Node::Char('x'));
        let optional = ast.new_node(Node::Char('2'));
        let grouped = ast.new_node(Node::Char('t'));
        let cx = RuleContext::new(
            &mut ast,
            parse_ctx.math_kb(),
            parse_ctx.text_kb(),
            &mut report,
        );

        let star = Some(Argument::from_value(
            ArgumentKind::Star,
            ArgumentValue::Boolean(true),
        ));
        let required = Some(Argument::from_value(
            ArgumentKind::Mandatory,
            ArgumentValue::MathContent(required),
        ));
        let optional = Some(Argument::from_value(
            ArgumentKind::Optional,
            ArgumentValue::MathContent(optional),
        ));
        let grouped = Some(Argument::from_value(
            ArgumentKind::Group,
            ArgumentValue::MathContent(grouped),
        ));

        assert!(
            cx.for_rule(TEST_RULE)
                .star_arg_value(&star, r"\example")
                .unwrap()
        );
        assert_eq!(
            cx.for_rule(TEST_RULE)
                .mandatory_math_content(&required, r"\example", "argument")
                .unwrap(),
            required
                .as_ref()
                .and_then(|arg| match arg.value {
                    ArgumentValue::MathContent(id) => Some(id),
                    _ => None,
                })
                .unwrap()
        );
        assert_eq!(
            cx.for_rule(TEST_RULE)
                .optional_math_content(&optional, r"\example", "order")
                .unwrap(),
            optional.as_ref().and_then(|arg| match arg.value {
                ArgumentValue::MathContent(id) => Some(id),
                _ => None,
            })
        );
        assert_eq!(
            cx.for_rule(TEST_RULE)
                .optional_group_math_content(&grouped, r"\example", "denominator")
                .unwrap(),
            grouped.as_ref().and_then(|arg| match arg.value {
                ArgumentValue::MathContent(id) => Some(id),
                _ => None,
            })
        );
        assert_eq!(
            cx.for_rule(TEST_RULE)
                .mandatory_or_group_math_content(&grouped, r"\example", "argument")
                .unwrap(),
            grouped
                .as_ref()
                .and_then(|arg| match arg.value {
                    ArgumentValue::MathContent(id) => Some(id),
                    _ => None,
                })
                .unwrap()
        );
        assert_eq!(
            cx.for_rule(TEST_RULE)
                .optional_math_content(&None, r"\example", "order")
                .unwrap(),
            None
        );
    }
}
