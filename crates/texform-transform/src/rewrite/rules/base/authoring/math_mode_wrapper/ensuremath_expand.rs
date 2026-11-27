//! Expand ensuremath to its math content: in place in math mode, as inline math in text mode.
//!
//! ```yaml
//! proposal: ensuremath-expand
//! triggers:
//!   - cmd:ensuremath
//! consumes:
//!   eliminates: cmd:ensuremath
//!   touches: null
//! produces: null
//! rewrite_patterns:
//!   - {label: math-splice, from: '\ensuremath{#1}', to: '#1'}
//!   - {label: scripted-base, from: '\ensuremath{#1 #2}^{#3}', to: '#1 #2^{#3}'}
//!   - {label: script-slot, from: '^\ensuremath{#1 #2}', to: '^{#1} #2'}
//!   - {label: text-mode, from: '\text{\ensuremath{#1}}', to: '\text{$#1$}'}
//! ```

use texform_knowledge::builtin::base;

use crate::ast::{Ast, ContentMode, GroupKind, Node, NodeId, ParentLink, Slot};
use crate::rewrite::helpers::{content_mode_at, remove_node_preserving_slot};
use crate::rewrite::rule::{RuleConsumes, RuleEffect, RuleProduces};
use crate::rewrite::{cmd_targets, define_rule};

define_rule! {
    pub static ENSUREMATH_EXPAND: EnsuremathExpandRule {
        key: Base / "ensuremath-expand",
        level: Authoring,
        summary: "Expand ensuremath to its math content: in place in math mode, as inline math in text mode.",
        enabled_by_packages: [Base],
        triggers: cmd_targets![&base::cmd::ENSUREMATH],
        consumes: RuleConsumes {
            eliminates: cmd_targets![&base::cmd::ENSUREMATH],
            touches: &[],
        },
        produces: RuleProduces {
            targets: &[],
        },
        apply(_rule, cx, node_id) {
            let Some(command) = cx.match_command(node_id, &base::cmd::ENSUREMATH) else {
                return Ok(RuleEffect::Skipped);
            };
            let scoped = cx.for_rule(Self::KEY);
            scoped.expect_arg_len(command.args, 1, r"\ensuremath")?;
            let content =
                scoped.mandatory_math_content(&command.args[0], r"\ensuremath", "argument")?;
            let mode = content_mode_at(cx.ast, node_id);
            let parent = cx.ast.parent(node_id);

            let nodes = take_content(cx.ast, content);
            match (mode, parent) {
                (Some(ContentMode::Text), _) => expand_in_text(cx.ast, node_id, nodes),
                (
                    _,
                    Some(ParentLink {
                        parent,
                        slot: Slot::ScriptBase,
                    }),
                ) => expand_script_base(cx.ast, node_id, parent, nodes),
                (
                    _,
                    Some(ParentLink {
                        parent,
                        slot: slot @ (Slot::ScriptSub | Slot::ScriptSup),
                    }),
                ) => expand_script(cx.ast, node_id, parent, slot, nodes),
                // A sequence child is spliced into its parent; a brace-less
                // macro argument or infix operand gets its content in place,
                // grouped when it has several nodes.
                _ => replace_with_sequence(cx.ast, node_id, nodes),
            }
            Ok(RuleEffect::Applied)
        }
    }
}

/// Detaches the content nodes of the argument. The parser wraps a multi-node
/// brace argument in an implicit group, whose children are the content; an
/// explicit group is a user-written brace pair and stays one content node.
fn take_content(ast: &mut Ast, content: NodeId) -> Vec<NodeId> {
    match ast.node(content) {
        Node::Group {
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
            children,
        } => {
            let len = children.len();
            ast.detach_children_range(content, 0..len)
        }
        _ => {
            let placeholder = ast.implicit_math_group(Vec::new());
            ast.replace_content_child(content, placeholder);
            vec![content]
        }
    }
}

