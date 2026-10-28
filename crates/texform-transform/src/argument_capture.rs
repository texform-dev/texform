//! Boundary hazards and the boundary separation step.
//!
//! A boundary hazard is an adjacency that serialization and reparsing would
//! change. A parsed tree never has one, but deleting, replacing, or inserting
//! nodes can create three kinds:
//!
//! - Command capture: a known command reads a following sibling as an omitted
//!   trailing argument, as in `\sin{}[2]` → `\sin[2]` when the `physics`
//!   `\sin` takes `o`.
//! - Environment capture: an environment reads the first token of its body as
//!   an omitted trailing optional argument, as in
//!   `\begin{aligned}{}[a]\end{aligned}` → `\begin{aligned}[a]\end{aligned}`.
//! - Text-mode space swallowing: in text mode, a control word drops the space
//!   that starts a following text node, as in `\ldots{} and` → `\ldots and`.
//!
//! The engine removes them once after a transform that changed the tree, with
//! [`separate_boundaries`]. FlattenGroups uses the same predicates so that it
//! never recreates a hazard by removing a separator.

use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, GroupKind, Node, NodeId, Slot,
};
use crate::knowledge::{ArgForm, Catalog, DelimiterToken};
use texform_knowledge::specs::ParsedArgSpec;

/// Whether the nearest node of `before`, walked backwards from a boundary, is
/// a known command that would read the first token of `after` as an omitted
/// trailing argument. Whitespace text between them counts as a leading space.
///
/// `catalog` resolves the active records of `mode`, the mode of the sequence
/// holding both sides.
pub(crate) fn boundary_captures<'a, 'r>(
    ast: &Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog,
    mode: ContentMode,
    before: impl Iterator<Item = &'a NodeId>,
    after: impl Iterator<Item = &'a NodeId>,
) -> bool {
    let mut after_space = false;
    let mut before = before.copied().skip_while(|&node| {
        let space = is_whitespace_text(ast, node);
        after_space |= space;
        space
    });
    // A command carrying scripts has already stopped reading arguments.
    let Some((name, args)) = before.next().and_then(|node| match ast.node(node) {
        Node::Command {
            name,
            args,
            known: true,
        }
        | Node::Declarative { name, args } => Some((name, args)),
        _ => None,
    }) else {
        return false;
    };
    let Some(record) = catalog(mode).lookup_command(name) else {
        return false;
    };
    // In math mode the serializer always writes a space between a control
    // sequence and a following `[` or `*`, as in `\\ *`.
    let serialized_space = mode == ContentMode::Math;
    trailing_slots_capture(
        ast,
        &record.argspec,
        args,
        after,
        after_space,
        serialized_space,
    )
}

/// Whether the known environment `env` would read the first token of its body
/// as an omitted trailing argument. `body` iterates the body's children.
pub(crate) fn environment_captures<'a, 'r>(
    ast: &Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog,
    env: NodeId,
    body: impl Iterator<Item = &'a NodeId>,
) -> bool {
    sequence_mode_of(ast, env)
        .is_some_and(|mode| environment_captures_in(ast, catalog, mode, env, body))
}

/// [`environment_captures`] for an environment in a sequence of `mode`, which
/// may be detached.
pub(crate) fn environment_captures_in<'a, 'r>(
    ast: &Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog,
    mode: ContentMode,
    env: NodeId,
    body: impl Iterator<Item = &'a NodeId>,
) -> bool {
    let Node::Environment {
        name,
        args,
        known: true,
        ..
    } = ast.node(env)
    else {
        return false;
    };
    let Some(record) = catalog(mode).lookup_env(name) else {
        return false;
    };
    // The body follows `\begin{name}`'s closing brace, not a control
    // sequence, so the serializer adds no space before it.
    trailing_slots_capture(ast, &record.argspec, args, body, false, false)
}

/// Whether, in text mode, the last node of `before` is a control word that
/// ends its serialization and the first node of `after` is text starting with
/// whitespace. The lexer drops spaces after a control word, so reparsing
/// would lose that space.
pub(crate) fn swallows_leading_space<'a>(
    ast: &Ast,
    mode: ContentMode,
    mut before: impl Iterator<Item = &'a NodeId>,
    mut after: impl Iterator<Item = &'a NodeId>,
) -> bool {
    mode == ContentMode::Text
        && before
            .next()
            .is_some_and(|&node| ends_with_control_word(ast, node))
        && after.next().is_some_and(
            |&node| matches!(ast.node(node), Node::Text(text) if text.starts_with(char::is_whitespace)),
        )
}

/// Whether a sequence boundary is a command capture or a text-mode space
/// swallow.
pub(crate) fn sequence_boundary_hazard<'a, 'r, B, A>(
    ast: &Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog,
    mode: ContentMode,
    before: B,
    after: A,
) -> bool
where
    B: Iterator<Item = &'a NodeId> + Clone,
    A: Iterator<Item = &'a NodeId> + Clone,
{
    boundary_captures(ast, catalog, mode, before.clone(), after.clone())
        || swallows_leading_space(ast, mode, before, after)
}

