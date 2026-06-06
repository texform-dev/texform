//! Final AST cleanup that does not depend on rewrite metadata.

use crate::ast::{Ast, ContentMode, Node, NodeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalizeAstConfig {
    pub enabled: bool,
}

impl FinalizeAstConfig {
    pub const ENABLED: Self = Self { enabled: true };
    pub const DISABLED: Self = Self { enabled: false };
    pub const DEFAULTS: Self = Self::ENABLED;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstReport {
    pub steps: FinalizeAstStepReports,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstStepReports {
    pub merge_adjacent_primes: FinalizeAstStepReport,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstStepReport {
    pub applied_count: usize,
}

pub fn run(ast: &mut Ast, config: &FinalizeAstConfig, report: &mut FinalizeAstReport) {
    if !config.enabled {
        return;
    }

    visit(ast, ast.root(), report);
    ast.assert_invariants();
}

fn visit(ast: &mut Ast, node: NodeId, report: &mut FinalizeAstReport) {
    if is_math_sequence_container(ast, node) {
        merge_adjacent_primes(ast, node, report);
    }

    for (child, _) in ast.edges(node) {
        if ast.contains(child) {
            visit(ast, child, report);
        }
    }
}

fn is_math_sequence_container(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Root {
            mode: ContentMode::Math,
            ..
        } | Node::Group {
            mode: ContentMode::Math,
            ..
        }
    )
}

fn merge_adjacent_primes(ast: &mut Ast, parent: NodeId, report: &mut FinalizeAstReport) {
    let children = ast.children(parent).to_vec();
    let mut next_children = Vec::with_capacity(children.len());
    let mut index = 0;
    let mut changed = false;

    while index < children.len() {
        let Some((count, next_index)) = collect_prime_run(ast, &children, index) else {
            next_children.push(children[index]);
            index += 1;
            continue;
        };

        next_children.push(ast.new_node(Node::Prime { count }));
        report.steps.merge_adjacent_primes.applied_count += 1;
        changed = true;
        index = next_index;
    }

    if !changed {
        return;
    }

    for removed in ast.replace_children(parent, next_children) {
        ast.remove_detached(removed);
    }
}

fn collect_prime_run(ast: &Ast, children: &[NodeId], start: usize) -> Option<(usize, usize)> {
    let mut index = start;
    let mut count = 0;

    while let Some(child) = children.get(index) {
        match ast.node(*child) {
            Node::Prime { count: prime_count } => {
                count += prime_count;
                index += 1;
            }
            _ => break,
        }
    }

    (index > start + 1).then_some((count, index))
}
