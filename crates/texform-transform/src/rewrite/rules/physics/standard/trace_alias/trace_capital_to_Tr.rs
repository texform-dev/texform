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

use crate::rewrite::alias_rule;

alias_rule! {
    pub static TRACE_CAPITAL_TO_TR: TraceCapitalToTrRule {
        key: Physics / "trace-capital-to-Tr",
        class: Standard,
        summary: "Collapse capital Trace to the local Tr anchor.",
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
    use crate::parse::Parser;
    use crate::rewrite::transform_examples;
    use crate::rewrite::{run_one_rule_for_test, RuleClass, RewriteRule as _};
    use crate::serialize::serialize;

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
        let parse_ctx = Parser::from_packages(&["base", "physics"]);
        let mut ast = parse_ctx
            .parse_to_ast(r"\Tr(M^2)=1", &texform_core::parse::ParseConfig::STRICT_NO_RECOVER)
            .expect("parse should succeed");
        let output = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &TRACE_CAPITAL_TO_TR,
            RuleClass::Standard,
        )
            .expect("transform should succeed");
        let expected_ast = parse_ctx
            .parse_to_ast(r"\Tr(M^2)=1", &texform_core::parse::ParseConfig::STRICT_NO_RECOVER)
            .expect("parse expected should succeed");

        assert!(output.rewrite.applied.is_empty());
        assert_eq!(serialize(&ast), serialize(&expected_ast));
    }
}