/// Whether replacing `node` with `replacement`, or removing it when
/// `replacement` is `None`, would leave a boundary hazard at its position.
///
/// `replacement` must be a detached node. A script base counts as the first
/// token of its scripted parent; removing it leaves an empty base group.
pub(crate) fn splice_creates_hazard<'r>(
    ast: &mut Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
    node: NodeId,
    replacement: Option<NodeId>,
) -> bool {
    if ast.slot(node) != Some(Slot::ScriptBase) {
        return splice_hazard(ast, catalog, node, replacement, true);
    }
    let Some(scripted) = ast.parent_id(node) else {
        return false;
    };
    // The scripts keep the new base from reading anything after it.
    match replacement {
        Some(base) => splice_hazard(ast, catalog, scripted, Some(base), false),
        None => {
            let probe = empty_group(ast, ContentMode::Math);
            let hazard = splice_hazard(ast, catalog, scripted, Some(probe), false);
            ast.remove_detached(probe);
            hazard
        }
    }
}

/// Whether the node at `anchor`'s sequence position, replaced by `first`,
/// meets a hazard on its left, or on its right when `check_trailing` is set.
fn splice_hazard<'r>(
    ast: &Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
    anchor: NodeId,
    first: Option<NodeId>,
    check_trailing: bool,
) -> bool {
    let Some(Slot::GroupChild(index)) = ast.slot(anchor) else {
        return false;
    };
    let Some(parent) = ast.parent_id(anchor) else {
        return false;
    };
    let Some(mode) = sequence_mode(ast, parent) else {
        return false;
    };
    let siblings = ast.children(parent);
    let (before, after) = (&siblings[..index], &siblings[index + 1..]);
    let middle = first.as_slice();
    let leading = sequence_boundary_hazard(
        ast,
        catalog,
        mode,
        before.iter().rev(),
        middle.iter().chain(after),
    ) || (index == 0
        && ast
            .parent(parent)
            .is_some_and(|link| link.slot == Slot::EnvBody)
        && environment_captures(
            ast,
            catalog,
            ast.parent_id(parent).expect("body has a parent"),
            middle.iter().chain(after),
        ));
    let trailing = check_trailing
        && first.is_some_and(|first| {
            sequence_boundary_hazard(ast, catalog, mode, std::iter::once(&first), after.iter())
        });
    leading || trailing
}

/// Removes every boundary hazard from the tree and returns the separators it
/// inserted: empty groups, or groups wrapping the capturing commands.
///
/// An environment capture gets an empty group at the start of the body. A
/// command capture or a swallowed space gets an empty group after the command,
/// unless the command would read that group too; then the command is wrapped
/// in a group, as in `{\grad}{}^2`. When the wrapped group is itself read by
/// a preceding command, the group grows left over that command, as in
/// `{\curl\div}[a]`, and stops before a declarative command so that its scope
/// is not cut. A hazard left at such a stop remains in the tree.
pub(crate) fn separate_boundaries<'r>(
    ast: &mut Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
) -> Vec<NodeId> {
    let mut separated = Vec::new();
    for env in ast.find_all(ast.root(), |node| matches!(node, Node::Environment { .. })) {
        let Node::Environment { body, .. } = ast.node(env) else {
            unreachable!("find_all returned an environment");
        };
        let body = *body;
        if environment_captures(ast, catalog, env, ast.children(body).iter()) {
            let mode = sequence_mode(ast, body).unwrap_or(ContentMode::Math);
            let separator = empty_group(ast, mode);
            ast.insert_child(body, 0, separator);
            separated.push(separator);
        }
    }
    for sequence in ast.find_all(ast.root(), |node| {
        matches!(node, Node::Root { .. } | Node::Group { .. })
    }) {
        separate_sequence(ast, catalog, sequence, &mut separated);
    }
    separated
}

fn separate_sequence<'r>(
    ast: &mut Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
    sequence: NodeId,
    separated: &mut Vec<NodeId>,
) {
    let Some(mode) = sequence_mode(ast, sequence) else {
        return;
    };
    let mut index = 0;
    while index < ast.children(sequence).len() {
        let children = ast.children(sequence);
        let candidate = matches!(
            ast.node(children[index]),
            Node::Command { .. } | Node::Declarative { .. }
        );
        let (before, after) = children.split_at(index + 1);
        if !candidate
            || !sequence_boundary_hazard(ast, catalog, mode, before.iter().rev(), after.iter())
        {
            index += 1;
            continue;
        }
        match separate_after(ast, catalog, sequence, mode, index) {
            Some(separator) => {
                separated.push(separator);
                let Some(Slot::GroupChild(at)) = ast.slot(separator) else {
                    unreachable!("a separator is a sequence child");
                };
                index = at + 1;
            }
            None => index += 1,
        }
    }
}

