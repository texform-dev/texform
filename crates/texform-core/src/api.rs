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

use crate::context::ParseContext;
use crate::knowledge::{self, AllowedMode, CommandKind, KnowledgeBase};
use crate::lexer::Token;
use crate::parser;
use crate::parser_utils::{Spanned, TokenStream, build_token_stream};
use texform_interface::syntax_node::{ContentMode, SyntaxNode};

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

/// One parse attempt produced by [`parse_with_argspecs`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseWithArgspecItem {
    pub input: String,
    pub output: ParseOutput,
}
pub type ParseWithArgspecOutput = Vec<ParseWithArgspecItem>;

#[derive(Debug, Clone, Copy)]
pub enum SpecTarget {
    Command {
        kind: CommandKind,
        allowed_mode: AllowedMode,
    },
    Environment {
        allowed_mode: AllowedMode,
        body_mode: ContentMode,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct TemporaryArgSpec<'a> {
    pub name: &'a str,
    pub target: SpecTarget,
    pub spec: &'a str,
}

/// Parse a LaTeX formula and return a unified output.
///
/// Uses chumsky's output+errors semantics (equivalent to `.into_output_errors()`)
/// so that a partial syntax tree can coexist with diagnostics.
pub fn parse_latex(src: &str, strict: bool) -> ParseOutput {
    parse_latex_with_kb(knowledge::kb(), src, strict)
}

pub(crate) fn parse_latex_with_kb(kb: &KnowledgeBase, src: &str, strict: bool) -> ParseOutput {
    let token_stream = build_token_stream(src);
    let (output, errors) = parse_raw(kb, token_stream, strict);

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

/// Test one or more ArgSpecs by injecting temporary commands/environments,
/// then parsing one or more inputs.
///
/// By default, this loads the embedded `test` package so parse probes can use `\text{...}`
/// when validating text-mode behavior. Pass `packages` to use an explicit package list instead
/// (for example `["dev"]` or `[]` for an empty knowledge base).
///
/// Each entry in `argspecs` is inserted into the context before parsing begins.
/// Duplicate `(target, name)` pairs are rejected with a diagnostic.
///
/// Prefer inputs that only exercise the temporary targets plus plain literal content
/// (letters, digits, simple operators, and grouping). The one allowed helper command is
/// `\text{...}` when you intentionally need to enter text mode. Avoid other commands/environments
/// and avoid argument/value syntax that depends on unrelated records.
pub fn parse_with_argspecs(
    argspecs: &[TemporaryArgSpec<'_>],
    inputs: &[&str],
    packages: Option<&[&str]>,
    strict: bool,
) -> ParseWithArgspecOutput {
    let mut ctx = match packages {
        Some(package_names) => match knowledge::try_build_kb_from_exact_packages(package_names) {
            Ok(kb) => ParseContext::from_kb(kb),
            Err(error) => {
                return invalid_inputs_output(inputs, format!("package loading failed: {}", error));
            }
        },
        None => match knowledge::try_build_kb_from_exact_packages(
            texform_specs::packages::PARSE_WITH_ARGSPEC_DEFAULT_PACKAGES,
        ) {
            Ok(kb) => ParseContext::from_kb(kb),
            Err(error) => {
                return invalid_inputs_output(inputs, format!("package loading failed: {}", error));
            }
        },
    };

    // Check for duplicate (target, name) pairs.
    {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for spec in argspecs {
            let target_tag = match spec.target {
                SpecTarget::Command { .. } => "command",
                SpecTarget::Environment { .. } => "environment",
            };
            if !seen.insert((target_tag, spec.name)) {
                return invalid_inputs_output(
                    inputs,
                    format!("duplicate {} name: {}", target_tag, spec.name),
                );
            }
        }
    }

    // Insert all argspecs.
    for argspec in argspecs {
        let insert_result = match argspec.target {
            SpecTarget::Command { kind, allowed_mode } => {
                ctx.insert_command(argspec.name, kind, allowed_mode, argspec.spec, &[])
            }
            SpecTarget::Environment {
                allowed_mode,
                body_mode,
            } => ctx.insert_env(argspec.name, allowed_mode, argspec.spec, body_mode, &[]),
        };

        if let Err(error) = insert_result {
            return invalid_inputs_output(
                inputs,
                format!("spec validation failed for {}: {}", argspec.name, error),
            );
        }
    }

    inputs
        .iter()
        .map(|input| ParseWithArgspecItem {
            input: (*input).to_string(),
            output: ctx.parse(input, strict),
        })
        .collect()
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

fn invalid_inputs_output(inputs: &[&str], message: String) -> ParseWithArgspecOutput {
    inputs
        .iter()
        .map(|input| ParseWithArgspecItem {
            input: (*input).to_string(),
            output: invalid_input_output(message.clone()),
        })
        .collect()
}

/// Run the parser with output+errors semantics.
fn parse_raw(
    kb: &KnowledgeBase,
    token_stream: TokenStream<'_>,
    strict: bool,
) -> (Option<Spanned<SyntaxNode>>, Vec<Rich<'static, Token>>) {
    let (output, errors) = parser::math_block_parser(kb, strict)
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

    #[test]
    fn parse_with_argspecs_command_target() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "m",
            }],
            &[r"\probe{a}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.result.is_some(),
            "command target should parse"
        );
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected"
        );
    }

