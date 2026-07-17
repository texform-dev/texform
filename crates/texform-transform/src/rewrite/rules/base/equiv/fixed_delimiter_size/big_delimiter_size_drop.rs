//! Drop the fixed delimiter size imposed by \big.
//!
//! ```yaml
//! proposal: big-delimiter-size-drop
//! triggers:
//!   - cmd:big
//! consumes:
//!   eliminates: cmd:big
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: ordinary-delimiter, from: \big (, to: (}
//!   - {label: scripted-ordinary-delimiter, from: '\big (_{#1}', to: '(_{#1}'}
//!   - {label: literal-left-angle, from: \big <, to: \langle}
//!   - {label: literal-right-angle, from: \big >, to: \rangle}
//!   - {label: control-left-angle, from: \big\lt, to: \langle}
//!   - {label: control-right-angle, from: \big\gt, to: \rangle}
//!   - {label: null-delimiter, from: \big ., to: ''}
//!   - {label: scripted-null-delimiter, from: '\big ._{#1}', to: '{}_{#1}'}
//! ```

use texform_knowledge::builtin::base;

use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static BIG_DELIMITER_SIZE_DROP: BigDelimiterSizeDropRule {
        key: Base / "big-delimiter-size-drop",
        level: Equiv,
        summary: "Drop the fixed delimiter size imposed by \\big.",
        fidelity: Reading,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::BIG],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::BIG],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::BIG) else {
                return Ok(RuleEffect::Skipped);
            };
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(command.args, 1, r"\big")?;
            let delimiter =
                scoped.mandatory_delimiter(&command.args[0], r"\big", "argument")?;

            super::helpers::drop_fixed_delimiter_size(cx, node_id, delimiter);
            Ok(RuleEffect::Applied)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Argument, ArgumentKind, ArgumentSlot, ArgumentValue, Ast, Node};
    use crate::parse::ParseContext;
    use crate::rewrite::{
        RewriteError, RuleError, RuleLevel, run_one_rule_for_test, transform_examples,
    };

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: BIG_DELIMITER_SIZE_DROP,
        level: Equiv,
        examples: [
        {
            label: ordinary_delimiter,
            packages: ["base"],
            input: r"a\big(b",
            expected: r"a(b",
        },
        {
            label: scripted_ordinary_delimiter,
            packages: ["base"],
            input: r"a\big(_i b",
            expected: r"a(_i b",
        },
        {
            label: literal_left_angle,
            packages: ["base"],
            input: r"a\big<b",
            expected: r"a\langle b",
        },
        {
            label: literal_right_angle,
            packages: ["base"],
            input: r"a\big>b",
            expected: r"a\rangle b",
        },
        {
            label: control_left_angle,
            packages: ["base"],
            input: r"a\big\lt b",
            expected: r"a\langle b",
        },
        {
            label: control_right_angle,
            packages: ["base"],
            input: r"a\big\gt b",
            expected: r"a\rangle b",
        },
        {
            label: null_delimiter,
            packages: ["base"],
            input: r"a\big.b",
            expected: r"ab",
        },
        {
            label: scripted_null_delimiter,
            packages: ["base"],
            input: r"a\big._i b",
            expected: r"a{}_i b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: BIG_DELIMITER_SIZE_DROP,
        level: Equiv,
        examples: [
        {
            label: preserves_other_control_delimiter_identity,
            packages: ["base"],
            input: r"a\big\lbrace b",
            expected: r"a\lbrace b",
        },
        {
            label: preserves_required_argument_slot_for_null_delimiter,
            packages: ["base"],
            input: r"\sqrt\big.",
            expected: r"\sqrt{}",
        },
        ]
    }

    #[test]
    fn rejects_malformed_delimiter_arguments_without_mutating_ast() {
        let malformed_args = [
            Vec::new(),
            vec![None],
            vec![Some(Argument::from_value(
                ArgumentKind::Mandatory,
                ArgumentValue::Dimension("1pt".to_string()),
            ))],
        ];

        for args in malformed_args {
            assert_malformed_argument_is_rejected(args);
        }
    }

    fn assert_malformed_argument_is_rejected(args: Vec<ArgumentSlot>) {
        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = Ast::new();
        let command = ast.new_node(Node::Command {
            name: "big".to_string(),
            args,
            known: true,
        });
        ast.append_child(ast.root(), command);
        let before = ast.to_syntax_root();

        let error = run_one_rule_for_test(
            &mut ast,
            &parse_ctx,
            &BIG_DELIMITER_SIZE_DROP,
            RuleLevel::Equiv,
        )
        .expect_err("malformed delimiter argument should fail");

        assert!(matches!(
            error,
            crate::TransformError::Rewrite(RewriteError::Rule {
                kind: RuleError::InvalidNodeShape { .. },
                ..
            })
        ));
        assert_eq!(ast.to_syntax_root(), before);
        ast.assert_invariants();
    }
}
