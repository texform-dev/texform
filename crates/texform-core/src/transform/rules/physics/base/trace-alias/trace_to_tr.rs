//! Collapse trace aliases to the local tr anchor.
//!
//! ```yaml
//! proposal: trace-to-tr
//! consumes:
//!   eliminates: [cmd:trace, cmd:Trace, cmd:Tr]
//!   touches: null
//! produces: cmd:tr
//! rewrite_patterns:
//!   - {label: trace, from: \trace, to: \tr}
//!   - {label: trace-capital, from: \Trace, to: \tr}
//!   - {label: tr-capital, from: \Tr, to: \tr}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    /// Collapse trace aliases to the local tr anchor.
    pub static TRACE_TO_TR: TraceToTrRule {
        key: Physics / "trace-to-tr",
        tier: Base,
        summary: "Collapse trace aliases to the local tr anchor.",
        phase: Normalize,
        safety: Lossless,
        enabled_by_packages: [Physics],
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
    use super::*;
    use crate::ast::Node;
    use crate::parse::ParseContext;
    use crate::transform::transform_examples;
    use crate::transform::{TransformProfile, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TRACE_TO_TR,
        tier: Base,
        examples: [
        {
            label: trace_density_operator,
            packages: ["base", "physics"],
            input: r"Z=\trace(\rho H)",
            expected: r"Z=\tr(\rho H)",
        },
        {
            label: trace_matrix_square,
            packages: ["base", "physics"],
            input: r"\Trace(M^2)=1",
            expected: r"\tr(M^2)=1",
        },
        {
            label: tr_partition_function,
            packages: ["base", "physics"],
            input: r"S=\Tr(e^{-\beta H})",
            expected: r"S=\tr(e^{-\beta H})",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn rewrites_all_trace_aliases_to_tr() {
        let parse_ctx = ParseContext::from_packages(&["physics"]);
        let transform_ctx = TransformProfile::AUTHORING
            .builder()
            .build_with(&parse_ctx)
            .expect("transform context should build");
        for input in [r"\Tr", r"\trace", r"\Trace"] {
            let mut ast = parse_ctx
                .parse_to_ast(input, true)
                .expect("parse should succeed");
            let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
                .unwrap_or_else(|error| panic!("trace-to-tr transform should succeed: {error:?}"));

            assert_eq!(output.applied.len(), 1);
            assert_eq!(output.applied[0].key.to_string(), "physics/trace-to-tr");

            let root = ast.root();
            let children = ast.children(root);
            assert_eq!(children.len(), 1);

            match ast.node(children[0]) {
                Node::Command { name, args, .. } => {
                    assert_eq!(name, "tr");
                    assert!(args.is_empty());
                }
                other => panic!("expected tr command after transform, got {:?}", other),
            }
        }
    }
}