/// Text mode typesets the content as inline math. Empty content is dropped
/// instead, since an empty `$$` would open display math.
fn expand_in_text(ast: &mut Ast, node_id: NodeId, nodes: Vec<NodeId>) {
    if nodes.is_empty() {
        remove_node_preserving_slot(ast, node_id);
        return;
    }
    ast.replace_node_drop_detached_children(
        node_id,
        Node::Group {
            children: nodes,
            kind: GroupKind::InlineMath,
            mode: ContentMode::Math,
        },
    );
}

fn replace_with_sequence(ast: &mut Ast, node_id: NodeId, mut nodes: Vec<NodeId>) {
    match nodes.pop() {
        Some(last) => ast.replace_with_math_sequence(node_id, nodes, last, Vec::new()),
        None => remove_node_preserving_slot(ast, node_id),
    }
}

/// The scripts of `\ensuremath{a b}^2` attach to the last content node, as in
/// the expansion `a b^2`; the other nodes precede the scripted node.
fn expand_script_base(ast: &mut Ast, node_id: NodeId, scripted: NodeId, mut nodes: Vec<NodeId>) {
    let Some(last) = nodes.pop() else {
        remove_node_preserving_slot(ast, node_id);
        return;
    };
    let (_, subscript, superscript) = release_scripted(ast, scripted, node_id);
    let last = attach_scripts(ast, last, subscript, superscript);
    ast.replace_with_math_sequence(scripted, nodes, last, Vec::new());
}

/// `^` and `_` take only the first math field of the expansion, so the first
/// content node becomes the script and the rest follow the scripted node:
/// `x^\ensuremath{ab}` reads as `x^{a}b`. When the content is empty, the
/// script is the next sibling: `x^\ensuremath{}y` reads as `x^y`. A first
/// field that carries scripts gives its base to the slot and its scripts to the
/// outer node: `x^\ensuremath{a_1 b}` reads as `x^a_1 b`.
///
/// Scripts are read in the serialized subscript-then-superscript order, so a
/// superscript after an expanded subscript attaches to the last trailing node:
/// `x_\ensuremath{ab}^c` reads as `x_{a}b^c`. A double script, which LaTeX
/// rejects, keeps the first field whole in the slot.
fn expand_script(
    ast: &mut Ast,
    node_id: NodeId,
    scripted: NodeId,
    slot: Slot,
    nodes: Vec<NodeId>,
) {
    let Node::Scripted {
        subscript,
        superscript,
        ..
    } = *ast.node(scripted)
    else {
        unreachable!("a script slot belongs to a scripted node");
    };
    let mut nodes = nodes.into_iter();
    let (first, after, from_sibling) = match nodes.next() {
        Some(first) => (first, nodes.collect::<Vec<_>>(), false),
        // A serialized superscript follows an expanded subscript directly,
        // so it, not the next sibling, would be read as the subscript.
        None => match next_math_field(ast, scripted)
            .filter(|_| slot == Slot::ScriptSup || superscript.is_none())
        {
            Some(next) => (next, Vec::new(), true),
            None => {
                remove_node_preserving_slot(ast, node_id);
                return;
            }
        },
    };

    let (head, head_events) = script_parts(ast, first);
    let mut events = Vec::new();
    if slot == Slot::ScriptSub {
        events.push(ScriptEvent::Sub(head));
        events.extend(head_events);
        if after.is_empty() {
            events.extend(superscript_events(ast, superscript));
        }
    } else {
        events.extend(subscript.map(ScriptEvent::Sub));
        events.push(ScriptEvent::Sup(head));
        events.extend(head_events);
    }
    let Some(fold) = ScriptFold::read(&events) else {
        if from_sibling {
            remove_node_preserving_slot(ast, node_id);
        } else {
            expand_script_whole(ast, node_id, scripted, slot, first, after);
        }
        return;
    };

    if from_sibling {
        ast.detach(first);
    }
    let (base, _, superscript) = release_scripted(ast, scripted, node_id);
    release_script_parts(ast, first);
    let mut after = after;
    if slot == Slot::ScriptSub {
        match after.pop() {
            Some(last) => after.push(attach_scripts(ast, last, None, superscript)),
            None => release_superscript(ast, superscript),
        }
    }
    let scripted_node = fold.build(ast, base);
    ast.replace_with_math_sequence(scripted, Vec::new(), scripted_node, after);
}

