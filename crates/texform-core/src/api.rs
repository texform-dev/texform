//! High-level convenience API for one-shot parsing, batch probing, and
//! serialization.
//!
//! This module sits on top of [`ParseContext`] and provides main entry points:
//!
//! - [`parse_latex`] — one-shot parse using the default all-packages context.
//! - [`parse_with_context_items`] — batch parse with caller-supplied context
//!   items (commands, environments, delimiter controls) injected into a fresh
//!   context.
//! - [`serialize_latex`] / [`serialize_latex_with`] — serialize a
//!   [`SyntaxNode`] back to LaTeX text via the canonical serializer.
//!
//! Parse functions return [`ParseOutput`], which preserves partial parse results
//! alongside diagnostics so that FFI consumers (Python, WASM) always get
//! structured feedback.

use serde::Serialize;
use texform_interface::syntax_node::SyntaxNode;

use crate::ast::Ast;
use crate::context::{
    ContextItem, KnowledgeBase, ParseContext, ParseDiagnostic, ParseOutput, Span,
};
use crate::serialize::{self, SerializeOptions};

/// One parse attempt produced by [`parse_with_context_items`].
///
/// Pairs the original input string with the corresponding [`ParseOutput`] so
/// callers can match results back to their inputs in batch operations.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseWithContextItem {
    /// The LaTeX source that was parsed
    pub input: String,
    /// Parse result and diagnostics for this input
    pub output: ParseOutput,
}

/// Batch output from [`parse_with_context_items`]: one entry per input string.
pub type ParseWithContextOutput = Vec<ParseWithContextItem>;

/// Parse a LaTeX formula using the default all-packages context.
///
/// This is the simplest entry point for one-shot parsing. It shares a
/// lazily-initialized [`ParseContext`] across calls so the knowledge base
/// is built only once.
///
/// Set `strict` to `true` to reject unknown command/environment names as
/// errors; when `false`, they are preserved as ordinary command/environment
/// nodes with `known: false`.
pub fn parse_latex(src: &str, strict: bool) -> ParseOutput {
    ParseContext::all_packages_shared().parse(src, strict)
}

/// Inject context items into a fresh parse context and parse multiple inputs.
///
/// Useful for probing: the caller supplies temporary command / environment /
/// delimiter definitions and a batch of LaTeX inputs. Each input is parsed
/// independently against the same augmented context.
///
/// When `packages` is `Some`, only the listed packages are loaded (an empty
/// slice gives a core-only context). When `None`, a core-only context is
/// used as the default base.
///
/// Validation errors (duplicate names, invalid specs, unknown packages) are
/// reported as diagnostics on every input rather than panicking.
pub fn parse_with_context_items(
    items: &[ContextItem],
    inputs: &[&str],
    packages: Option<&[&str]>,
    strict: bool,
) -> ParseWithContextOutput {
    let mut kb = match build_probe_kb(packages) {
        Ok(kb) => kb,
        Err(error) => {
            return invalid_inputs_output(inputs, format!("package loading failed: {}", error));
        }
    };

    if let Some(message) = validate_context_items(items) {
        return invalid_inputs_output(inputs, message);
    }

    for item in items {
        let insert_result = kb.insert_item(item.clone());

        if let Err(error) = insert_result {
            return invalid_inputs_output(
                inputs,
                format!("spec validation failed for {}: {}", item.name(), error),
            );
        }
    }

    let ctx = ParseContext::new(kb);

    inputs
        .iter()
        .map(|input| ParseWithContextItem {
            input: (*input).to_string(),
            output: ctx.parse(input, strict),
        })
        .collect()
}

/// Serialize a [`SyntaxNode`] back to canonical LaTeX text using default options.
pub fn serialize_latex(node: &SyntaxNode) -> String {
    assert_serializable_syntax_node(node);
    let ast = Ast::from_syntax_node(node);
    serialize::serialize(&ast)
}