/// Separates the command at `index` from what follows it. Returns the
/// separator, after which scanning resumes, or `None` when the hazard is left
/// in place.
fn separate_after<'r>(
    ast: &mut Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
    sequence: NodeId,
    mode: ContentMode,
    index: usize,
) -> Option<NodeId> {
    let probe = empty_group(ast, mode);
    let reads_group = |ast: &Ast, end: usize| {
        boundary_captures(
            ast,
            catalog,
            mode,
            ast.children(sequence)[..end].iter().rev(),
            std::iter::once(&probe),
        )
    };
    if !reads_group(ast, index + 1) {
        ast.insert_child(sequence, index + 1, probe);
        return Some(probe);
    }
    // Wrapping a declarative command would cut its scope at the group.
    if matches!(
        ast.node(ast.children(sequence)[index]),
        Node::Declarative { .. }
    ) {
        ast.remove_detached(probe);
        return None;
    }
    let mut start = index;
    loop {
        let children = ast.children(sequence);
        let Some(previous) = (0..start)
            .rev()
            .find(|&at| !is_whitespace_text(ast, children[at]))
        else {
            break;
        };
        if matches!(ast.node(children[previous]), Node::Declarative { .. })
            || !reads_group(ast, start)
        {
            break;
        }
        start = previous;
    }
    ast.remove_detached(probe);
    let wrapped = ast.detach_children_range(sequence, start..index + 1);
    let group = ast.new_node(Node::Group {
        children: wrapped,
        kind: GroupKind::Explicit,
        mode,
    });
    ast.insert_child(sequence, start, group);
    Some(group)
}

/// Separates `node` from a preceding command that would read it as an
/// omitted trailing argument. Returns whether it changed the tree.
///
/// Use this after a rewrite places `node` where a separator used to be. An
/// empty group goes before `node`, as in `\\{}[x]`, unless the command would
/// read that group too; then the command itself is wrapped, as in
/// `{\exp}(x)`. FlattenGroups keeps either group.
pub(crate) fn separate_from_preceding_command<'r>(
    ast: &mut Ast,
    catalog: impl Fn(ContentMode) -> &'r Catalog + Copy,
    node: NodeId,
) -> bool {
    let Some(Slot::GroupChild(index)) = ast.slot(node) else {
        return false;
    };
    let Some(parent) = ast.parent_id(node) else {
        return false;
    };
    let Some(mode) = sequence_mode(ast, parent) else {
        return false;
    };
    let siblings = ast.children(parent);
    let (before, after) = siblings.split_at(index);
    if !boundary_captures(ast, catalog, mode, before.iter().rev(), after.iter()) {
        return false;
    }

    let Some(command) = before
        .iter()
        .rev()
        .copied()
        .find(|&sibling| !is_whitespace_text(ast, sibling))
    else {
        return false;
    };
    let Some(Slot::GroupChild(command_index)) = ast.slot(command) else {
        return false;
    };

    let separator = empty_group(ast, mode);
    let siblings = ast.children(parent);
    if !boundary_captures(
        ast,
        catalog,
        mode,
        siblings[..index].iter().rev(),
        std::iter::once(&separator),
    ) {
        ast.insert_child(parent, index, separator);
        return true;
    }
    ast.remove_detached(separator);
    let command = ast.detach_children_range(parent, command_index..command_index + 1);
    let group = ast.new_node(Node::Group {
        children: command,
        kind: GroupKind::Explicit,
        mode,
    });
    ast.insert_child(parent, command_index, group);
    true
}

/// Whether arguments after the last supplied slot in `args` could take the
/// first non-space token of `after`.
///
/// With `serialized_space`, a following `[` or `*` counts as preceded by a
/// space, which the serializer writes there in math mode, so a slot that
/// rejects a leading space cannot take it.
fn trailing_slots_capture<'a>(
    ast: &Ast,
    spec: &ParsedArgSpec,
    args: &[ArgumentSlot],
    after: impl Iterator<Item = &'a NodeId>,
    mut after_space: bool,
    serialized_space: bool,
) -> bool {
    let Some(next) = after.copied().find(|&node| {
        let space = is_whitespace_text(ast, node);
        after_space |= space;
        !space
    }) else {
        return false;
    };
    if let Node::Text(text) = ast.node(next) {
        after_space |= text.starts_with(char::is_whitespace);
    }
    after_space |=
        serialized_space && (starts_with_char(ast, next, '[') || starts_with_char(ast, next, '*'));
    let spec = spec.args;
    if spec.len() != args.len() {
        return false;
    }
    // Arguments are read in order, so only slots after the last supplied one
    // can still take a following token.
    let start = args
        .iter()
        .rposition(is_supplied)
        .map_or(0, |last| last + 1);
    for slot in &spec[start..] {
        if slot.required {
            return false;
        }
        if after_space && slot.no_leading_space {
            continue;
        }
        if form_opens_with(ast, &slot.form, next) {
            return true;
        }
    }
    false
}

/// An absent star is stored as `false` rather than as an empty slot.
fn is_supplied(slot: &ArgumentSlot) -> bool {
    slot.as_ref().is_some_and(|argument| {
        argument.kind != ArgumentKind::Star || argument.value != ArgumentValue::Boolean(false)
    })
}

/// Whether `node` serializes as a control word with nothing after it.
fn ends_with_control_word(ast: &Ast, node: NodeId) -> bool {
    match ast.node(node) {
        Node::Command { name, args, .. } | Node::Declarative { name, args } => {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphabetic())
                && !args.iter().any(is_supplied)
        }
        _ => false,
    }
}