/// The sibling after `scripted` that `^` or `_` would read once an empty
/// expansion leaves nothing in between. Text, errors, and cell or row
/// separators are not math fields; the input is then a LaTeX error.
fn next_math_field(ast: &Ast, scripted: NodeId) -> Option<NodeId> {
    let next = ast.next_sibling(scripted)?;
    let head = match *ast.node(next) {
        Node::Scripted { base, .. } => base,
        _ => next,
    };
    match ast.node(head) {
        Node::Text(_)
        | Node::Error { .. }
        | Node::Prime { .. }
        | Node::AlignmentTab
        | Node::Declarative { .. } => None,
        Node::Command { name, .. }
            if name == base::cmd::_BACKSLASH.name || name == base::cmd::CR.name =>
        {
            None
        }
        _ => Some(next),
    }
}

/// Puts the whole first field in the script slot, for inputs that LaTeX
/// rejects with a double script.
fn expand_script_whole(
    ast: &mut Ast,
    node_id: NodeId,
    scripted: NodeId,
    slot: Slot,
    script: NodeId,
    mut after: Vec<NodeId>,
) {
    let (base, mut subscript, mut superscript) = release_scripted(ast, scripted, node_id);
    if slot == Slot::ScriptSub {
        subscript = Some(script);
        if let Some(last) = after.pop() {
            after.push(attach_scripts(ast, last, None, superscript.take()));
        }
    } else {
        superscript = Some(script);
    }
    let scripted_node = ast.new_node(Node::Scripted {
        base,
        subscript,
        superscript,
    });
    ast.replace_with_math_sequence(scripted, Vec::new(), scripted_node, after);
}

/// Empties `scripted` into a placeholder group, removes the expanded
/// `\ensuremath` node, and returns the remaining base and scripts as detached
/// roots.
fn release_scripted(
    ast: &mut Ast,
    scripted: NodeId,
    node_id: NodeId,
) -> (NodeId, Option<NodeId>, Option<NodeId>) {
    let Node::Scripted {
        base,
        subscript,
        superscript,
    } = *ast.node(scripted)
    else {
        unreachable!("a script slot belongs to a scripted node");
    };
    ast.replace_node(scripted, empty_math_group());
    ast.remove_detached(node_id);
    let keep = |id: NodeId| (id != node_id).then_some(id);
    (
        base,
        subscript.and_then(keep),
        superscript.and_then(keep),
    )
}

/// Attaches detached scripts to the detached `target`. When the target already
/// carries scripts, all of them are read in order as the parser would read
/// the expansion, so `\ensuremath{f'}^2` gives the tree of `f'^2`. A double
/// script, which LaTeX rejects, keeps the target as a braced base.
fn attach_scripts(
    ast: &mut Ast,
    target: NodeId,
    subscript: Option<NodeId>,
    superscript: Option<NodeId>,
) -> NodeId {
    if subscript.is_none() && superscript.is_none() {
        return target;
    }
    let base = match ast.node(target) {
        Node::Scripted { .. } => {
            let (base, mut events) = script_parts(ast, target);
            events.extend(subscript.map(ScriptEvent::Sub));
            events.extend(superscript_events(ast, superscript));
            if let Some(fold) = ScriptFold::read(&events) {
                release_script_parts(ast, target);
                release_superscript(ast, superscript);
                return fold.build(ast, base);
            }
            ast.implicit_math_group(vec![target])
        }
        _ => target,
    };
    ast.new_node(Node::Scripted {
        base,
        subscript,
        superscript,
    })
}

