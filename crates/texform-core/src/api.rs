//! High-level convenience API for one-shot parsing and batch probing.
//!
//! This module sits on top of [`ParseContext`] and provides two main entry points:
//!
//! - [`parse_latex`] — one-shot parse using the default all-packages context.
//! - [`parse_with_context_items`] — batch parse with caller-supplied context
//!   items (commands, environments, delimiter controls) injected into a fresh
//!   context.
//!
//! Both functions return [`ParseOutput`], which preserves partial parse results
//! alongside diagnostics so that FFI consumers (Python, WASM) always get
//! structured feedback.

use serde::Serialize;

use crate::context::{ContextItem, ParseContext, ParseDiagnostic, ParseOutput, Span};

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
/// Set `strict` to `true` to reject unknown commands as errors; when
/// `false`, unknown commands are preserved as [`SyntaxNode::UnknownCommand`].
///
/// [`SyntaxNode::UnknownCommand`]: texform_interface::syntax_node::SyntaxNode::UnknownCommand
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
    let mut ctx = match build_probe_context(packages) {
        Ok(ctx) => ctx,
        Err(error) => {
            return invalid_inputs_output(inputs, format!("package loading failed: {}", error));
        }
    };

    if let Some(message) = validate_context_items(items) {
        return invalid_inputs_output(inputs, message);
    }

    for item in items {
        let insert_result = ctx.insert_item(item.clone());

        if let Err(error) = insert_result {
            return invalid_inputs_output(
                inputs,
                format!("spec validation failed for {}: {}", item.name(), error),
            );
        }
    }

    inputs
        .iter()
        .map(|input| ParseWithContextItem {
            input: (*input).to_string(),
            output: ctx.parse(input, strict),
        })
        .collect()
}

fn build_probe_context(
    packages: Option<&[&str]>,
) -> Result<ParseContext, crate::context::PackageLoadError> {
    match packages {
        Some(package_names) => ParseContext::try_from_packages(package_names),
        None => Ok(ParseContext::core_only()),
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
        }],
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
