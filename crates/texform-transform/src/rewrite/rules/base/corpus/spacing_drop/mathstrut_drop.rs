//! Drop the invisible mathstrut layout helper in cleanup-oriented normalization.
//!
//! ```yaml
//! proposal: mathstrut-drop
//! triggers:
//!   - cmd:mathstrut
//! consumes:
//!   eliminates: cmd:mathstrut
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \mathstrut, to: ''}
//! ```

use texform_knowledge::builtin::base;

use super::helpers::drop_layout_hint;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static MATHSTRUT_DROP: MathstrutDropRule {
        key: Base / "mathstrut-drop",
        level: Corpus,
        summary: "Drop the invisible mathstrut layout helper in cleanup-oriented normalization.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::MATHSTRUT],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::MATHSTRUT],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::MATHSTRUT) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, "\\mathstrut")?;

            drop_layout_hint(cx.ast, node_id);
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
        rule: MATHSTRUT_DROP,
        level: Corpus,
        examples: [
        {
            label: mathstrut_after_tall_term,
            packages: ["base"],
            input: r"x_1 + y^2\mathstrut + z_3",
            expected: r"x_1 + y^2 + z_3",
        },
        {
            label: mathstrut_inside_radical_changes_root_size,
            packages: ["base"],
            input: r"\sqrt{x\mathstrut}+\sqrt{x^i\mathstrut}",
            expected: r"\sqrt{x}+\sqrt{x^i}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: MATHSTRUT_DROP,
        level: Corpus,
        examples: [
        {
            label: mathstrut_as_script_base,
            packages: ["base"],
            input: r"\mathstrut^2",
            expected: r"{}^2",
        },
        ]
    }

    #[test]
    fn empty_text_arguments_serialize_without_visible_spaces() {
        use crate::ast::Node;
        use crate::parse::{ParseConfig, ParseContext};
        use crate::serialize::{SerializeOptions, Style, serialize_with};
        use crate::{BuildConfig, Profile, TransformContext};

        let parse_ctx = ParseContext::from_packages(&["base", "ams", "textmacros"]);
        let engine =
            TransformContext::from_build_config(BuildConfig::profile(Profile::Corpus), &parse_ctx)
                .unwrap();
        for (input, spaced, compact) in [
            (r"\text\mathstrut", r"\text {}", r"\text{}"),
            (r"\text{\mathstrut}", r"\text {}", r"\text{}"),
            (r"\text{}", r"\text {}", r"\text{}"),
            (r"\tag \mathstrut[", r"\tag {} [", r"\tag{}["),
        ] {
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
            engine.run(&mut ast, &parse_ctx).unwrap();
            ast.assert_invariants();
            for (style, expected) in [(Style::Spaced, spaced), (Style::Compact, compact)] {
                let output = serialize_with(&ast, &SerializeOptions { style });
                assert_eq!(output, expected, "{input}");
                let reparsed =
                    crate::parse_to_ast_for_test(&parse_ctx, &output, &ParseConfig::STRICT);
                assert!(
                    reparsed
                        .find_all(reparsed.root(), |node| matches!(node, Node::Text(_)))
                        .is_empty(),
                    "{input}"
                );
            }
        }
    }
}