fn empty_group(ast: &mut Ast, mode: ContentMode) -> NodeId {
    ast.new_node(Node::Group {
        children: Vec::new(),
        kind: GroupKind::Explicit,
        mode,
    })
}

fn sequence_mode(ast: &Ast, sequence: NodeId) -> Option<ContentMode> {
    match ast.node(sequence) {
        Node::Root { mode, .. } | Node::Group { mode, .. } => Some(*mode),
        _ => None,
    }
}

/// The mode of the sequence holding `node`.
fn sequence_mode_of(ast: &Ast, node: NodeId) -> Option<ContentMode> {
    let parent = ast.parent_id(node)?;
    match ast.node(parent) {
        Node::Scripted { .. } => sequence_mode_of(ast, parent),
        _ => sequence_mode(ast, parent),
    }
}

fn is_whitespace_text(ast: &Ast, node: NodeId) -> bool {
    matches!(ast.node(node), Node::Text(text) if text.trim().is_empty())
}

/// Whether `node` begins with a token that opens an argument of `form`.
fn form_opens_with(ast: &Ast, form: &ArgForm, node: NodeId) -> bool {
    match form {
        ArgForm::Standard => starts_with_char(ast, node, '['),
        ArgForm::Star => starts_with_char(ast, node, '*'),
        ArgForm::Group => starts_with_brace_group(ast, node),
        ArgForm::Delimited { open, .. } => starts_with_token(ast, node, open),
        ArgForm::Paired { pairs } => pairs
            .iter()
            .any(|(open, _)| starts_with_token(ast, node, open)),
        ArgForm::Until { .. } => false,
    }
}

fn starts_with_token(ast: &Ast, node: NodeId, token: &DelimiterToken) -> bool {
    match token {
        // A brace delimiter is read as a brace group, which the AST keeps as
        // a group node rather than as characters.
        DelimiterToken::Char('{') => starts_with_brace_group(ast, node),
        DelimiterToken::Char(c) => starts_with_char(ast, node, *c),
        DelimiterToken::ControlSeq(control) => matches!(
            ast.node(first_token(ast, node)),
            Node::Command { name, .. } if name == control.as_ref()
        ),
    }
}

fn starts_with_brace_group(ast: &Ast, node: NodeId) -> bool {
    matches!(
        ast.node(first_token(ast, node)),
        Node::Group {
            kind: GroupKind::Explicit | GroupKind::Implicit,
            ..
        }
    )
}

fn starts_with_char(ast: &Ast, node: NodeId, c: char) -> bool {
    match ast.node(first_token(ast, node)) {
        Node::Char(value) => *value == c,
        Node::Text(text) => text.trim_start().starts_with(c),
        _ => false,
    }
}

/// The node that serializes first: scripts follow their base.
fn first_token(ast: &Ast, node: NodeId) -> NodeId {
    match ast.node(node) {
        Node::Scripted { base, .. } => first_token(ast, *base),
        _ => node,
    }
}

#[cfg(test)]
mod tests {
    use texform_knowledge::builtin::base;
    use texform_knowledge::specs::CommandKind;

    use super::*;
    use crate::parse::{ParseConfig, ParseContext};
    use crate::rewrite::rule_context::RuleContext;
    use crate::rewrite::{
        PackageName, Plan, RewriteRule, RuleConsumes, RuleEffect, RuleFidelity, RuleKey, RuleLevel,
        RuleMeta, RuleProduces, RuleTarget,
    };
    use crate::round_trip::{self, Check, Outcome};
    use crate::serialize::serialize;
    use crate::{
        BuildConfig, FinalizeAstConfig, FlattenGroupsConfig, LowerAttributesConfig, Profile,
        RewriteConfig, TransformConfig, TransformContext,
    };

    const PHYSICS: &[&str] = round_trip::PHYSICS_PACKAGES;
    const DEFAULT: &[&str] = round_trip::DEFAULT_PACKAGES;

    fn parse(parse_ctx: &ParseContext, source: &str) -> Ast {
        crate::parse_to_ast_for_test(parse_ctx, source, &ParseConfig::default())
    }

    fn engine(parse_ctx: &ParseContext, profile: Profile) -> TransformContext {
        TransformContext::from_build_config(BuildConfig::profile(profile), parse_ctx).unwrap()
    }

