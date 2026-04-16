//! Rule context and typed node views for rule matching.
//!
//! This module provides [`RuleContext`], the runtime context object passed to
//! [`TransformRule::apply()`](super::rule::TransformRule::apply) during AST
//! transformation. It bundles mutable AST access with knowledge-base lookups,
//! validation helpers, and statistics tracking.
//!
//! It also defines a family of read-only *view* structs ([`CommandView`],
//! [`InfixView`], [`DeclarativeView`], [`EnvironmentView`]) that the
//! `match_*` helpers extract from AST nodes. Rules operate on these views
//! instead of pattern-matching raw [`Node`] variants directly, which keeps
//! rule implementations concise and type-safe.

use crate::ast::{ArgumentSlot, Ast, Node, NodeId};
use crate::knowledge::{KnowledgeBase, lookup_command_node_name, lookup_environment_node_name};
use crate::transform::context::TransformContext;
use crate::transform::engine::{TransformError, TransformReport};
use crate::transform::rule::RuleKey;
use texform_specs::specs::{
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

/// A read-only view of a declarative command node for use in rule matching.
#[derive(Clone, Copy)]
pub struct DeclarativeView<'a> {
    /// The command name without the leading backslash.
    pub name: &'a str,
    /// The explicit argument slots parsed for this command.
    pub args: &'a [ArgumentSlot],
    /// The scope subtree that this declaration affects (up to the enclosing group boundary).
    pub scope: NodeId,
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

/// The runtime context object passed to [`TransformRule::apply()`](super::rule::TransformRule::apply).
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
    kb: &'a KnowledgeBase,
    transform_ctx: &'a TransformContext,
    report: &'a mut TransformReport,
}

impl<'a> RuleContext<'a> {
    pub fn new(
        ast: &'a mut Ast,
        kb: &'a KnowledgeBase,
        transform_ctx: &'a TransformContext,
        report: &'a mut TransformReport,
    ) -> Self {
        Self {
            ast,
            kb,
            transform_ctx,
            report,
        }
    }

    /// Looks up the active command record for the node at `node_id` by extracting its name from the AST.
    pub fn active_command(&self, node_id: NodeId) -> Option<&ActiveCommandRecord> {
        let name = lookup_command_node_name(self.ast.node(node_id))?;
        self.kb.lookup_command(name)
    }

    /// Looks up the active character record for the node at `node_id` by extracting its name from the AST.
    pub fn active_character(&self, node_id: NodeId) -> Option<&ActiveCharacterRecord> {
        let name = lookup_command_node_name(self.ast.node(node_id))?;
        self.kb.lookup_character(name)
    }

    /// Looks up the active environment record for the node at `node_id` by extracting its name from the AST.
    pub fn active_env(&self, node_id: NodeId) -> Option<&ActiveEnvironmentRecord> {
        let name = lookup_environment_node_name(self.ast.node(node_id))?;
        self.kb.lookup_env(name)
    }

    /// Looks up a command record by name directly in the knowledge base.
    pub fn lookup_command(&self, name: &str) -> Option<&ActiveCommandRecord> {
        self.kb.lookup_command(name)
    }

    /// Looks up a character record by name directly in the knowledge base.
    pub fn lookup_character(&self, name: &str) -> Option<&ActiveCharacterRecord> {
        self.kb.lookup_character(name)
    }

    /// Looks up an environment record by name directly in the knowledge base.
    pub fn lookup_env(&self, name: &str) -> Option<&ActiveEnvironmentRecord> {
        self.kb.lookup_env(name)
    }

    pub fn transform_context(&self) -> &TransformContext {
        self.transform_ctx
    }

    /// Records that a rule was successfully applied, incrementing its count in the report.
    pub fn mark_rule_applied(&mut self, key: RuleKey) {
        self.report.mark_rule_applied(key);
    }

    /// Records that a rule was attempted after trigger matching but made no change.
    pub fn mark_rule_skipped(&mut self, key: RuleKey) {
        self.report.mark_rule_skipped(key);
    }

    /// Records the total number of fixed-point iterations the engine performed.
    pub fn record_iteration(&mut self, iterations: usize) {
        self.report.record_iteration(iterations);
    }

    /// Returns the AST node for the given identifier.
    pub fn node(&self, node_id: NodeId) -> &Node {
        self.ast.node(node_id)
    }

    /// Creates an [`InvalidNodeShape`](TransformError::InvalidNodeShape) error for the given rule.
    pub fn invalid_shape(&self, rule: RuleKey, message: impl Into<String>) -> TransformError {
        TransformError::InvalidNodeShape {
            rule,
            message: message.into(),
        }
    }

    /// Creates a [`MissingMetadata`](TransformError::MissingMetadata) error for the given rule.
    pub fn missing_metadata(&self, rule: RuleKey, name: impl Into<String>) -> TransformError {
        TransformError::MissingMetadata {
            rule,
            name: name.into(),
        }
    }

    /// Returns `Ok(())` when `condition` is true, or an [`InvalidNodeShape`](TransformError::InvalidNodeShape) error otherwise.
    pub fn ensure_shape(
        &self,
        condition: bool,
        rule: RuleKey,
        message: impl Into<String>,
    ) -> Result<(), TransformError> {
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
    ) -> Result<(), TransformError> {
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
    ) -> Result<(), TransformError> {
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
            Node::Declarative { name, args, scope } if name == record.name => {
                Some(DeclarativeView {
                    name: name.as_str(),
                    args: args.as_slice(),
                    scope: *scope,
                })
            }
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
