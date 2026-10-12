use texform_knowledge::specs::BuiltinCommandRecord;

use crate::ast::{ContentMode, Node, NodeId};
use crate::rewrite::RuleError;
use crate::rewrite::helpers::followed_by_paren_bracket_or_group;
use crate::rewrite::rule::{RuleEffect, RuleKey};
use crate::rewrite::rule_context::RuleContext;

/// Renames a bare long physics name to its short named function.
///
/// The long names (`\sine`, `\cosine`, `\naturallogarithm`, `\Probability`)
/// are the original LaTeX operators that physics.sty saves with `\let`, so
/// they grab nothing. The short names grab a following `(...)` or `[...]`
/// (and `\Pr` also `{...}`), so only an occurrence that is not followed by
/// one of these is renamed. The new command gets the empty argument slots of
/// the short name's active record, which is an optional power under physics.
pub(super) fn rename_bare_named_function(
    cx: &mut RuleContext<'_>,
    rule: RuleKey,
    node_id: NodeId,
    alias: &'static BuiltinCommandRecord,
    canonical: &'static BuiltinCommandRecord,
) -> Result<RuleEffect, RuleError> {
    let Some(command) = cx.match_command(node_id, alias) else {
        return Ok(RuleEffect::Skipped);
    };
    cx.for_rule(rule)
        .expect_no_args(command.args, &command.subject())?;
    if followed_by_paren_bracket_or_group(cx.ast, node_id) {
        return Ok(RuleEffect::Skipped);
    }
    let slots = cx
        .lookup_command(canonical.name, ContentMode::Math)
        .ok_or_else(|| cx.for_rule(rule).missing_metadata(canonical.name))?
        .argspec
        .args
        .len();

    cx.ast.replace_node_drop_detached_children(
        node_id,
        Node::Command {
            name: canonical.name.to_string(),
            args: vec![None; slots],
            known: true,
        },
    );
    Ok(RuleEffect::Applied)
}
