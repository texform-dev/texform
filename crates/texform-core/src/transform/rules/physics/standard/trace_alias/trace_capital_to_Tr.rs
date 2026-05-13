//! Collapse capital Trace to the local Tr anchor.
//!
//! ```yaml
//! proposal: trace-capital-to-Tr
//! triggers:
//!   - cmd:Trace
//! consumes:
//!   eliminates: cmd:Trace
//!   touches: null
//! produces: cmd:Tr
//! rewrite_patterns:
//!   - {label: trace-capital, from: \Trace, to: \Tr}
//! ```

use texform_specs::builtin::physics;

use crate::transform::alias_rule;

alias_rule! {
    pub static TRACE_CAPITAL_TO_TR: TraceCapitalToTrRule {
        key: Physics / "trace-capital-to-Tr",
        class: Standard,
        summary: "Collapse capital Trace to the local Tr anchor.",
        phase: ApplyRules,
        safety: Lossless,
        enabled_by_packages: [Physics],
        canonical: &physics::cmd::TR_2,
        aliases: [
            &physics::cmd::TRACE_2,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ParseContext;
    use crate::serialize::serialize;
    use crate::transform::transform_examples;
    use crate::transform::{TransformContextBuilder, RuleClass, transform_ast};

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TRACE_CAPITAL_TO_TR,
        class: Standard,
        examples: [
        {
            label: trace_matrix_square,
            packages: ["base", "physics"],
            input: r"\Trace(M^2)=1",
            expected: r"\Tr(M^2)=1",
        },
        ]
    }
    // END: Generated examples

    #[test]
    fn leaves_existing_capital_tr_unchanged() {
        use crate::transform::TransformRule as _;

        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let transform_ctx = TransformContextBuilder::from_classes(&[RuleClass::Standard])
            .only(TRACE_CAPITAL_TO_TR.meta().key)
            .build_with(&parse_ctx)
            .expect("transform context should build");
        let mut ast = parse_ctx
            .parse_to_ast(r"\Tr(M^2)=1", true)
            .expect("parse should succeed");
        let output = transform_ast(&mut ast, &parse_ctx, &transform_ctx)
            .expect("transform should succeed");
        let expected_ast = parse_ctx
            .parse_to_ast(r"\Tr(M^2)=1", true)
            .expect("parse expected should succeed");

        assert!(output.applied.is_empty());
        assert_eq!(serialize(&ast), serialize(&expected_ast));
    }
}
