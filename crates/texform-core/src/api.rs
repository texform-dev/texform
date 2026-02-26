//! High-level API for FFI consumers.
//!
//! Provides [`parse_latex`] as the unified entry point, returning [`ParseOutput`]
//! that preserves partial parse results alongside diagnostics.
//!
//! ## Lossy string conversion
//!
//! The `expected` and `found` fields in [`ParseDiagnostic`] are produced by
//! converting chumsky's internal `RichPattern<Token>` and `MaybeRef<Token>` via
//! their `Display` implementations. This is intentionally a **lossy** conversion:
//! structural information (e.g. the distinction between a token pattern and a
//! label) is flattened to plain strings.

use chumsky::prelude::*;
use serde::Serialize;

use crate::lexer::Token;
use crate::parser;
use crate::parser_utils::{Spanned, TokenStream, build_token_stream};
use texform_interface::syntax_node::SyntaxNode;

/// Byte-offset span.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// Successful (possibly partial) parse result.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "tsify", tsify(into_wasm_abi))]
pub struct ParseResult {
    pub node: SyntaxNode,
    pub span: Span,
}

/// A single diagnostic produced during parsing.
///
/// `expected` and `found` are **lossy** string conversions of chumsky's internal
/// pattern types (see module-level docs).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseDiagnostic {
    pub message: String,
    pub span: Span,
    pub expected: Vec<String>,
    pub found: Option<String>,
}

/// Unified parse output carrying an optional result and zero or more diagnostics.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseOutput {
    pub result: Option<ParseResult>,
    pub diagnostics: Vec<ParseDiagnostic>,
}

/// Parse a LaTeX formula and return a unified output.
///
/// Uses chumsky's output+errors semantics (equivalent to `.into_output_errors()`)
/// so that a partial syntax tree can coexist with diagnostics.
pub fn parse_latex(src: &str, strict: bool) -> ParseOutput {
    let token_stream = build_token_stream(src);
    let (output, errors) = parse_raw(token_stream, strict);

    let result = output.map(|(node, span)| ParseResult {
        node,
        span: Span {
            start: span.start,
            end: span.end,
        },
    });

    let diagnostics = errors.into_iter().map(convert_diagnostic).collect();

    ParseOutput {
        result,
        diagnostics,
    }
}

/// Run the parser with output+errors semantics.
fn parse_raw(
    token_stream: TokenStream<'_>,
    strict: bool,
) -> (Option<Spanned<SyntaxNode>>, Vec<Rich<'static, Token>>) {
    let (output, errors) = parser::math_block_parser(strict)
        .map_with(|node, e| (node, e.span()))
        .then_ignore(end())
        .parse(token_stream)
        .into_output_errors();

    // Convert borrowed errors to owned so they outlive the token stream.
    let errors = errors.into_iter().map(|e| e.into_owned()).collect();
    (output, errors)
}

/// Convert a chumsky `Rich` error into our serializable `ParseDiagnostic`.
fn convert_diagnostic(err: Rich<'static, Token>) -> ParseDiagnostic {
    let span = {
        let s = err.span();
        Span {
            start: s.start,
            end: s.end,
        }
    };

    let reason = err.reason();

    let (message, expected, found) = match reason {
        chumsky::error::RichReason::ExpectedFound {
            expected: exp,
            found: f,
        } => {
            let expected: Vec<String> = exp.iter().map(|p| format!("{p}")).collect();
            let found = f.as_ref().map(|t| format!("{}", &**t));

            let msg = format!("{reason}");
            (msg, expected, found)
        }
        chumsky::error::RichReason::Custom(msg) => (msg.clone(), Vec::new(), None),
    };

    ParseDiagnostic {
        message,
        span,
        expected,
        found,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_success() {
        let output = parse_latex(r"\frac{a}{b}", false);
        assert!(output.result.is_some(), "should produce a result");
        assert!(output.diagnostics.is_empty(), "no diagnostics expected");

        let res = output.result.unwrap();
        assert_eq!(res.span.start, 0);
        assert_eq!(res.span.end, 11);

        // Verify serializable
        let json = serde_json::to_value(&res).unwrap();
        assert!(json.get("node").is_some());
        assert!(json.get("span").is_some());
    }

    #[test]
    fn pure_failure_strict() {
        // Unknown command in strict mode -> error, no output
        let output = parse_latex(r"\unknowncmd", true);
        assert!(output.result.is_none(), "strict unknown should fail");
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    }

    #[test]
    fn partial_success_or_failure() {
        // Unclosed brace: parser may recover partially or fail entirely
        let output = parse_latex(r"\frac{a}{", false);
        // Either way, diagnostics should be present
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");

        // Verify diagnostic fields are populated
        let d = &output.diagnostics[0];
        assert!(!d.message.is_empty());
    }

    #[test]
    fn mode_error_for_math_only_command_in_text() {
        let output = parse_latex(r"\text{\frac{a}{b}}", true);
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    }

    #[test]
    fn mode_error_for_math_only_environment_in_text() {
        let output = parse_latex(r"\text\begin{matrix}a\end{matrix}", true);
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    }

    #[test]
    fn diagnostics_serialize() {
        let output = parse_latex(r"\unknowncmd", true);
        let json = serde_json::to_value(&output).unwrap();
        let diags = json.get("diagnostics").unwrap().as_array().unwrap();
        assert!(!diags.is_empty());
        let d = &diags[0];
        assert!(d.get("message").is_some());
        assert!(d.get("span").is_some());
        assert!(d.get("expected").is_some());
    }
}
