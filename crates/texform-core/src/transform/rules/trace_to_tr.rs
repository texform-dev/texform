//! Canonicalize trace aliases into the lowercase `\tr` command.

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Canonicalize `\Tr`, `\trace`, and `\Trace` into `\tr`.
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Canonical / "trace-to-tr",
        summary: "Canonicalize \\Tr, \\trace, and \\Trace into \\tr",
        phase: Normalize,
        safety: Lossless,
        canonical: &physics::cmd::TR,
        aliases: [
            &physics::cmd::TR_2,
            &physics::cmd::TRACE,
            &physics::cmd::TRACE_2,
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Node;
    use crate::context::ParseContext;
    use crate::transform::{RuleAvailability, TransformProfile};

    #[test]
    fn rewrites_all_trace_aliases_to_tr() {
        let ctx = ParseContext::from_packages(&["physics"]);
        for input in [r"\Tr", r"\trace", r"\Trace"] {
            let output = ctx
                .parse_and_transform(input, true, &TransformProfile::default())
                .unwrap_or_else(|error| panic!("trace-to-tr transform should succeed: {error:?}"));

            assert_eq!(output.transform_report.applied.len(), 1);
            assert_eq!(
                output.transform_report.applied[0].key.to_string(),
                "canonical/trace-to-tr"
            );

            let root = output.ast.root();
            let children = output.ast.children(root);
            assert_eq!(children.len(), 1);

            match output.ast.node(children[0]) {
                Node::Command { name, args } => {
                    assert_eq!(name, "tr");
                    assert!(args.is_empty());
                }
                other => panic!("expected tr command after transform, got {:?}", other),
            }
        }
    }

    #[test]
    fn reports_rule_as_available_for_physics_profile() {
        let ctx = ParseContext::from_packages(&["physics"]);
        let statuses = ctx
            .transform_rule_statuses(&TransformProfile::default())
            .expect("profile compilation should succeed");

        let status = statuses
            .iter()
            .find(|status| status.key.to_string() == "canonical/trace-to-tr")
            .expect("trace-to-tr status should exist");
        assert!(matches!(status.availability, RuleAvailability::Available));
    }
}
