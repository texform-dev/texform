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

use texform_knowledge::builtin::physics;

use crate::rewrite::alias_rule;

alias_rule! {
    pub static TRACE_CAPITAL_TO_TR: TraceCapitalToTrRule {
        key: Physics / "trace-capital-to-Tr",
        level: Standard,
        summary: "Collapse capital Trace to the local Tr anchor.",
        fidelity: Lossless,
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
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, NormalizationLevel};
    use crate::serialize::serialize;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: TRACE_CAPITAL_TO_TR,
        level: Standard,
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
        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, r"\Tr(M^2)=1", &texform_core::parse::ParseConfig::STRICT);
        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &TRACE_CAPITAL_TO_TR,
            NormalizationLevel::Standard,
        )
            .expect("transform should succeed");
        let expected_ast = crate::parse_to_ast_for_test(&parse_ctx, r"\Tr(M^2)=1", &texform_core::parse::ParseConfig::STRICT);

        assert!(output.rewrite.rules.is_empty());
        assert_eq!(serialize(&ast), serialize(&expected_ast));
    }
}
