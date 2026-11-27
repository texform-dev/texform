//! Rewrite the display-style mathtools dcases environment to AMS cases.
//!
//! ```yaml
//! proposal: dcases-to-cases
//! triggers:
//!   - env:dcases
//! consumes:
//!   eliminates: env:dcases
//!   touches: null
//! produces: env:cases
//! rewrite_patterns:
//!   - {from: '\begin{dcases} #1 \end{dcases}', to: '\begin{cases} #1 \end{cases}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::mathtools;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static DCASES_TO_CASES: DcasesToCasesRule {
        key: Mathtools / "dcases-to-cases",
        level: Equiv,
        summary: "Rewrite the display-style mathtools dcases environment to AMS cases.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::DCASES],
        consumes: RuleConsumes {
            eliminates: env_targets![&mathtools::env::DCASES],
            touches: &[],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::CASES],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::DCASES) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            cx.for_rule(Self::KEY).expect_no_args(env.args, &subject)?;
            let body = env.body;

            cx.ast.replace_node(
                node_id,
                Node::Environment {
                    name: ams::env::CASES.name.to_string(),
                    args: Vec::new(),
                    known: true,
                    body,
                },
            );
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: DCASES_TO_CASES,
        level: Equiv,
        examples: [
        {
            label: plain_cells,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{dcases} x & x>0 \\ -x & x\le 0 \end{dcases}",
            expected: r"\begin{cases} x & x>0 \\ -x & x\le 0 \end{cases}",
        },
        {
            label: display_style_cells,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{dcases} \frac{1}{2} & \sum_{i=1}^{n} x_i>0 \\ 0 & \text{otherwise} \end{dcases}",
            expected: r"\begin{cases} \frac{1}{2} & \sum_{i=1}^{n} x_i>0 \\ 0 & \text{otherwise} \end{cases}",
        },
        ]
    }
    // END: Generated examples

    /// AMS supplies the produced `cases` environment, so without it the rule
    /// leaves the plan and `dcases` is kept instead of failing the build.
    #[test]
    fn disabled_without_ams_cases() {
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base", "mathtools"]);
        let transform_ctx = crate::TransformContext::from_build_config(
            crate::BuildConfig::profile(crate::Profile::Equiv),
            &parse_ctx,
        )
        .expect("transform context should build");
        let input = r"\begin{dcases} x & y \end{dcases}";
        let parse_config = crate::parse::ParseConfig::STRICT;
        let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &parse_config);
        let expected = crate::serialize::serialize(&ast);
        transform_ctx
            .run(&mut ast, &parse_ctx)
            .expect("transform should succeed");
        assert_eq!(crate::serialize::serialize(&ast), expected);
    }
}