    fn assert_round_trip(parse_ctx: &ParseContext, engine: &TransformContext, ast: &Ast) {
        let failures = round_trip::check_transformed(parse_ctx, engine, ast);
        assert!(
            failures.is_empty(),
            "{}: {}",
            serialize(ast),
            failures
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
    }

    /// Each kind of boundary hazard is separated in the documented form, and
    /// the result passes every transformed-tree check.
    #[test]
    fn boundary_hazards_are_separated() {
        for (packages, profile, source, expected) in [
            (
                PHYSICS,
                Profile::Corpus,
                r"\ln\badbreak[2]x",
                r"\ln { } [ 2 ] x",
            ),
            (
                PHYSICS,
                Profile::Corpus,
                r"\cos\allowbreak[n]x",
                r"\cos { } [ n ] x",
            ),
            (
                PHYSICS,
                Profile::Corpus,
                r"\sin\mathstrut[2]x",
                r"\sin { } [ 2 ] x",
            ),
            (
                PHYSICS,
                Profile::Equiv,
                r"\sin\quad[2]x",
                r"\sin { } [ 2 ] x",
            ),
            (
                PHYSICS,
                Profile::Equiv,
                r"\log\enspace[2](n)",
                r"\log { } [ 2 ] ( n )",
            ),
            (
                PHYSICS,
                Profile::Corpus,
                r"\sin\nobreak(x)",
                r"\sin { } ( x )",
            ),
            (PHYSICS, Profile::Equiv, r"\sin\quad(x)", r"\sin { } ( x )"),
            (PHYSICS, Profile::Equiv, r"\dd\quad[x", r"{ \dd } [ x"),
            (
                PHYSICS,
                Profile::Equiv,
                r"\qty\enspace|x|",
                r"{ \qty } | x |",
            ),
            (
                PHYSICS,
                Profile::Corpus,
                r"\mqty\strut(a)",
                r"{ \mqty } ( a )",
            ),
            (PHYSICS, Profile::Authoring, r"\smqty\ast", r"{ \smqty } *"),
            (
                PHYSICS,
                Profile::Equiv,
                r"\curl\div\quad[a]",
                r"{ \curl \div } [ a ]",
            ),
            (
                PHYSICS,
                Profile::Equiv,
                r"\grad\Large'^2",
                r"{ \grad } { } ^ { \prime 2 }",
            ),
            (
                DEFAULT,
                Profile::Equiv,
                r"\begin{aligned}\big[a]\end{aligned}",
                r"\begin {aligned} { } [ a ] \end {aligned}",
            ),
            (
                DEFAULT,
                Profile::Equiv,
                r"\begin{aligned}\quad[a]\end{aligned}",
                r"\begin {aligned} { } [ a ] \end {aligned}",
            ),
            (
                DEFAULT,
                Profile::Corpus,
                r"\begin{aligned}\mathstrut[a]\end{aligned}",
                r"\begin {aligned} { } [ a ] \end {aligned}",
            ),
            (
                DEFAULT,
                Profile::Equiv,
                r"\begin{aligned}\big[a\end{aligned}",
                r"\begin {aligned} { } [ a \end {aligned}",
            ),
            (
                DEFAULT,
                Profile::Equiv,
                r"\text{\ldots\quad{} and}",
                r"\text {\ldots{} and}",
            ),
            // FlattenGroups keeps the braces that already separate them.
            (
                DEFAULT,
                Profile::Corpus,
                r"\text{{\ldots} and}",
                r"\text {{\ldots} and}",
            ),
            (
                DEFAULT,
                Profile::Corpus,
                r"\mbox{\S{} b}",
                r"\mbox {\S{} b}",
            ),
        ] {
            let parse_ctx = ParseContext::from_packages(packages);
            let engine = engine(&parse_ctx, profile);
            let mut ast = parse(&parse_ctx, source);
            let separated = engine
                .run_counting_separations(&mut ast, &parse_ctx, engine.default_config())
                .unwrap();
            assert_eq!(serialize(&ast), expected, "{source}");
            assert!(
                separated > 0 || source.contains("{\\ldots}") || source.contains("\\S{}"),
                "{source}"
            );
            assert_round_trip(&parse_ctx, &engine, &ast);
        }
    }

    /// Every separator is needed: on the stratified adversarial inputs,
    /// removing any one of them, or replacing a wrapping group with an empty
    /// group after the wrapped commands, makes some output style reparse to a
    /// different tree.
    #[test]
    fn every_separator_is_needed() {
        let mut checked = 0;
        let mut unneeded = Vec::new();
        for (lane, packages) in round_trip::PACKAGE_SETS {
            let parse_ctx = ParseContext::from_packages(packages);
            let engines: Vec<_> = round_trip::PROFILES
                .into_iter()
                .map(|profile| (profile, engine(&parse_ctx, profile)))
                .collect();
            for input in round_trip::adversarial::stratified(&parse_ctx) {
                let Some(original) = round_trip::parse(&parse_ctx, &input.source) else {
                    continue;
                };
                for (profile, engine) in &engines {
                    let mut ast = original.clone();
                    if crate::engine::execute_without_separation(engine, &mut ast, &parse_ctx)
                        .is_err()
                    {
                        continue;
                    }
                    for separator in separate_boundaries(&mut ast, |mode| parse_ctx.catalog(mode)) {
                        checked += 1;
                        let (removed, wrapped) = without_separator(&ast, separator);
                        let mut variants = vec![("removed", removed.clone())];
                        if let Some(end) = wrapped {
                            let mut instead = removed;
                            let parent = ast.parent_id(separator).unwrap();
                            let mode = sequence_mode(&ast, parent).unwrap();
                            let group = empty_group(&mut instead, mode);
                            instead.insert_child(parent, end, group);
                            variants.push(("replaced by an empty group", instead));
                        }
                        for (change, variant) in variants {
                            if reparses_to_itself(&parse_ctx, &variant) {
                                unneeded.push(format!(
                                    "[{lane} {}] {}: separator {change} in {}",
                                    round_trip::profile_name(*profile),
                                    input.source,
                                    serialize(&ast)
                                ));
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 0);
        assert!(
            unneeded.is_empty(),
            "{} unneeded separator(s):\n{}",
            unneeded.len(),
            unneeded.join("\n")
        );
    }

    /// `ast` without `separator`, and for a wrapping group the sequence index
    /// just after the commands it wrapped.
    fn without_separator(ast: &Ast, separator: NodeId) -> (Ast, Option<usize>) {
        let mut ast = ast.clone();
        let parent = ast.parent_id(separator).unwrap();
        let Some(Slot::GroupChild(index)) = ast.slot(separator) else {
            unreachable!("a separator is a sequence child");
        };
        let len = ast.children(separator).len();
        let wrapped = ast.detach_children_range(separator, 0..len);
        ast.detach(separator);
        ast.remove_detached(separator);
        for (offset, child) in wrapped.into_iter().enumerate() {
            ast.insert_child(parent, index + offset, child);
        }
        (ast, (len > 0).then_some(index + len))
    }

    fn reparses_to_itself(parse_ctx: &ParseContext, ast: &Ast) -> bool {
        round_trip::STYLES.into_iter().all(|style| {
            round_trip::parse(parse_ctx, &round_trip::print(ast, style))
                .is_some_and(|reparsed| round_trip::tree_difference(ast, &reparsed).is_none())
        })
    }

    #[test]
    fn wrapped_command_grows_left_over_capturing_commands_only() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        let equiv = engine(&parse_ctx, Profile::Equiv);
        let mut ast = parse(&parse_ctx, r"\curl\grad\div\quad[a]");
        equiv.run(&mut ast, &parse_ctx).unwrap();
        assert_eq!(serialize(&ast), r"{ \curl \grad \div } [ a ]");
        assert_eq!(ast.children(ast.root()).len(), 4);
        assert_round_trip(&parse_ctx, &equiv, &ast);

        // Moving `\rm` into the group would cut its scope.
        let lowering_off = TransformConfig {
            lower_attributes: LowerAttributesConfig::DISABLED,
            ..*equiv.default_config()
        };
        let mut ast = parse(&parse_ctx, r"\rm\curl\div\quad[a]");
        equiv.run_with(&mut ast, &parse_ctx, &lowering_off).unwrap();
        assert_eq!(serialize(&ast), r"\rm { \curl \div } [ a ]");
        let reparsed = round_trip::parse(&parse_ctx, &serialize(&ast)).unwrap();
        assert_eq!(round_trip::tree_difference(&ast, &reparsed), None);
    }

    /// The separation strategy relies on these knowledge-base facts; a new
    /// record that breaks one fails here instead of being mishandled.
    #[test]
    fn separators_cannot_be_read_by_environments_or_declaratives() {
        let packages = texform_knowledge::builtin::all_package_names();
        let parse_ctx = ParseContext::from_packages(&packages);
        for mode in [ContentMode::Math, ContentMode::Text] {
            // An empty group at the start of a body never fills a trailing
            // optional environment argument.
            for record in parse_ctx.environments(mode) {
                let trailing = record
                    .argspec
                    .args
                    .iter()
                    .rev()
                    .take_while(|arg| !arg.required);
                for arg in trailing {
                    assert!(
                        !matches!(arg.form, ArgForm::Group)
                            && !matches!(&arg.form, ArgForm::Delimited { open, .. } if *open == DelimiterToken::Char('{')),
                        "environment {} takes a trailing optional brace group",
                        record.name
                    );
                }
            }
            // A group after a declarative is never its argument, so a wrapped
            // group can stop before one.
            for record in parse_ctx.commands(mode) {
                if record.kind != CommandKind::Declarative {
                    continue;
                }
                for arg in record
                    .argspec
                    .args
                    .iter()
                    .rev()
                    .take_while(|arg| !arg.required)
                {
                    assert!(
                        !matches!(
                            arg.form,
                            ArgForm::Group | ArgForm::Delimited { .. } | ArgForm::Paired { .. }
                        ),
                        "declarative {} takes a trailing optional group or delimited argument",
                        record.name
                    );
                }
            }
        }
    }

    /// Rules may see a hazard that a later deletion creates. Either order of
    /// renaming and deleting gives a different but valid form.
    #[test]
    fn rule_order_changes_form_but_not_the_round_trip() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        let corpus = engine(&parse_ctx, Profile::Corpus);
        let single = |name: &'static str| {
            let package = if name == "sine-to-sin" {
                PackageName::Physics
            } else {
                PackageName::Base
            };
            TransformContext::from_build_config(
                BuildConfig::profile(Profile::Corpus)
                    .only_rule_for_tests(RuleKey { package, name }),
                &parse_ctx,
            )
            .unwrap()
        };
        let rename = single("sine-to-sin");
        let drop = single("nobreak-drop");
        let mut outputs = Vec::new();
        for order in [[&rename, &drop], [&drop, &rename]] {
            let mut ast = parse(&parse_ctx, r"\sine\nobreak(x)");
            for step in order {
                step.run(&mut ast, &parse_ctx).unwrap();
            }
            let reparsed = round_trip::parse(&parse_ctx, &serialize(&ast)).unwrap();
            assert_eq!(round_trip::tree_difference(&ast, &reparsed), None);
            let mut once = ast.clone();
            corpus.run(&mut once, &parse_ctx).unwrap();
            assert_round_trip(&parse_ctx, &corpus, &once);
            outputs.push(serialize(&ast));
        }
        assert_eq!(outputs[0], r"\sin { } ( x )");
        assert_ne!(outputs[0], outputs[1]);
        let mut full = parse(&parse_ctx, r"\sine\nobreak(x)");
        corpus.run(&mut full, &parse_ctx).unwrap();
        assert_round_trip(&parse_ctx, &corpus, &full);
    }

    #[test]
    fn separation_runs_only_after_a_change() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        let corpus = engine(&parse_ctx, Profile::Corpus);
        let disabled = TransformConfig {
            lower_attributes: LowerAttributesConfig::DISABLED,
            rewrite: RewriteConfig {
                enabled: false,
                ..RewriteConfig::DEFAULT
            },
            finalize_ast: FinalizeAstConfig::DISABLED,
            flatten_groups: FlattenGroupsConfig::DISABLED,
        };
        let mut ast = parse(&parse_ctx, r"\ln\badbreak[2]x");
        let before = ast.to_syntax_root();
        assert_eq!(
            corpus.run_counting_separations(&mut ast, &parse_ctx, &disabled),
            Ok(0)
        );
        assert_eq!(ast.to_syntax_root(), before);

        // An unchanged parsed tree has no hazard to separate.
        let mut ast = parse(&parse_ctx, r"\ln[2]x");
        let before = ast.to_syntax_root();
        assert_eq!(
            corpus.run_counting_separations(&mut ast, &parse_ctx, corpus.default_config()),
            Ok(0)
        );
        assert_eq!(ast.to_syntax_root(), before);
    }

    #[test]
    fn separation_is_idempotent() {
        for (packages, profile, source) in [
            (PHYSICS, Profile::Corpus, r"\ln\badbreak[2]x"),
            (PHYSICS, Profile::Equiv, r"\curl\grad\div\big[a]"),
            (PHYSICS, Profile::Equiv, r"\grad\Large'^2"),
            (PHYSICS, Profile::Authoring, r"\smqty\ast"),
            (
                DEFAULT,
                Profile::Equiv,
                r"\begin{aligned}\big[a]\end{aligned}",
            ),
            (DEFAULT, Profile::Equiv, r"\text{\ldots\quad{} and}"),
        ] {
            let parse_ctx = ParseContext::from_packages(packages);
            let mut ast = parse(&parse_ctx, source);
            engine(&parse_ctx, profile)
                .run(&mut ast, &parse_ctx)
                .unwrap();
            let once = ast.to_syntax_root();
            assert_eq!(
                separate_boundaries(&mut ast, |mode| parse_ctx.catalog(mode)).len(),
                0,
                "{source}"
            );
            assert_eq!(ast.to_syntax_root(), once, "{source}");
        }
    }

    #[test]
    fn flatten_groups_keeps_separators() {
        let parse_ctx = ParseContext::from_packages(DEFAULT);
        let physics = ParseContext::from_packages(PHYSICS);
        let context = engine(&parse_ctx, Profile::Corpus);
        for flatten_groups in [
            FlattenGroupsConfig::STRUCTURAL_ONLY,
            FlattenGroupsConfig::STRICT,
        ] {
            let config = TransformConfig {
                lower_attributes: LowerAttributesConfig::DISABLED,
                rewrite: RewriteConfig {
                    enabled: false,
                    ..RewriteConfig::DEFAULT
                },
                finalize_ast: FinalizeAstConfig::DISABLED,
                flatten_groups,
            };
            for (parse_ctx, source) in [
                (&parse_ctx, r"\text{\ldots{} and}"),
                (&parse_ctx, r"\text{{\ldots} and}"),
                (&parse_ctx, r"\begin{aligned}{}[a]\end{aligned}"),
                (&physics, r"\ln{}[2]x"),
                (&physics, r"{\curl\div}[a]"),
                (&physics, r"{\grad}{}^{\prime 2}"),
            ] {
                let mut ast = parse(parse_ctx, source);
                let before = serialize(&ast);
                context.run_with(&mut ast, parse_ctx, &config).unwrap();
                assert_eq!(serialize(&ast), before, "{source}");
            }
        }
    }

    /// Render rules use the rule-side predicate to leave hazardous contexts
    /// unchanged; it agrees with what separation would do.
    #[test]
    fn rule_side_predicate_matches_separation() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        for (source, hazard) in [
            (r"\ln\badbreak[2]x", true),
            (r"\ln\badbreak x", false),
            (r"x\badbreak[2]", false),
            (r"\curl\div\badbreak[a]", true),
        ] {
            let mut ast = parse(&parse_ctx, source);
            let target = ast
                .find(
                    ast.root(),
                    |node| matches!(node, Node::Command { name, .. } if name == "badbreak"),
                )
                .unwrap();
            let mut cx =
                RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
            assert_eq!(
                cx.removal_creates_boundary_hazard(target),
                hazard,
                "{source}"
            );
            ast.remove_node(target);
            let separated = separate_boundaries(&mut ast, |mode| parse_ctx.catalog(mode));
            assert_eq!(!separated.is_empty(), hazard, "{source}");
        }

        let mut ast = parse(&parse_ctx, r"\smqty\ast");
        let target = ast.children(ast.root())[1];
        let replacement = ast.new_node(Node::Char('*'));
        let mut cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        assert!(cx.replacement_creates_boundary_hazard(target, replacement));
        let replacement = ast.new_node(Node::Char('+'));
        let mut cx = RuleContext::new(&mut ast, parse_ctx.math_catalog(), parse_ctx.text_catalog());
        assert!(!cx.replacement_creates_boundary_hazard(target, replacement));
    }

    /// A Render rule that drops `\badbreak`, optionally guarded by the
    /// rule-side predicate.
    struct DropBadbreak {
        guarded: bool,
    }
    static GUARDED: DropBadbreak = DropBadbreak { guarded: true };
    static UNGUARDED: DropBadbreak = DropBadbreak { guarded: false };

    impl RewriteRule for DropBadbreak {
        fn meta(&self) -> &'static RuleMeta {
            static META: RuleMeta = RuleMeta {
                key: RuleKey {
                    package: PackageName::Base,
                    name: "drop-badbreak-test",
                },
                enabled_by_packages: &[PackageName::Base],
                level: RuleLevel::Authoring,
                summary: "Drop badbreak unless that exposes a boundary hazard.",
                fidelity: RuleFidelity::Render,
                triggers: &[RuleTarget::Command(&base::cmd::BADBREAK)],
                consumes: RuleConsumes {
                    eliminates: &[],
                    touches: &[RuleTarget::Command(&base::cmd::BADBREAK)],
                },
                produces: RuleProduces { targets: &[] },
            };
            &META
        }

        fn apply(
            &self,
            cx: &mut RuleContext<'_>,
            node: NodeId,
        ) -> Result<RuleEffect, crate::rewrite::RuleError> {
            if cx.match_command(node, &base::cmd::BADBREAK).is_none()
                || (self.guarded && cx.removal_creates_boundary_hazard(node))
            {
                return Ok(RuleEffect::Skipped);
            }
            cx.ast.remove_node(node);
            Ok(RuleEffect::Applied)
        }
    }

