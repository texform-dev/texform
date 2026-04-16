use texform_specs::specs::{BuiltinCommandRecord, CommandKind};

use crate::ast::{Node, NodeId};
use crate::transform::engine::TransformError;
use crate::transform::rule::RuleEffect;
use crate::transform::rule_context::RuleContext;

pub(crate) fn debug_assert_prefix_alias_group_compatible(
    canonical: &'static BuiltinCommandRecord,
    aliases: &[&'static BuiltinCommandRecord],
) {
    debug_assert!(
        !aliases.is_empty(),
        "alias_rule! requires at least one alias record"
    );
    debug_assert_eq!(
        canonical.kind,
        CommandKind::Prefix,
        "alias_rule! canonical record must be a prefix command"
    );

    for &alias in aliases {
        debug_assert_eq!(
            alias.kind,
            CommandKind::Prefix,
            "alias_rule! aliases must be prefix commands",
        );
        debug_assert_ne!(
            alias.name, canonical.name,
            "alias_rule! aliases must not include the canonical command itself",
        );
        debug_assert_eq!(
            alias.allowed_mode, canonical.allowed_mode,
            "alias_rule! aliases must share the same allowed_mode as the canonical command",
        );
        debug_assert_eq!(
            alias.argspec.source, canonical.argspec.source,
            "alias_rule! aliases must share the same argspec as the canonical command",
        );
    }
}

pub(crate) fn rename_prefix_command_alias(
    cx: &mut RuleContext<'_>,
    node_id: NodeId,
    canonical: &'static BuiltinCommandRecord,
    alias_names: &[&str],
) -> Result<RuleEffect, TransformError> {
    let args = match cx.node(node_id) {
        Node::Command { name, args, .. } if alias_names.contains(&name.as_str()) => args.clone(),
        _ => return Ok(RuleEffect::Skipped),
    };

    cx.ast.replace_node(
        node_id,
        Node::Command {
            name: canonical.name.to_string(),
            args,
            known: true,
        },
    );
    Ok(RuleEffect::Applied)
}

#[cfg(test)]
mod tests {
    use texform_specs::argspec;
    use texform_specs::specs::{AllowedMode, BuiltinCommandRecord, CommandKind};

    use super::*;

    static CANONICAL: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "canonical",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m"),
        tags: &["presentation"],
    };

    static VALID_ALIAS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "alias",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m"),
        tags: &[],
    };

    static SAME_AS_CANONICAL: [&BuiltinCommandRecord; 1] = [&CANONICAL];
    static VALID_ALIASES: [&BuiltinCommandRecord; 1] = [&VALID_ALIAS];
    static EMPTY_ALIASES: [&BuiltinCommandRecord; 0] = [];

    static NON_PREFIX_ALIAS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "infix-alias",
        kind: CommandKind::Infix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m"),
        tags: &[],
    };

    static MODE_MISMATCH_ALIAS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "mode-mismatch",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Text,
        argspec: argspec!("m"),
        tags: &[],
    };

    static ARGSPEC_MISMATCH_ALIAS: BuiltinCommandRecord = BuiltinCommandRecord {
        name: "argspec-mismatch",
        kind: CommandKind::Prefix,
        allowed_mode: AllowedMode::Math,
        argspec: argspec!("m m"),
        tags: &[],
    };

    #[test]
    fn accepts_structurally_compatible_prefix_aliases() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &VALID_ALIASES);
    }

    #[test]
    #[should_panic(expected = "at least one alias")]
    fn rejects_empty_alias_lists() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &EMPTY_ALIASES);
    }

    #[test]
    #[should_panic(expected = "must not include the canonical")]
    fn rejects_alias_lists_that_repeat_the_canonical_name() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &SAME_AS_CANONICAL);
    }

    #[test]
    #[should_panic(expected = "must be a prefix command")]
    fn rejects_non_prefix_canonical_records() {
        debug_assert_prefix_alias_group_compatible(&NON_PREFIX_ALIAS, &VALID_ALIASES);
    }

    #[test]
    #[should_panic(expected = "must be prefix commands")]
    fn rejects_non_prefix_alias_records() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &[&NON_PREFIX_ALIAS]);
    }

    #[test]
    #[should_panic(expected = "allowed_mode")]
    fn rejects_allowed_mode_mismatches() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &[&MODE_MISMATCH_ALIAS]);
    }

    #[test]
    #[should_panic(expected = "argspec")]
    fn rejects_argspec_mismatches() {
        debug_assert_prefix_alias_group_compatible(&CANONICAL, &[&ARGSPEC_MISMATCH_ALIAS]);
    }
}