/// Serialize a [`SyntaxNode`] back to LaTeX text with explicit style options.
pub fn serialize_latex_with(node: &SyntaxNode, options: &SerializeOptions) -> String {
    assert_serializable_syntax_node(node);
    let ast = Ast::from_syntax_node(node);
    serialize::serialize_with(&ast, options)
}

fn build_probe_kb(
    packages: Option<&[&str]>,
) -> Result<KnowledgeBase, crate::context::PackageLoadError> {
    match packages {
        Some(package_names) => KnowledgeBase::try_build_from_packages(package_names),
        None => Ok(KnowledgeBase::core_only()),
    }
}

fn validate_context_items(items: &[ContextItem]) -> Option<String> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    for item in items {
        let target_tag = item.target_tag();
        let name = item.name();
        if !seen.insert((target_tag, name)) {
            return Some(format!("duplicate {} name: {}", target_tag, name));
        }
    }
    None
}

fn invalid_input_output(message: String) -> ParseOutput {
    ParseOutput {
        result: None,
        diagnostics: vec![ParseDiagnostic {
            message,
            span: Span { start: 0, end: 0 },
            expected: Vec::new(),
            found: None,
            contexts: Vec::new(),
        }],
    }
}

fn assert_serializable_syntax_node(node: &SyntaxNode) {
    match node {
        SyntaxNode::Group { children, .. } => {
            for child in children {
                assert_serializable_syntax_node(child);
            }
        }
        SyntaxNode::Command { args, .. } | SyntaxNode::Environment { args, .. } => {
            for arg in args.iter().flatten() {
                assert_serializable_argument_value(&arg.value);
            }
            if let SyntaxNode::Environment { body, .. } = node {
                assert_serializable_syntax_node(body);
            }
        }
        SyntaxNode::Infix {
            args, left, right, ..
        } => {
            for arg in args.iter().flatten() {
                assert_serializable_argument_value(&arg.value);
            }
            assert_serializable_syntax_node(left);
            assert_serializable_syntax_node(right);
        }
        SyntaxNode::Declarative { args, scope, .. } => {
            for arg in args.iter().flatten() {
                assert_serializable_argument_value(&arg.value);
            }
            assert_serializable_syntax_node(scope);
        }
        SyntaxNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            assert_serializable_syntax_node(base);
            if let Some(subscript) = subscript {
                assert_serializable_syntax_node(subscript);
            }
            if let Some(superscript) = superscript {
                assert_serializable_syntax_node(superscript);
            }
        }
        SyntaxNode::Error { .. } => {
            panic!("cannot serialize syntax tree containing Error node")
        }
        SyntaxNode::Text(_) | SyntaxNode::Char(_) | SyntaxNode::ActiveSpace => {}
    }
}

fn assert_serializable_argument_value(value: &texform_interface::syntax_node::ArgumentValue) {
    match value {
        texform_interface::syntax_node::ArgumentValue::MathContent(node)
        | texform_interface::syntax_node::ArgumentValue::TextContent(node) => {
            assert_serializable_syntax_node(node);
        }
        texform_interface::syntax_node::ArgumentValue::Delimiter(_)
        | texform_interface::syntax_node::ArgumentValue::CSName(_)
        | texform_interface::syntax_node::ArgumentValue::Dimension(_)
        | texform_interface::syntax_node::ArgumentValue::Integer(_)
        | texform_interface::syntax_node::ArgumentValue::KeyVal(_)
        | texform_interface::syntax_node::ArgumentValue::Column(_)
        | texform_interface::syntax_node::ArgumentValue::Boolean(_) => {}
    }
}

fn invalid_inputs_output(inputs: &[&str], message: String) -> ParseWithContextOutput {
    inputs
        .iter()
        .map(|input| ParseWithContextItem {
            input: (*input).to_string(),
            output: invalid_input_output(message.clone()),
        })
        .collect()
}
