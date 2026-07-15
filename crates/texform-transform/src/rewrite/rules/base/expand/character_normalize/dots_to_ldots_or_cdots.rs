//! Resolve plain \dots to an explicit ellipsis only when the following math atom class determines MathJax's choice.
//!
//! ```yaml
//! proposal: dots-to-ldots-or-cdots
//! triggers:
//!   - cmd:dots
//! consumes:
//!   eliminates: null
//!   touches: cmd:dots
//! produces:
//!   - char:ldots
//!   - char:cdots
//! rewrite_patterns:
//!   - {label: following-non-bin-rel, from: '\dots #1', to: '\ldots #1'}
//!   - {label: following-bin-rel, from: '\dots #1', to: '\cdots #1'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{ContentMode, GroupKind, Node, NodeId};
use crate::rewrite::helpers::bare_command_node;
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::rule_context::RuleContext;
use crate::rewrite::{char_targets, cmd_targets, define_rule};

#[derive(Clone, Copy)]
enum DotsChoice {
    Ldots,
    Cdots,
}

fn following_atom_choice(cx: &RuleContext<'_>, node_id: NodeId) -> Option<DotsChoice> {
    match cx.node(node_id) {
        Node::Scripted { base, .. } => following_atom_choice(cx, *base),
        Node::Char(ch) => literal_character_choice(*ch),
        Node::Command { name, .. } => {
            let character = cx.lookup_character(name, ContentMode::Math)?;
            match character.attributes.tex_class.as_deref() {
                Some("BIN" | "Bin" | "bin" | "REL" | "Rel" | "rel") => {
                    Some(DotsChoice::Cdots)
                }
                Some(_) => Some(DotsChoice::Ldots),
                None => character
                    .unicode_value
                    .chars()
                    .next()
                    .filter(|_| character.unicode_value.chars().count() == 1)
                    .and_then(literal_character_choice),
            }
        }
        Node::Group { children, kind, .. } => match kind {
            GroupKind::Explicit | GroupKind::Implicit => match children.as_slice() {
                [] => Some(DotsChoice::Ldots),
                [core] => following_atom_choice(cx, *core),
                _ => None,
            },
            GroupKind::Delimited { .. } | GroupKind::InlineMath => Some(DotsChoice::Ldots),
        },
        Node::Environment { .. } => Some(DotsChoice::Ldots),
        Node::Prime { .. } | Node::ActiveSpace => Some(DotsChoice::Ldots),
        Node::Root { .. }
        | Node::Infix { .. }
        | Node::Declarative { .. }
        | Node::Text(_)
        | Node::Error { .. } => None,
    }
}

fn literal_character_choice(ch: char) -> Option<DotsChoice> {
    match ch {
        '+' | '-' | '*' | ':' | '=' | '<' | '>' => Some(DotsChoice::Cdots),
        ch if ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                ',' | '.' | '/' | ';' | '!' | '?' | '(' | ')' | '[' | ']' | '|'
            ) =>
        {
            Some(DotsChoice::Ldots)
        }
        _ => None,
    }
}

define_rule! {
    pub static DOTS_TO_LDOTS_OR_CDOTS: DotsToLdotsOrCdotsRule {
        key: Base / "dots-to-ldots-or-cdots",
        level: Expand,
        summary: "Resolve plain \\dots to an explicit ellipsis only when the following math atom class determines MathJax's choice.",
        fidelity: Full,
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::DOTS],
        consumes: RuleConsumes {
            eliminates: &[],
            touches: cmd_targets![&base::cmd::DOTS],
        },
        produces: RuleProduces {
            targets: char_targets![&base::chars::LDOTS, &base::chars::CDOTS],
        },
        apply(rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::DOTS) else {
                return Ok(RuleEffect::Skipped);
            };
            cx.for_rule(Self::KEY).expect_no_args(command.args, r"\dots")?;

            let Some(next) = cx.ast.next_sibling(node_id) else {
                return Ok(RuleEffect::Skipped);
            };
            let Some(choice) = following_atom_choice(cx, next) else {
                return Ok(RuleEffect::Skipped);
            };
            let replacement = match choice {
                DotsChoice::Ldots => base::chars::LDOTS.name,
                DotsChoice::Cdots => base::chars::CDOTS.name,
            };
            cx.ast.replace_node(node_id, bare_command_node(replacement));
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
        rule: DOTS_TO_LDOTS_OR_CDOTS,
        level: Expand,
        examples: [
        {
            label: following_non_bin_rel,
            packages: ["base"],
            input: r"a,\dots,b",
            expected: r"a,\ldots,b",
        },
        {
            label: following_bin_rel,
            packages: ["base"],
            input: r"a\dots=b",
            expected: r"a\cdots=b",
        },
        {
            label: right_boundary_preserved,
            packages: ["base"],
            input: r"x+\dots",
            expected: r"x+\dots",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: DOTS_TO_LDOTS_OR_CDOTS,
        level: Expand,
        examples: [
        {
            label: scripted_relation_core,
            packages: ["base"],
            input: r"a\dots=^2b",
            expected: r"a\cdots=^2b",
        },
        {
            label: explicit_group_is_non_bin_rel,
            packages: ["base"],
            input: r"\dots{x}",
            expected: r"\ldots{x}",
        },
        {
            label: colon_relation_uses_cdots,
            packages: ["base"],
            input: r"a\dots:b",
            expected: r"a\cdots:b",
        },
        {
            label: grouped_relation_core_uses_cdots,
            packages: ["base"],
            input: r"a\dots{=}b",
            expected: r"a\cdots{=}b",
        },
        {
            label: unclassifiable_prefix_is_preserved,
            packages: ["base"],
            input: r"\dots\frac{1}{2}",
            expected: r"\dots\frac{1}{2}",
        },
        ]
    }
}
