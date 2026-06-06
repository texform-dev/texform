//! Convert the prime control sequence to the Prime AST node.
//!
//! ```yaml
//! proposal: prime-to-prime-node
//! triggers:
//!   - char:prime
//! consumes:
//!   eliminates: char:prime
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {from: \prime, to: ''''}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::Node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{char_targets, define_rule};

define_rule! {
    pub static PRIME_TO_PRIME_NODE: PrimeToPrimeNodeRule {
        key: Base / "prime-to-prime-node",
        class: Standard,
        summary: "Convert the prime control sequence to the Prime AST node.",
        safety: Lossless,
        enabled_by_packages: [Base],
        triggers: char_targets![&base::chars::PRIME],
        consumes: RuleConsumes {
            eliminates: char_targets![&base::chars::PRIME],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(rule, cx, node_id) {
            let args = match cx.node(node_id) {
                Node::Command { name, args, .. } if name == base::chars::PRIME.name => args,
                _ => return Ok(RuleEffect::Skipped),
            };
            cx.for_rule(Self::KEY).expect_no_args(args, r"\prime")?;

            cx.ast.replace_node(node_id, Node::Prime { count: 1 });
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
        rule: PRIME_TO_PRIME_NODE,
        class: Standard,
        examples: [
        {
            label: script_superscript_shorthand,
            packages: ["base"],
            input: r"f^\prime",
            expected: r"f'",
        },
        {
            label: subscripted_symbol_prime,
            packages: ["base"],
            input: r"F_\nu^\prime",
            expected: r"F_\nu'",
        },
        ]
    }
    // END: Generated examples
}