/// A script attachment, in the order the parser reads it after a base.
#[derive(Clone, Copy)]
enum ScriptEvent {
    Sub(NodeId),
    /// A run of this many primes.
    Primes(usize),
    Sup(NodeId),
}

enum Superscript {
    Primes(usize),
    Explicit(NodeId),
    /// A prime run followed by `^`, read as one superscript.
    Merged(usize, NodeId),
}

/// Scripts read in order onto one base, mirroring the parser: a prime run
/// followed by `^` merges into one superscript and adjacent prime runs join,
/// while any other repeated subscript or superscript is a double script.
#[derive(Default)]
struct ScriptFold {
    subscript: Option<NodeId>,
    superscript: Option<Superscript>,
}

impl ScriptFold {
    /// Returns `None` on a double script.
    fn read(events: &[ScriptEvent]) -> Option<Self> {
        let mut fold = Self::default();
        let mut after_primes = false;
        for event in events {
            let extends_primes = std::mem::replace(&mut after_primes, false);
            match *event {
                ScriptEvent::Sub(node) => {
                    if fold.subscript.replace(node).is_some() {
                        return None;
                    }
                }
                ScriptEvent::Primes(count) => {
                    fold.superscript = Some(match fold.superscript.take() {
                        None => Superscript::Primes(count),
                        Some(Superscript::Primes(previous)) if extends_primes => {
                            Superscript::Primes(previous + count)
                        }
                        Some(_) => return None,
                    });
                    after_primes = true;
                }
                ScriptEvent::Sup(node) => {
                    fold.superscript = Some(match fold.superscript.take() {
                        None => Superscript::Explicit(node),
                        Some(Superscript::Primes(count)) => Superscript::Merged(count, node),
                        Some(_) => return None,
                    });
                }
            }
        }
        Some(fold)
    }

    /// Builds the scripted node; every referenced script node must be a
    /// detached root.
    fn build(self, ast: &mut Ast, base: NodeId) -> NodeId {
        let superscript = self.superscript.map(|superscript| match superscript {
            Superscript::Primes(count) => ast.new_node(Node::Prime { count }),
            Superscript::Explicit(node) => node,
            // The parser's tree for `f'^2`; a prime that is not alone in a
            // superscript serializes as `\prime`.
            Superscript::Merged(count, node) => {
                let prime = ast.new_node(Node::Prime { count });
                ast.implicit_math_group(vec![prime, node])
            }
        });
        ast.new_node(Node::Scripted {
            base,
            subscript: self.subscript,
            superscript,
        })
    }
}

/// The base of `node` and the script events of its own scripts.
fn script_parts(ast: &Ast, node: NodeId) -> (NodeId, Vec<ScriptEvent>) {
    match *ast.node(node) {
        Node::Scripted {
            base,
            subscript,
            superscript,
        } => {
            let mut events: Vec<_> = subscript.map(ScriptEvent::Sub).into_iter().collect();
            events.extend(superscript_events(ast, superscript));
            (base, events)
        }
        _ => (node, Vec::new()),
    }
}

/// Detaches the base and scripts of a scripted detached root, as read by
/// [`script_parts`], and removes the nodes the events do not reference.
fn release_script_parts(ast: &mut Ast, node: NodeId) {
    let Node::Scripted { superscript, .. } = *ast.node(node) else {
        return;
    };
    ast.replace_node(node, empty_math_group());
    ast.remove_detached(node);
    release_superscript(ast, superscript);
}

/// A prime run is a `Prime` node, or a parser-merged superscript group whose
/// leading prime commands or nodes precede one operand.
fn superscript_parts(ast: &Ast, superscript: NodeId) -> (usize, Option<NodeId>) {
    let prime_count = |node: NodeId| match ast.node(node) {
        Node::Prime { count } => Some(*count),
        Node::Command { name, args, .. }
            if name == base::chars::PRIME.name && args.is_empty() =>
        {
            Some(1)
        }
        _ => None,
    };
    match ast.node(superscript) {
        Node::Prime { count } => (*count, None),
        Node::Group {
            kind: GroupKind::Implicit,
            mode: ContentMode::Math,
            children,
        } if children.len() >= 2 => {
            let (operand, primes) = children.split_last().expect("group has children");
            match primes.iter().map(|&node| prime_count(node)).sum::<Option<usize>>() {
                Some(count) => (count, Some(*operand)),
                None => (0, Some(superscript)),
            }
        }
        _ => (0, Some(superscript)),
    }
}

