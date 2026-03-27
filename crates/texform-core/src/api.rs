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

use crate::context::{ContextItem, ParseContext};
use crate::knowledge::{self, KnowledgeBase};
use crate::lexer::Token;
use crate::parser::{self, Spanned, TokenStream, build_token_stream};
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

/// One parse attempt produced by [`parse_with_context_items`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseWithContextItem {
    pub input: String,
    pub output: ParseOutput,
}
pub type ParseWithContextOutput = Vec<ParseWithContextItem>;

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

/// Test one or more context items by injecting them into a fresh parse context,
/// then parsing one or more inputs.
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
) -> Result<ParseContext, knowledge::PackageLoadError> {
    match packages {
        Some(package_names) => {
            knowledge::try_build_kb_from_exact_packages(package_names).map(ParseContext::from_kb)
        }
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
    use crate::context::{CommandItem, ContextItem, DelimiterControlItem, EnvironmentItem};
    use crate::knowledge::{AllowedMode, CommandKind};
    use texform_interface::syntax_node::{ArgumentValue, ContentMode, Delimiter, SyntaxNode};

    fn command_item(
        name: &str,
        kind: CommandKind,
        allowed_mode: AllowedMode,
        spec: &str,
    ) -> ContextItem {
        CommandItem::new(name, kind, allowed_mode, spec).into()
    }

    fn environment_item(
        name: &str,
        allowed_mode: AllowedMode,
        body_mode: ContentMode,
        spec: &str,
    ) -> ContextItem {
        EnvironmentItem::new(name, allowed_mode, body_mode, spec).into()
    }

    fn delimiter_control_item(name: &str) -> ContextItem {
        DelimiterControlItem::new(name).into()
    }

    fn text_command_item() -> ContextItem {
        command_item("text", CommandKind::Prefix, AllowedMode::Math, "m:T")
    }

    fn frac_command_item() -> ContextItem {
        command_item("frac", CommandKind::Prefix, AllowedMode::Math, "m m")
    }

    fn matrix_environment_item() -> ContextItem {
        environment_item("matrix", AllowedMode::Math, ContentMode::Math, "")
    }

    fn parse_with_items(items: &[ContextItem], src: &str, strict: bool) -> ParseOutput {
        let mut ctx = ParseContext::core_only();
        ctx.insert_items(items.iter().cloned())
            .expect("context items should be valid");
        ctx.parse(src, strict)
    }

    #[test]
    fn full_success() {
        let output = parse_latex(r"\\*[1cm]", false);
        assert!(output.result.is_some(), "should produce a result");
        assert!(output.diagnostics.is_empty(), "no diagnostics expected");

        let res = output.result.unwrap();
        assert_eq!(res.span.start, 0);
        assert_eq!(res.span.end, 8);

        let json = serde_json::to_value(&res).unwrap();
        assert!(json.get("node").is_some());
        assert!(json.get("span").is_some());
    }

    #[test]
    fn pure_failure_strict() {
        let output = parse_latex(r"\unknowncmd", true);
        assert!(output.result.is_none(), "strict unknown should fail");
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    }

    #[test]
    fn partial_success_or_failure() {
        let output = parse_with_items(&[frac_command_item()], r"\frac{a}{", false);
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");

        let d = &output.diagnostics[0];
        assert!(!d.message.is_empty());
    }

    #[test]
    fn mode_error_for_math_only_command_in_text() {
        let output = parse_with_items(
            &[text_command_item(), frac_command_item()],
            r"\text{\frac{a}{b}}",
            true,
        );
        assert!(!output.diagnostics.is_empty(), "should have diagnostics");
    }

    #[test]
    fn mode_error_for_math_only_environment_in_text() {
        let output = parse_with_items(
            &[text_command_item(), matrix_environment_item()],
            r"\text\begin{matrix}a\end{matrix}",
            true,
        );
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
    fn parse_with_context_items_command_target() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m",
            )],
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
    fn parse_with_context_items_environment_target() {
        let output = parse_with_context_items(
            &[environment_item(
                "probeenv",
                AllowedMode::Math,
                ContentMode::Math,
                "",
            )],
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
    fn parse_with_context_items_reports_invalid_spec() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "s:T",
            )],
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
    fn parse_with_context_items_defaults_to_core_only_context() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m",
            )],
            &[r"\probe{\text{a}}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            !output[0].output.diagnostics.is_empty(),
            "core-only default should not enable \\text"
        );
    }

    #[test]
    fn parse_with_context_items_supports_explicit_text_command() {
        let output = parse_with_context_items(
            &[
                command_item("probe", CommandKind::Prefix, AllowedMode::Math, "m"),
                text_command_item(),
            ],
            &[r"\probe{\text{a}}"],
            None,
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.result.is_some(),
            "explicit text command should enable \\text"
        );
        assert!(
            output[0].output.diagnostics.is_empty(),
            "no diagnostics expected when text is injected"
        );
    }

    #[test]
    fn parse_with_context_items_supports_explicit_control_delimiter_args() {
        let output = parse_with_context_items(
            &[
                command_item("probe", CommandKind::Prefix, AllowedMode::Math, "m:D"),
                delimiter_control_item("langle"),
                delimiter_control_item("rangle"),
                delimiter_control_item("|"),
            ],
            &[r"\probe\langle", r"\probe\rangle", r"\probe\|"],
            None,
            true,
        );
        assert_eq!(output.len(), 3);

        let expected = [
            Delimiter::Control("langle"),
            Delimiter::Control("rangle"),
            Delimiter::Control("|"),
        ];

        for (item, expected_delimiter) in output.iter().zip(expected) {
            assert!(
                item.output.diagnostics.is_empty(),
                "unexpected diagnostics for {}: {:?}",
                item.input,
                item.output.diagnostics
            );

            let result = item
                .output
                .result
                .as_ref()
                .unwrap_or_else(|| panic!("expected parse result for {}", item.input));

            match &result.node {
                SyntaxNode::Group { children, .. } => match &children[0] {
                    SyntaxNode::Command { args, .. } => match &args[0]
                        .as_ref()
                        .unwrap_or_else(|| panic!("expected argument for {}", item.input))
                        .value
                    {
                        ArgumentValue::Delimiter(value) => {
                            assert_eq!(*value, expected_delimiter);
                        }
                        other => panic!(
                            "expected delimiter argument for {}, got {:?}",
                            item.input, other
                        ),
                    },
                    other => panic!("expected command node for {}, got {:?}", item.input, other),
                },
                other => panic!("expected root group for {}, got {:?}", item.input, other),
            }
        }
    }

    #[test]
    fn parse_with_context_items_supports_runtime_delimiter_controls() {
        let output = parse_with_context_items(
            &[
                delimiter_control_item("langle"),
                delimiter_control_item("rangle"),
            ],
            &[r"\left\langle x\right\rangle"],
            Some(&[]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.diagnostics.is_empty(),
            "unexpected diagnostics: {:?}",
            output[0].output.diagnostics
        );

        let result = output[0]
            .output
            .result
            .as_ref()
            .expect("runtime delimiter controls should parse");

        match &result.node {
            SyntaxNode::Group { children, .. } => match &children[0] {
                SyntaxNode::Group { kind, .. } => match kind {
                    texform_interface::syntax_node::GroupKind::Delimited { left, right } => {
                        assert_eq!(*left, Delimiter::Control("langle"));
                        assert_eq!(*right, Delimiter::Control("rangle"));
                    }
                    other => panic!("expected delimited group, got {:?}", other),
                },
                other => panic!("expected child group, got {:?}", other),
            },
            other => panic!("expected root group, got {:?}", other),
        }
    }

    #[test]
    fn parse_with_context_items_supports_nullable_delimiter_arguments() {
        let output = parse_with_context_items(
            &[command_item(
                "genfracprobe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m:D? m:D? m m m m",
            )],
            &[
                r"\genfracprobe{}{}{0}{1}{a}{b}",
                r"\genfracprobe{(}{)}{0}{1}{a}{b}",
            ],
            None,
            true,
        );
        assert_eq!(output.len(), 2);

        let expected = [
            [Delimiter::None, Delimiter::None],
            [Delimiter::Char('('), Delimiter::Char(')')],
        ];

        for (item, expected_pair) in output.iter().zip(expected) {
            assert!(
                item.output.diagnostics.is_empty(),
                "unexpected diagnostics for {}: {:?}",
                item.input,
                item.output.diagnostics
            );
            let result = item
                .output
                .result
                .as_ref()
                .unwrap_or_else(|| panic!("expected parse result for {}", item.input));

            match &result.node {
                SyntaxNode::Group { children, .. } => match &children[0] {
                    SyntaxNode::Command { args, .. } => {
                        for (slot, expected_delimiter) in args.iter().take(2).zip(expected_pair) {
                            match &slot
                                .as_ref()
                                .unwrap_or_else(|| panic!("expected argument for {}", item.input))
                                .value
                            {
                                ArgumentValue::Delimiter(value) => {
                                    assert_eq!(*value, expected_delimiter);
                                }
                                other => panic!(
                                    "expected delimiter argument for {}, got {:?}",
                                    item.input, other
                                ),
                            }
                        }
                    }
                    other => panic!("expected command node for {}, got {:?}", item.input, other),
                },
                other => panic!("expected root group for {}, got {:?}", item.input, other),
            }
        }
    }

    #[test]
    fn parse_with_context_items_can_use_empty_package_list() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m",
            )],
            &[r"\probe{\text{a}}"],
            Some(&[]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            !output[0].output.diagnostics.is_empty(),
            "\\text should fail when the caller explicitly requests a core-only knowledge base"
        );
    }

    #[test]
    fn parse_with_context_items_can_load_explicit_packages() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m",
            )],
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
    fn parse_with_context_items_reports_unknown_package() {
        let output = parse_with_context_items(
            &[command_item(
                "probe",
                CommandKind::Prefix,
                AllowedMode::Math,
                "m",
            )],
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
    fn parse_with_context_items_multiple_specs() {
        let output = parse_with_context_items(
            &[
                command_item("foo", CommandKind::Prefix, AllowedMode::Math, "m"),
                environment_item("bar", AllowedMode::Math, ContentMode::Math, ""),
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
    fn parse_with_context_items_duplicate_name_rejected() {
        let output = parse_with_context_items(
            &[
                command_item("foo", CommandKind::Prefix, AllowedMode::Math, "m"),
                command_item("foo", CommandKind::Prefix, AllowedMode::Math, "o m"),
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

    #[test]
    fn parse_with_context_items_duplicate_delimiter_rejected() {
        let output = parse_with_context_items(
            &[
                delimiter_control_item("langle"),
                delimiter_control_item("langle"),
            ],
            &[r"\left\langle x\right\rangle"],
            Some(&[]),
            true,
        );
        assert_eq!(output.len(), 1);
        assert!(
            output[0].output.diagnostics[0]
                .message
                .contains("duplicate delimiter control name: langle"),
            "unexpected diagnostic: {}",
            output[0].output.diagnostics[0].message
        );
    }
}