    #[test]
    fn attribution_detects_render_rules_that_rely_on_separation() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        let rewrite_only = TransformConfig {
            lower_attributes: LowerAttributesConfig::DISABLED,
            rewrite: RewriteConfig::DEFAULT,
            finalize_ast: FinalizeAstConfig::DISABLED,
            flatten_groups: FlattenGroupsConfig::DISABLED,
        };
        for (rule, expected) in [
            (&GUARDED, (0, r"\ln \badbreak [ 2 ] x")),
            (&UNGUARDED, (1, r"\ln { } [ 2 ] x")),
        ] {
            let context = TransformContext::from_rewrite_plan_for_tests(
                rewrite_only,
                Plan::from_rules_for_tests(vec![rule]),
            );
            let mut ast = parse(&parse_ctx, r"\ln\badbreak[2]x");
            let separated = context
                .run_counting_separations(&mut ast, &parse_ctx, &rewrite_only)
                .unwrap();
            assert_eq!((separated, serialize(&ast).as_str()), expected);
        }
    }

    /// Without separation, the reparse check catches the capture while the
    /// text-idempotency check passes; a failing transform is reported rather
    /// than skipped.
    #[test]
    fn round_trip_checks_detect_what_text_idempotency_misses() {
        let parse_ctx = ParseContext::from_packages(PHYSICS);
        let corpus = engine(&parse_ctx, Profile::Corpus);
        let mut ast = parse(&parse_ctx, r"\ln\badbreak[2]x");
        crate::engine::execute_without_separation(&corpus, &mut ast, &parse_ctx).unwrap();
        let output = serialize(&ast);
        assert_eq!(output, r"\ln [ 2 ] x");
        let failures = round_trip::check_transformed(&parse_ctx, &corpus, &ast);
        assert!(
            failures
                .iter()
                .any(|failure| failure.check == Check::Reparse)
        );
        let mut again = round_trip::parse(&parse_ctx, &output).unwrap();
        corpus.run(&mut again, &parse_ctx).unwrap();
        assert_eq!(serialize(&again), output);

        let looping = TransformContext::from_rewrite_plan_for_tests(
            TransformConfig {
                lower_attributes: LowerAttributesConfig::ENABLED,
                rewrite: RewriteConfig::DEFAULT,
                finalize_ast: FinalizeAstConfig::ENABLED,
                flatten_groups: FlattenGroupsConfig::STRICT,
            },
            Plan::from_rules_for_tests(vec![&crate::engine::tests::WRAP_SINGLETON]),
        );
        let ast = parse(&parse_ctx, r"\mathbf{x}");
        assert!(matches!(
            round_trip::check_transform(&parse_ctx, &looping, &ast),
            Outcome::TransformFailed(_)
        ));
    }
}
