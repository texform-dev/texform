//! Rewrite a mathtools matrix* environment with the default center alignment to the AMS matrix environment.
//!
//! ```yaml
//! proposal: matrix-star-default-align-to-matrix
//! triggers:
//!   - env:matrix*
//! consumes:
//!   eliminates: null
//!   touches: env:matrix*
//! produces: env:matrix
//! rewrite_patterns:
//!   - {label: no-alignment, from: '\begin{matrix*} #1 \end{matrix*}', to: '\begin{matrix} #1 \end{matrix}'}
//!   - {label: center-alignment, from: '\begin{matrix*}[c] #1 \end{matrix*}', to: '\begin{matrix} #1 \end{matrix}'}
//! ```

use texform_knowledge::builtin::ams;
use texform_knowledge::builtin::mathtools;

use crate::ast::{Argument, ArgumentValue, Node};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{define_rule, env_targets};

define_rule! {
    pub static MATRIX_STAR_DEFAULT_ALIGN_TO_MATRIX: MatrixStarDefaultAlignToMatrixRule {
        key: Mathtools / "matrix-star-default-align-to-matrix",
        level: Authoring,
        summary: "Rewrite a mathtools matrix* environment with the default center alignment to the AMS matrix environment.",
        enabled_by_packages: [Mathtools],
        triggers: env_targets![&mathtools::env::MATRIX_STAR],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: env_targets![&mathtools::env::MATRIX_STAR],
        },
        produces: RuleProduces {
            targets: env_targets![&ams::env::MATRIX],
        },
        apply(rule, cx, node_id) {
            let Some(env) = cx.match_environment(node_id, &mathtools::env::MATRIX_STAR) else {
                return Ok(RuleEffect::Skipped);
            };
            let subject = format!(r"\begin{{{}}}", env.name);
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(env.args, 1, &subject)?;
            // mathtools starts `matrix*` with `\MT_matrix_begin:N c` by default, which
            // is exactly the AMS `matrix` preamble. Any other alignment keeps `matrix*`.
            let default_alignment = match &env.args[0] {
                None => true,
                Some(Argument {
                    value: ArgumentValue::CSName(alignment),
                    ..
                }) => alignment.trim() == "c",
                Some(_) => {
                    return Err(
                        scoped.invalid_shape(format!("{subject} alignment should be a name argument"))
                    );
                }
            };
            if !default_alignment {
                return Ok(RuleEffect::Skipped);
            }
            let body = env.body;

            cx.ast.replace_node(
                node_id,
                Node::Environment {
                    name: ams::env::MATRIX.name.to_string(),
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
        rule: MATRIX_STAR_DEFAULT_ALIGN_TO_MATRIX,
        level: Authoring,
        examples: [
        {
            label: no_alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*} a & b \\ c & d \end{matrix*}",
            expected: r"\begin{matrix} a & b \\ c & d \end{matrix}",
        },
        {
            label: center_alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*}[c] -1 & 2 \\ 3 & -4 \end{matrix*}",
            expected: r"\begin{matrix} -1 & 2 \\ 3 & -4 \end{matrix}",
        },
        {
            label: right_alignment_kept,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*}[r] -1 & 2 \\ 3 & -4 \end{matrix*}",
            expected: r"\begin{matrix*}[r] -1 & 2 \\ 3 & -4 \end{matrix*}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: MATRIX_STAR_DEFAULT_ALIGN_TO_MATRIX,
        level: Authoring,
        examples: [
        {
            label: padded_center_alignment,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*}[ c ] a & b \end{matrix*}",
            expected: r"\begin{matrix} a & b \end{matrix}",
        },
        {
            label: left_alignment_kept,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*}[l] a & b \end{matrix*}",
            expected: r"\begin{matrix*}[l] a & b \end{matrix*}",
        },
        {
            label: nested_matrices_are_checked_independently,
            packages: ["base", "ams", "mathtools"],
            input: r"\begin{matrix*} \begin{matrix*}[r] a \end{matrix*} & b \end{matrix*}",
            expected: r"\begin{matrix} \begin{matrix*}[r] a \end{matrix*} & b \end{matrix}",
        },
        ]
    }

    /// The corpus fenced-matrix rules forward the alignment slot to `matrix*`,
    /// so a redundant `[c]` only collapses to `matrix` once both levels run.
    #[test]
    fn fenced_starred_matrices_chain_into_matrix_under_corpus() {
        let parse_ctx = crate::parse::ParseContext::from_packages(&["base", "ams", "mathtools"]);
        let parse_config = crate::parse::ParseConfig::STRICT;
        for (profile, input, expected) in [
            (
                crate::Profile::Authoring,
                r"\begin{pmatrix*}[c] a \end{pmatrix*}",
                r"\begin{pmatrix*}[c] a \end{pmatrix*}",
            ),
            (
                crate::Profile::Corpus,
                r"\begin{pmatrix*}[c] a \end{pmatrix*}",
                r"\left(\begin{matrix} a \end{matrix}\right)",
            ),
            (
                crate::Profile::Corpus,
                r"\begin{Bmatrix*} a \end{Bmatrix*}",
                r"\left\{\begin{matrix} a \end{matrix}\right\}",
            ),
            (
                crate::Profile::Corpus,
                r"\begin{bmatrix*}[r] a \end{bmatrix*}",
                r"\left[\begin{matrix*}[r] a \end{matrix*}\right]",
            ),
        ] {
            let transform_ctx = crate::TransformContext::from_build_config(
                crate::BuildConfig::profile(profile),
                &parse_ctx,
            )
            .expect("transform context should build");
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &parse_config);
            transform_ctx
                .run(&mut ast, &parse_ctx)
                .expect("transform should succeed");
            let expected_ast = crate::parse_to_ast_for_test(&parse_ctx, expected, &parse_config);
            assert_eq!(
                crate::serialize::serialize(&ast),
                crate::serialize::serialize(&expected_ast),
                "{profile:?}: {input}"
            );
        }
    }
}