fn superscript_events(ast: &Ast, superscript: Option<NodeId>) -> Vec<ScriptEvent> {
    let Some(superscript) = superscript else {
        return Vec::new();
    };
    let (count, operand) = superscript_parts(ast, superscript);
    (count > 0)
        .then_some(ScriptEvent::Primes(count))
        .into_iter()
        .chain(operand.map(ScriptEvent::Sup))
        .collect()
}

/// Removes a detached superscript that [`superscript_events`] read as a prime
/// run, keeping only its operand as a detached root.
fn release_superscript(ast: &mut Ast, superscript: Option<NodeId>) {
    let Some(superscript) = superscript else {
        return;
    };
    let (count, operand) = superscript_parts(ast, superscript);
    if count == 0 {
        return;
    }
    let children: Vec<NodeId> = ast.children(superscript).to_vec();
    ast.replace_node(superscript, empty_math_group());
    for child in children {
        if Some(child) != operand {
            ast.remove_detached(child);
        }
    }
    ast.remove_detached(superscript);
}

fn empty_math_group() -> Node {
    Node::Group {
        children: Vec::new(),
        kind: GroupKind::Implicit,
        mode: ContentMode::Math,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rewrite::transform_examples;

    // START: Generated examples; DO NOT modify
    transform_examples! {
        rule: ENSUREMATH_EXPAND,
        level: Authoring,
        examples: [
        {
            label: math_splice,
            packages: ["base"],
            input: r"a\ensuremath{x+1}b",
            expected: r"ax+1b",
        },
        {
            label: brace_less_argument,
            packages: ["base", "boldsymbol"],
            input: r"\ensuremath\boldsymbol{B} \succ 0",
            expected: r"\boldsymbol{B} \succ 0",
        },
        {
            label: scripted_base,
            packages: ["base"],
            input: r"\ensuremath{ab}^2",
            expected: r"ab^2",
        },
        {
            label: script_slot,
            packages: ["base"],
            input: r"2^\ensuremath{\mathcal{P}}",
            expected: r"2^{\mathcal{P}}",
        },
        {
            label: script_slot_multi,
            packages: ["base"],
            input: r"x^\ensuremath{ab}",
            expected: r"x^{a}b",
        },
        {
            label: text_mode,
            packages: ["base"],
            input: r"\text{a \ensuremath{x^2} b}",
            expected: r"\text{a $x^2$ b}",
        },
        {
            label: prime_script_clash,
            packages: ["base"],
            input: r"\ensuremath{f'}^2",
            expected: r"f'^2",
        },
        {
            label: empty_script_content,
            packages: ["base"],
            input: r"x^\ensuremath{}y",
            expected: r"x^y",
        },
        {
            label: scripted_script_head,
            packages: ["base"],
            input: r"x^\ensuremath{a_1 b}",
            expected: r"x^a_1 b",
        },
        {
            label: script_order,
            packages: ["base"],
            input: r"x^\ensuremath{ab}_c",
            expected: r"x_c^{a}b",
        },
        ]
    }
    // END: Generated examples

    transform_examples! {
        rule: ENSUREMATH_EXPAND,
        level: Authoring,
        examples: [
        {
            label: empty_sequence_content_is_dropped,
            packages: ["base"],
            input: r"a\ensuremath{}b",
            expected: r"ab",
        },
        {
            label: user_braces_stay_grouped,
            packages: ["base"],
            input: r"\ensuremath{{ab}c}",
            expected: r"{ab}c",
        },
        {
            label: sole_user_brace_group_stays_grouped,
            packages: ["base"],
            input: r"a\ensuremath{{x}}b",
            expected: r"a{x}b",
        },
        {
            label: nested_occurrences_expand,
            packages: ["base"],
            input: r"\ensuremath{\ensuremath{x}y}",
            expected: r"xy",
        },
        {
            label: matrix_cell_content_is_spliced,
            packages: ["base", "ams"],
            input: r"\begin{matrix}\ensuremath{ab}&c\end{matrix}",
            expected: r"\begin{matrix}ab&c\end{matrix}",
        },
        {
            label: single_node_script_base,
            packages: ["base"],
            input: r"\ensuremath{a}^2",
            expected: r"a^2",
        },
        {
            label: empty_script_base_keeps_an_empty_group,
            packages: ["base"],
            input: r"x\ensuremath{}^2",
            expected: r"x{}^2",
        },
        {
            label: script_base_merges_inner_scripts,
            packages: ["base"],
            input: r"\ensuremath{a_1}^2",
            expected: r"a_1^2",
        },
        {
            label: script_base_clash_keeps_braced_base,
            packages: ["base"],
            input: r"\ensuremath{a^1}^2",
            expected: r"{a^1}^2",
        },
        {
            label: script_base_primes_join,
            packages: ["base"],
            input: r"\ensuremath{f'}'",
            expected: r"f''",
        },
        {
            label: script_base_primes_merge_with_subscript,
            packages: ["base"],
            input: r"\ensuremath{f'}_k^2",
            expected: r"f'_k^2",
        },
        {
            label: script_base_prime_after_superscript_keeps_braced_base,
            packages: ["base"],
            input: r"\ensuremath{f^2}'",
            expected: r"{f^2}'",
        },
        {
            label: script_base_in_single_child_slot,
            packages: ["base"],
            input: r"\frac{1}{\ensuremath{ab}^2}",
            expected: r"\frac{1}{ab^2}",
        },
        {
            label: superscript_after_subscript,
            packages: ["base"],
            input: r"x_c^\ensuremath{ab}",
            expected: r"x_c^{a}b",
        },
        {
            label: subscript_before_superscript_moves_superscript,
            packages: ["base"],
            input: r"x_\ensuremath{ab}^c",
            expected: r"x_{a}b^c",
        },
        {
            label: empty_script_without_next_field_keeps_an_empty_group,
            packages: ["base"],
            input: r"{x^\ensuremath{}}x^\ensuremath{}",
            expected: r"{x^{}}x^{}",
        },
        {
            label: empty_script_reads_the_next_field_and_its_scripts,
            packages: ["base"],
            input: r"x^\ensuremath{}y_2+x_\ensuremath{}\alpha",
            expected: r"x^y_2+x_\alpha",
        },
        {
            label: empty_script_double_script_keeps_an_empty_group,
            packages: ["base"],
            input: r"x^\ensuremath{}y^2",
            expected: r"x^{}y^2",
        },
        {
            label: empty_subscript_before_superscript_keeps_an_empty_group,
            packages: ["base"],
            input: r"x_\ensuremath{}^c y",
            expected: r"x_{}^c y",
        },
        {
            label: empty_script_does_not_read_a_cell_separator,
            packages: ["base", "ams"],
            input: r"\begin{matrix}x^\ensuremath{}&y\end{matrix}",
            expected: r"\begin{matrix}x^{}&y\end{matrix}",
        },
        {
            label: scripted_script_head_in_subscript,
            packages: ["base"],
            input: r"x_\ensuremath{a'b}",
            expected: r"x_a'b",
        },
        {
            label: scripted_script_head_double_script_keeps_it_whole,
            packages: ["base"],
            input: r"x_c^\ensuremath{a_1 b}+x^\ensuremath{a^2}",
            expected: r"x_c^{a_1}b+x^{a^2}",
        },
        {
            label: braced_script_splices_inside_braces,
            packages: ["base"],
            input: r"x^{\ensuremath{ab}}",
            expected: r"x^{ab}",
        },
        {
            label: brace_less_macro_argument_keeps_its_slot,
            packages: ["base"],
            input: r"\hat\ensuremath{ab}+\frac\ensuremath{a}{b}",
            expected: r"\hat{ab}+\frac{a}{b}",
        },
        {
            label: text_argument_becomes_inline_math,
            packages: ["base"],
            input: r"\text{\ensuremath{x}}",
            expected: r"\text{$x$}",
        },
        {
            label: empty_text_content_is_dropped,
            packages: ["base"],
            input: r"\text{a\ensuremath{}b}+\text{\ensuremath{}}",
            expected: r"\text{ab}+\text{}",
        },
        {
            label: nested_text_occurrences_expand_once_to_inline_math,
            packages: ["base"],
            input: r"\text{\ensuremath{\ensuremath{x}}}",
            expected: r"\text{$x$}",
        },
        {
            label: inline_math_inside_text_is_math_mode,
            packages: ["base"],
            input: r"\text{a $\ensuremath{x}y$}",
            expected: r"\text{a $xy$}",
        },
        ]
    }

    #[test]
    fn explicit_group_in_the_slot_keeps_its_braces() {
        use crate::rewrite::helpers::mandatory_content_slot;
        use crate::parse::ParseContext;
        use crate::rewrite::{RuleLevel, run_one_rule_for_test};

        let parse_ctx = ParseContext::from_packages(&["base"]);
        let mut ast = Ast::new();
        let a = ast.new_node(Node::Char('a'));
        let b = ast.new_node(Node::Char('b'));
        // A rule may leave an explicit group directly in the slot; it is a
        // user brace pair, not the argument wrapper.
        let group = ast.new_node(Node::Group {
            children: vec![a, b],
            kind: GroupKind::Explicit,
            mode: ContentMode::Math,
        });
        let ensuremath = ast.new_node(Node::Command {
            name: "ensuremath".to_string(),
            args: vec![mandatory_content_slot(group, ContentMode::Math)],
            known: true,
        });
        ast.append_child(ast.root(), ensuremath);

        run_one_rule_for_test(&mut ast, &parse_ctx, &ENSUREMATH_EXPAND, RuleLevel::Authoring)
            .unwrap();
        ast.assert_invariants();
        let [child] = ast.children(ast.root()) else {
            panic!("the expansion should leave one sequence child");
        };
        assert!(matches!(
            ast.node(*child),
            Node::Group {
                kind: GroupKind::Explicit,
                children,
                ..
            } if children.len() == 2
        ));
    }

    #[test]
    fn authoring_profile_output_matches_expansion() {
        use crate::parse::{ParseConfig, ParseContext};
        use crate::{BuildConfig, Profile, TransformContext};

        let parse_ctx = ParseContext::from_packages(&["base"]);
        let engine = TransformContext::from_build_config(
            BuildConfig::profile(Profile::Authoring),
            &parse_ctx,
        )
        .unwrap();
        for (input, expected) in [
            (r"a\ensuremath{x+1}b", "ax+1b"),
            (r"x^\ensuremath{ab}", "x^{a}b"),
            (r"x_\ensuremath{ab}^c", "x_{a}b^{c}"),
            (r"\ensuremath{ab}^2", "ab^{2}"),
            (r"\text{a \ensuremath{x^2} b}", r"\text{a $x^{2}$ b}"),
        ] {
            let mut ast = crate::parse_to_ast_for_test(&parse_ctx, input, &ParseConfig::STRICT);
            engine.run(&mut ast, &parse_ctx).unwrap();
            ast.assert_invariants();
            let output = crate::serialize::serialize(&ast);
            let expected = crate::serialize::serialize(&crate::parse_to_ast_for_test(
                &parse_ctx,
                expected,
                &ParseConfig::STRICT,
            ));
            assert_eq!(output, expected, "{input}");
        }
    }
}
