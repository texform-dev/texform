use crate::ast::{Ast, ContentMode, Delimiter, GroupKind, Node, NodeId, Slot};
use crate::rewrite::helpers::{
    convert_group_to_auto_fence, matching_sibling_delimiter, replace_siblings_with_auto_fence,
    script_base,
};

/// How physics.sty lets a named function grab the argument that follows it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgumentBraces {
    /// `\trigbraces` (`o d()`): an optional power, then auto-sized parentheses.
    /// The power is printed literally as `[...]` when no parentheses follow.
    Trig,
    /// `\opbraces` (`g o d()`): only the first of `{...}`, `[...]`, or `(...)`
    /// becomes an auto-sized fence; any later bracket or parenthesis group
    /// is printed literally.
    Operator,
    /// `\DeclareMathOperator`: following delimiters are ordinary characters.
    Plain,
}

/// The argument physics.sty grabs after a named function.
pub(super) enum FenceArgument {
    /// A braced group (possibly carrying scripts) right after the command.
    Braced { group: NodeId },
    /// Literal delimiter characters at `start..=end` of `parent`.
    Delimited {
        parent: NodeId,
        start: usize,
        end: usize,
        open: char,
        close: char,
    },
}

/// Finds the argument physics.sty would grab after `command`.
///
/// xparse skips spaces and matches nested delimiters of the same kind; brace
/// groups hide their delimiters, which the AST already isolates. A command
/// that carries scripts grabs nothing. An opening parenthesis or bracket that
/// carries scripts is reported as no argument even though xparse would grab
/// it, because the scripts have no place on a fence. An unbalanced opener
/// grabs nothing; LaTeX would report a runaway argument there.
pub(super) fn find_fence_argument(
    ast: &Ast,
    command: NodeId,
    braces: ArgumentBraces,
) -> Option<FenceArgument> {
    if braces == ArgumentBraces::Plain {
        return None;
    }
    let Some(Slot::GroupChild(index)) = ast.slot(command) else {
        return None;
    };
    let parent = ast.parent_id(command)?;
    let start = index + 1;
    let opener = *ast.children(parent).get(start)?;

    let group = script_base(ast, opener);
    if braces == ArgumentBraces::Operator
        && matches!(
            ast.node(group),
            Node::Group {
                kind: GroupKind::Explicit,
                mode: ContentMode::Math,
                ..
            }
        )
    {
        return Some(FenceArgument::Braced { group });
    }

    let (open, close) = match ast.node(opener) {
        Node::Char('(') => ('(', ')'),
        Node::Char('[') if braces == ArgumentBraces::Operator => ('[', ']'),
        _ => return None,
    };
    let end = matching_sibling_delimiter(ast, parent, start, open, close)?;
    Some(FenceArgument::Delimited {
        parent,
        start,
        end,
        open,
        close,
    })
}

/// Rewrites a grabbed argument into a `\left...\right` fence in place.
///
/// Scripts on the closing delimiter or on the braced group stay on the fence,
/// matching physics.sty, which closes the fence inside a `\mathclose` group.
pub(super) fn replace_with_fence(ast: &mut Ast, argument: FenceArgument) {
    match argument {
        FenceArgument::Braced { group } => convert_group_to_auto_fence(
            ast,
            group,
            Delimiter::Control("{".to_string()),
            Delimiter::Control("}".to_string()),
        ),
        FenceArgument::Delimited {
            parent,
            start,
            end,
            open,
            close,
        } => {
            replace_siblings_with_auto_fence(
                ast,
                parent,
                start,
                end,
                Delimiter::Char(open),
                Delimiter::Char(close),
            );
        }
    }
}
