//! Profile-neutral AST representation canonicalization.
//!
//! Rewrite turns surface forms into semantic AST nodes. FinalizeAst then
//! converges semantically equivalent AST shapes into a canonical representation:
//! adjacent `Prime` merges, text-sequence merges, ordinary whitespace collapse,
//! and empty-text cleanup. Steps here do not depend on rewrite metadata,
//! packages, or profile rule sets.

use texform_core::lexer::is_whitespace_char;

use crate::ast::{ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};

/// Per-run switch for the FinalizeAst phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalizeAstConfig {
    /// Run the phase when `true`; skip it entirely when `false`. Enabled by
    /// default in every public profile.
    pub enabled: bool,
}

impl FinalizeAstConfig {
    pub const ENABLED: Self = Self { enabled: true };
    pub const DISABLED: Self = Self { enabled: false };
    pub const DEFAULTS: Self = Self::ENABLED;
}

/// What the FinalizeAst phase changed in the tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstReport {
    /// Per-step counters for the phase's cleanup passes.
    pub steps: FinalizeAstStepReports,
}

/// One report per FinalizeAst cleanup step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstStepReports {
    /// Counter for merging runs of adjacent `Prime` nodes into one.
    pub merge_adjacent_primes: FinalizeAstStepReport,
    /// Counter for text-sequence merge, whitespace collapse, and empty-text cleanup.
    pub normalize_text_sequences: FinalizeAstStepReport,
}

/// Activity counter for a single FinalizeAst step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FinalizeAstStepReport {
    /// Number of times the step rewrote part of the tree.
    pub applied_count: usize,
}

pub fn run(ast: &mut Ast, config: &FinalizeAstConfig, report: &mut FinalizeAstReport) {
    if !config.enabled {
        return;
    }

    visit(ast, ast.root(), report);
    // Debug-only structural contract check. `assert_invariants` is an
    // O(n * branching) full-tree sweep, so running it on every transform made
    // long, wide formulas quadratic in release. The rewrite scheduler gates its
    // own per-rule check the same way.
    #[cfg(debug_assertions)]
    ast.assert_invariants();
}

fn visit(ast: &mut Ast, node: NodeId, report: &mut FinalizeAstReport) {
    if is_math_sequence_container(ast, node) {
        merge_adjacent_primes(ast, node, report);
    }
    if is_text_sequence_container(ast, node) {
        normalize_text_sequences(ast, node, report);
        for child in ast.children(node).to_vec() {
            if ast.contains(child) && !matches!(ast.node(child), Node::Text(_)) {
                visit(ast, child, report);
            }
        }
        return;
    }

    if matches!(ast.node(node), Node::Text(_)) && is_text_content_argument(ast, node) {
        normalize_text_content_slot(ast, node, report);
        return;
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

fn is_text_sequence_container(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(node),
        Node::Root {
            mode: ContentMode::Text,
            ..
        } | Node::Group {
            mode: ContentMode::Text,
            ..
        }
    )
}

fn is_text_content_argument(ast: &Ast, node: NodeId) -> bool {
    let Some(ParentLink {
        parent,
        slot: Slot::Argument(index),
    }) = ast.parent(node)
    else {
        return false;
    };

    let args = match ast.node(parent) {
        Node::Command { args, .. }
        | Node::Infix { args, .. }
        | Node::Declarative { args, .. }
        | Node::Environment { args, .. } => args,
        _ => return false,
    };

    matches!(
        args.get(index).and_then(|arg| arg.as_ref()),
        Some(arg) if matches!(arg.value, ArgumentValue::TextContent(_))
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

fn normalize_text_sequences(ast: &mut Ast, parent: NodeId, report: &mut FinalizeAstReport) {
    let children = ast.children(parent).to_vec();
    let mut next_children = Vec::with_capacity(children.len());
    let mut index = 0;
    let mut changed = false;

    while index < children.len() {
        let Some((run_end, normalized)) = collect_text_run(ast, &children, index) else {
            next_children.push(children[index]);
            index += 1;
            continue;
        };

        let run_len = run_end - index;
        let singleton = run_len == 1;
        let original = match ast.node(children[index]) {
            Node::Text(text) => text.as_str(),
            _ => unreachable!("text run collector only returns Text nodes"),
        };

        if normalized.is_empty() {
            // Drop the whole empty run from the sequence container.
            report.steps.normalize_text_sequences.applied_count += 1;
            changed = true;
            index = run_end;
            continue;
        }

        if singleton && original == normalized.as_str() {
            // Already canonical non-empty singleton: keep the original node.
            next_children.push(children[index]);
            index = run_end;
            continue;
        }

        next_children.push(ast.new_node(Node::Text(normalized)));
        report.steps.normalize_text_sequences.applied_count += 1;
        changed = true;
        index = run_end;
    }

    if !changed {
        return;
    }

    for removed in ast.replace_children(parent, next_children) {
        ast.remove_detached(removed);
    }
}

fn collect_text_run(ast: &Ast, children: &[NodeId], start: usize) -> Option<(usize, String)> {
    let Node::Text(first) = ast.node(children.get(start).copied()?) else {
        return None;
    };

    let mut end = start + 1;
    let mut joined = first.clone();
    while let Some(child) = children.get(end) {
        let Node::Text(text) = ast.node(*child) else {
            break;
        };
        joined.push_str(text);
        end += 1;
    }

    Some((end, normalize_whitespace(&joined)))
}

fn normalize_text_content_slot(ast: &mut Ast, node: NodeId, report: &mut FinalizeAstReport) {
    let text = match ast.node(node) {
        Node::Text(text) => text.clone(),
        _ => return,
    };
    let normalized = normalize_whitespace(&text);

    if normalized.is_empty() {
        ast.replace_node_drop_detached_children(
            node,
            Node::Group {
                mode: ContentMode::Text,
                kind: GroupKind::Implicit,
                children: Vec::new(),
            },
        );
        report.steps.normalize_text_sequences.applied_count += 1;
        return;
    }

    if normalized == text {
        return;
    }

    ast.replace_node(node, Node::Text(normalized));
    report.steps.normalize_text_sequences.applied_count += 1;
}

fn normalize_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_ws = false;
    for ch in input.chars() {
        if is_whitespace_char(ch) {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(ch);
            in_ws = false;
        }
    }
    out
}
