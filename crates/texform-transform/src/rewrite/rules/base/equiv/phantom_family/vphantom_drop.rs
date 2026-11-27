//! Drop math-mode vertical phantoms while preserving argument slots and script attachment.
//!
//! ```yaml
//! proposal: vphantom-drop
//! triggers:
//!   - cmd:vphantom
//! consumes:
//!   eliminates: null
//!   touches: cmd:vphantom
//! produces: null
//! rewrite_patterns:
//!   - {label: sequence, from: '\vphantom{#1}', to: ''}
//!   - {label: script-base, from: '\vphantom{#1}_{#2}^{#3}', to: '{}_{#2}^{#3}'}
//!   - {label: required-slot, from: '\frac{\vphantom{#1}}{#2}', to: '\frac{}{#2}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::ContentMode;
use crate::rewrite::helpers::{
    active_command_is_builtin, content_mode_at, remove_node_preserving_slot,
};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static VPHANTOM_DROP: VphantomDropRule {
        key: Base / "vphantom-drop",
        level: Equiv,
        summary: "Drop math-mode vertical phantoms while preserving argument slots and script attachment.",
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::VPHANTOM],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::VPHANTOM],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::VPHANTOM) else {
                return Ok(RuleEffect::Skipped);
            };
            // The same-name textmacros record is the text-mode reading and
            // stays in place; only the base math record is a pure vertical strut.
            let is_math_record = content_mode_at(cx.ast, node_id) == Some(ContentMode::Math)
                && cx
                    .lookup_command(command.name, ContentMode::Math)
                    .is_some_and(|active| {
                        active_command_is_builtin(active, &base::cmd::VPHANTOM, "base", false)
                    });
            if !is_math_record {
                return Ok(RuleEffect::Skipped);
            }
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(command.args, 1, r"\vphantom")?;
            scoped.mandatory_math_content(&command.args[0], r"\vphantom", "argument")?;

            // The hidden argument goes with the node. Script bases and
            // single-child slots keep an empty math group so scripts do not
            // reattach to a preceding atom and the vacated argument stays filled.
            remove_node_preserving_slot(cx.ast, node_id);
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
        rule: VPHANTOM_DROP,
        level: Equiv,
        examples: [
        {
            label: sequence,
            packages: ["base"],
            input: r"a\vphantom{x}b",
            expected: r"ab",
        },
        {
            label: script_base,
            packages: ["base"],
            input: r"x\vphantom{\frac{1}{2}}_i^j",
            expected: r"x{}_i^j",
        },
        {
            label: required_slot,
            packages: ["base"],
            input: r"\frac{\vphantom{x}}{2}",
            expected: r"\frac{}{2}",
        },
        {
            label: eval_expanded_fence,
            packages: ["base", "physics"],
            input: r"\left.F(x)\vphantom{\int}\right|_a^b",
            expected: r"\left.F(x)\right|_a^b",
        },
        {
            label: eval_expanded_star_fence,
            packages: ["base", "physics"],
            input: r"\left.\smash{F(x)}\vphantom{\int}\right|_a^b",
            expected: r"\left.\smash{F(x)}\right|_a^b",
        },
        {
            label: empty_matrix_cell,
            packages: ["base", "ams"],
            input: r"\begin{matrix}\vphantom{\frac{1}{2}}&a\\b&c\end{matrix}",
            expected: r"\begin{matrix}&a\\b&c\end{matrix}",
        },
        {
            label: math_and_text_records,
            packages: ["base", "textmacros"],
            input: r"x\vphantom{y}+\text{a\vphantom{b}c}",
            expected: r"x+\text{a\vphantom{b}c}",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: VPHANTOM_DROP,
        level: Equiv,
        examples: [
        {
            label: drops_brace_less_script_operand,
            packages: ["base"],
            input: r"x_\vphantom{y}^2",
            expected: r"x_{}^2",
        },
        {
            label: drops_brace_less_required_argument,
            packages: ["base"],
            input: r"\frac\vphantom{x}2",
            expected: r"\frac{}2",
        },
        {
            label: drops_sole_radicand,
            packages: ["base"],
            input: r"\sqrt{\vphantom{x}}",
            expected: r"\sqrt{}",
        },
        {
            label: drops_nested_phantom_with_its_argument,
            packages: ["base"],
            input: r"a\vphantom{\vphantom{b}c}d",
            expected: r"ad",
        },
        {
            label: drops_inline_math_inside_text,
            packages: ["base", "textmacros"],
            input: r"\text{a$\vphantom{x}y$b}",
            expected: r"\text{a$y$b}",
        },
        {
            label: keeps_text_record_as_direct_text_argument,
            packages: ["base", "textmacros"],
            input: r"\text\vphantom{b}",
            expected: r"\text\vphantom{b}",
        },
        {
            label: leaves_horizontal_phantoms,
            packages: ["base"],
            input: r"\phantom{x}\hphantom{y}",
            expected: r"\phantom{x}\hphantom{y}",
        },
        ]
    }

    #[test]
    fn script_base_keeps_an_empty_math_group() {
        use crate::ast::{GroupKind, Node};
        use crate::parse::{ParseConfig, ParseContext};
        use crate::rewrite::{RuleLevel, run_one_rule_for_test};

        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast =
            crate::parse_to_ast_for_test(&parse_ctx, r"x\vphantom{y}_i", &ParseConfig::STRICT);
        run_one_rule_for_test(&mut ast, &parse_ctx, &VPHANTOM_DROP, RuleLevel::Equiv).unwrap();

        let scripted = ast.children(ast.root())[1];
        let Node::Scripted { base, .. } = ast.node(scripted) else {
            panic!("the phantom's scripts should stay on their own node");
        };
        assert_eq!(
            ast.node(*base),
            &Node::Group {
                children: Vec::new(),
                kind: GroupKind::Implicit,
                mode: ContentMode::Math,
            }
        );
        ast.assert_invariants();
    }

    #[test]
    fn equiv_profile_drops_phantoms_from_expanded_eval() {
        use crate::parse::{ParseConfig, ParseContext};
        use crate::{BuildConfig, Profile, TransformContext};

        let parse_ctx = ParseContext::from_packages(&["base", "physics"]);
        let engine =
            TransformContext::from_build_config(BuildConfig::profile(Profile::Equiv), &parse_ctx)
                .unwrap();
        for input in [r"\eval{F(x)}_a^b", r"\eval*{F(x)}_a^b"] {
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
            engine.run(&mut ast, &parse_ctx).unwrap();
            let output = crate::serialize::serialize(&ast);
            assert!(!output.contains("vphantom"), "{input} -> {output}");
            ast.assert_invariants();
        }
    }
}
