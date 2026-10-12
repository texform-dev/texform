use crate::ast::{
    ArgumentKind, ArgumentSlot, ArgumentValue, Ast, ContentMode, Delimiter, GroupKind, Node, NodeId,
};
use crate::rewrite::RuleError;
use crate::rewrite::rule_context::RuleScopedContext;

/// How physics.sty lets a named function grab the argument that follows it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgumentBraces {
    /// `\trigbraces` (`o d()`): an optional power, then auto-sized parentheses.
    /// The power is printed literally as `[...]` when no parentheses follow.
    Trig,
    /// `\opbraces`: only the first of `{...}`, `[...]`, or `(...)` becomes an
    /// auto-sized fence, recorded as one paired `d<(,)><[,]><{,}>` slot; any
    /// later bracket or parenthesis group is printed literally.
    Operator,
    /// `\DeclareMathOperator`: following delimiters are ordinary characters.
    Plain,
}

/// An auto-sized argument the parser attached to a physics named function.
pub(super) struct FenceArgument {
    body: NodeId,
    left: Delimiter,
    right: Delimiter,
}

/// Reads the `d()` or paired slot that holds a physics auto-sized argument.
///
/// Returns `None` for an omitted slot. A brace pair prints as `\{...\}`.
pub(super) fn fence_argument(
    scoped: &RuleScopedContext<'_, '_>,
    slot: &ArgumentSlot,
    subject: &str,
) -> Result<Option<FenceArgument>, RuleError> {
    let Some(argument) = slot else {
        return Ok(None);
    };
    let (ArgumentKind::Delimited { open, close } | ArgumentKind::Paired { open, close }) =
        &argument.kind
    else {
        return Err(scoped.invalid_shape(format!(
            "{subject} auto-sized argument should be a delimited slot"
        )));
    };
    let ArgumentValue::MathContent(body) = argument.value else {
        return Err(scoped.invalid_shape(format!(
            "{subject} auto-sized argument should be math content"
        )));
    };
    let fence = |delimiter: &Delimiter| match delimiter {
        Delimiter::Char(brace @ ('{' | '}')) => Delimiter::Control(brace.to_string()),
        other => other.clone(),
    };
    Ok(Some(FenceArgument {
        body,
        left: fence(open),
        right: fence(close),
    }))
}

/// Replaces `command` with `operator` followed by a `\left...\right` fence
/// around the grabbed argument.
///
/// Scripts on the command were written after the closing delimiter, so they
/// move to the fence, matching physics.sty, which closes the fence inside a
/// `\mathclose` group. `operator` must be a detached root.
pub(super) fn replace_with_fenced_operator(
    ast: &mut Ast,
    command: NodeId,
    operator: NodeId,
    argument: FenceArgument,
) {
    let mut children = Vec::new();
    ast.append_cloned_math_content(&mut children, argument.body);
    let fence = ast.new_node(Node::Group {
        children,
        kind: GroupKind::Delimited {
            left: argument.left,
            right: argument.right,
        },
        mode: ContentMode::Math,
    });
    ast.replace_with_math_sequence_preserving_scripts(command, Vec::new(), operator, vec![fence]);
}

/// Raises `operator` to a copy of the grabbed `\trigbraces` power, as
/// physics.sty prints `\sin[n](x)` as `\sin^{n}` followed by the fence.
pub(super) fn with_power(ast: &mut Ast, operator: NodeId, power: Option<NodeId>) -> NodeId {
    match power {
        Some(power) => {
            let power = ast.clone_subtree(power);
            ast.superscript(operator, power)
        }
        None => operator,
    }
}