    #[test]
    fn parse_with_argspecs_environment_target() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probeenv",
                target: SpecTarget::Environment {
                    allowed_mode: AllowedMode::Math,
                    body_mode: ContentMode::Math,
                },
                spec: "",
            }],
            &[r"\begin{probeenv}a\end{probeenv}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.result.is_some(),
            "environment target should parse"
        );
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected"
        );
    }

    #[test]
    fn parse_with_argspecs_reports_invalid_spec() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "s:T",
            }],
            &[r"\probe", r"\probe*"],
            None,
            true,
        );
        assert_eq!(output.len(), 2);
        assert!(
            output[0].output.diagnostics[0]
                .message
                .contains("spec validation failed"),
            "unexpected diagnostic: {}",
            output[0].output.diagnostics[0].message
        );
    }

    #[test]
    fn parse_with_argspecs_uses_test_package_by_default() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "m",
            }],
            &[r"\probe{\text{a}}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.result.is_some(),
            "default test package should enable \\text"
        );
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected when the default test package is loaded"
        );
    }

    #[test]
    fn parse_with_argspecs_can_use_empty_package_list() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "m",
            }],
            &[r"\probe{\text{a}}"],
            Some(&[]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            !output[0].output.diagnostics.is_empty(),
            "\\text should fail when the caller explicitly requests an empty knowledge base"
        );
    }

    #[test]
    fn parse_with_argspecs_can_load_explicit_packages() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "m",
            }],
            &[r"\probe{\hspace{1em}}"],
            Some(&["dev"]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.result.is_some(),
            "explicit dev package should enable \\hspace"
        );
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected when dev is loaded"
        );
    }

    #[test]
    fn parse_with_argspecs_reports_unknown_package() {
        let output = parse_with_argspecs(
            &[TemporaryArgSpec {
                name: "probe",
                target: SpecTarget::Command {
                    kind: CommandKind::Prefix,
                    allowed_mode: AllowedMode::Math,
                },
                spec: "m",
            }],
            &[r"\probe{a}"],
            Some(&["missing-package"]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.diagnostics[0]
                .message
                .contains("package loading failed"),
            "unexpected diagnostic: {}",
            output[0].output.diagnostics[0].message
        );
    }

    #[test]
    fn parse_with_argspecs_multiple_specs() {
        let output = parse_with_argspecs(
            &[
                TemporaryArgSpec {
                    name: "foo",
                    target: SpecTarget::Command {
                        kind: CommandKind::Prefix,
                        allowed_mode: AllowedMode::Math,
                    },
                    spec: "m",
                },
                TemporaryArgSpec {
                    name: "bar",
                    target: SpecTarget::Environment {
                        allowed_mode: AllowedMode::Math,
                        body_mode: ContentMode::Math,
                    },
                    spec: "",
                },
            ],
            &[r"\foo{\begin{bar}x\end{bar}}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(output[0].output.result.is_some(), "multi-spec should parse");
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected"
        );
    }

    #[test]
    fn parse_with_argspecs_duplicate_name_rejected() {
        let output = parse_with_argspecs(
            &[
                TemporaryArgSpec {
                    name: "foo",
                    target: SpecTarget::Command {
                        kind: CommandKind::Prefix,
                        allowed_mode: AllowedMode::Math,
                    },
                    spec: "m",
                },
                TemporaryArgSpec {
                    name: "foo",
                    target: SpecTarget::Command {
                        kind: CommandKind::Prefix,
                        allowed_mode: AllowedMode::Math,
                    },
                    spec: "o m",
                },
            ],
            &[r"\foo{x}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.diagnostics[0]
                .message
                .contains("duplicate command name: foo"),
            "unexpected diagnostic: {}",
            output[0].output.diagnostics[0].message
        );
    }
}
