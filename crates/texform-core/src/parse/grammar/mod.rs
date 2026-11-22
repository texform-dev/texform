//! Hand-written recursive descent grammar productions for LaTeX math and text
//! modes.

mod arguments;
mod command;
mod environment;
mod group;
mod math;
pub(crate) mod scalar;
mod text;

use texform_interface::syntax_node::{ContentMode, SyntaxNode};

use crate::lexer::Token;
use crate::parse::diagnostics;
use crate::parse::parser::{PResult, RecoveryHint, TokenParser};
use crate::parse::tree::{self, ParsedNode};

/// Return the opposite [`ContentMode`].
#[inline]
pub(super) fn other_mode(mode: ContentMode) -> ContentMode {
    match mode {
        ContentMode::Math => ContentMode::Text,
        ContentMode::Text => ContentMode::Math,
    }
}

impl<'tok, 'src, 'ctx> TokenParser<'tok, 'src, 'ctx> {
    /// Parse a top-level math or text formula and promote it to
    /// [`SyntaxNode::Root`].
    ///
    /// Fails if unexpected trailing tokens remain after the content loop
    /// finishes (e.g. unmatched `}`, `\right`, `\end`, or `$` in math mode).
    pub(crate) fn parse_root(&mut self, mode: ContentMode) -> PResult<ParsedNode> {
        let start_pos = self.cursor.pos();
        let children = match mode {
            ContentMode::Math => self.parse_math_content()?,
            ContentMode::Text => self.parse_text_content()?,
        };

        if !self.cursor.is_at_end() {
            let found = self.cursor.peek();
            let span = self.cursor.peek_span();
            if mode == ContentMode::Math && found == Some(Token::MathShift) {
                return self.fail_err(
                    diagnostics::unexpected_math_shift(self.src, span),
                    RecoveryHint::Skip,
                );
            }
            return Err(self.emit_expected(span, &["end of input"], found));
        }

        let span = self.cursor.span_of_range(start_pos..self.cursor.pos());
        let (nodes, span_kids) = tree::decompose_children(children);
        Ok(ParsedNode::with_kids(
            SyntaxNode::Root {
                mode,
                children: nodes,
            },
            span,
            span_kids,
        ))
    }
}

#[cfg(test)]
mod tests {
    use texform_interface::syntax_node::{ContentMode, SyntaxNode};

    use crate::lexer::lex_source;
    use crate::parse::config::ParseConfig;
    use crate::parse::context::ParseContext;
    use crate::parse::cursor::Cursor;
    use crate::parse::diagnostics::finish_diagnostics;
    use crate::parse::parser::TokenParser;
    use crate::parse::result::{ParseDiagnostic, ParseDiagnosticKind};
    use crate::parse::tree::SpanTree;

    fn parse_root_raw(
        ctx: &ParseContext,
        src: &str,
        config: &ParseConfig,
        mode: ContentMode,
    ) -> (Option<(SyntaxNode, SpanTree)>, Vec<ParseDiagnostic>) {
        let stream = match lex_source(src) {
            Ok(stream) => stream,
            Err(diag) => return (None, vec![diag]),
        };
        let cursor = Cursor::from_stream(&stream, src.len());
        let mut parser = TokenParser::new(cursor, src, ctx, config);
        let output = parser.parse_root(mode).ok().map(|node| node.finish_root());
        let diags = finish_diagnostics(parser.into_diagnostics());
        (output, diags)
    }

    #[test]
    fn error_diagnostics_and_recovery_match_contracts() {
        let ctx = ParseContext::shared();

        // Ambiguous infix.
        let (out, diags) = parse_root_raw(
            ctx,
            r"a \over b + 1 \over c",
            &ParseConfig::LENIENT,
            ContentMode::Math,
        );
        assert!(out.is_none());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::AmbiguousInfix);
        assert_eq!(diags[0].message, r"Ambiguous use of \over");

        // Unexpected math shift with currency hint.
        let (out, diags) =
            parse_root_raw(ctx, r"x + $100", &ParseConfig::STRICT, ContentMode::Math);
        assert!(out.is_none());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::UnexpectedMathShift);
        assert_eq!(
            diags[0].message,
            "Unexpected $ inside a math formula; it looks like a currency marker"
        );

        // Text script error.
        let (out, diags) = parse_root_raw(ctx, r"a^b", &ParseConfig::STRICT, ContentMode::Text);
        assert!(out.is_none());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::TextScriptError);

        // Missing script content in-place recovery under LENIENT.
        let (out, diags) =
            parse_root_raw(ctx, r"{x^} + y", &ParseConfig::LENIENT, ContentMode::Math);
        assert!(out.is_some());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::MissingScriptOperand);
        assert_eq!(diags[0].message, "Missing superscript content");
        assert_eq!(diags[0].found.as_deref(), Some("}"));

        // Unknown command head recovery leaves `{x}` intact.
        let reject_recover = ParseConfig {
            reject_unknown: true,
            abort_on_error: false,
            ..Default::default()
        };
        let (out, diags) =
            parse_root_raw(ctx, r"\mycmd{x} + y", &reject_recover, ContentMode::Math);
        let (root, _) = out.expect("should recover unknown command head");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, ParseDiagnosticKind::UnknownCommand);
        assert!(matches!(root, SyntaxNode::Root { ref children, .. } if children.len() == 4));

        // Max group depth exceeded produces an in-place Error even under abort_on_error=true.
        let depth_cfg = ParseConfig {
            max_group_depth: 1,
            abort_on_error: true,
            ..ParseConfig::STRICT
        };
        let (out, diags) = parse_root_raw(ctx, "{{x}}", &depth_cfg, ContentMode::Math);
        assert!(out.is_some());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::MaxGroupDepthExceeded);

        // Unclosed inline math points at opening `$`.
        let (out, diags) =
            parse_root_raw(ctx, r"$a$ b $c", &ParseConfig::STRICT, ContentMode::Text);
        assert!(out.is_none());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::UnclosedInlineMath);
        assert_eq!(
            diags[0].message,
            "Unclosed inline math: missing closing '$'"
        );
        assert_eq!(diags[0].expected, vec!["'$'"]);
        assert_eq!(diags[0].found, None);
        assert_eq!(diags[0].span, crate::parse::Span::new(6, 7));

        // Unclosed brace inside environment upgrades to UnclosedEnvironment at EOF.
        for cfg in [ParseConfig::STRICT, ParseConfig::LENIENT] {
            let (_, diags) = parse_root_raw(ctx, r"\begin{matrix} {a", &cfg, ContentMode::Math);
            assert_eq!(diags.len(), 1, "{cfg:?}: {diags:?}");
            assert_eq!(diags[0].kind, ParseDiagnosticKind::UnclosedEnvironment);
            assert_eq!(
                diags[0].message,
                r"Environment matrix missing closing \end{matrix}"
            );
            assert_eq!(diags[0].expected, vec![r"\end{matrix}"]);
            assert_eq!(diags[0].found, None);
        }

        // Nested env mismatch leaves outer `\end{matrix}` for the outer env to close.
        let (out, diags) = parse_root_raw(
            ctx,
            r"\begin{matrix} \begin{align} x \end{matrix}",
            &ParseConfig::LENIENT,
            ContentMode::Math,
        );
        assert!(out.is_some(), "outer matrix should close cleanly");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].kind, ParseDiagnosticKind::EnvironmentNameMismatch);
        assert_eq!(
            diags[0].message,
            r"Environment name mismatch: expected \end{align}, found \end{matrix}"
        );

        // Environment mismatch followed by trailing `}` keeps both diagnostics.
        let (out, diags) = parse_root_raw(
            ctx,
            r"\begin{matrix} x \end{align}}",
            &ParseConfig::LENIENT,
            ContentMode::Math,
        );
        assert!(out.is_none());
        assert_eq!(diags.len(), 2, "{diags:?}");
        assert_eq!(diags[0].kind, ParseDiagnosticKind::EnvironmentNameMismatch);
        assert_eq!(diags[1].kind, ParseDiagnosticKind::UnexpectedToken);
        assert_eq!(diags[1].message, "found '}' expected end of input");

        // Invalid `\left` delimiter propagates with `left-delimited group` context.
        let (out, diags) = parse_root_raw(
            ctx,
            r"\left\foo x \right)",
            &ParseConfig::LENIENT,
            ContentMode::Math,
        );
        assert!(out.is_none());
        assert_eq!(diags[0].kind, ParseDiagnosticKind::LeftRightDelimiter);
        assert_eq!(diags[0].message, r"invalid \left delimiter");
        assert!(
            diags[0]
                .contexts
                .iter()
                .any(|c| c.label == "left-delimited group")
        );

        // Environment mode error always reports the full `\begin{matrix}` header
        // span regardless of `abort_on_error`.
        let src = r"\begin{matrix}a\end{matrix}";
        for cfg in [ParseConfig::STRICT, ParseConfig::LENIENT] {
            let (_, diags) = parse_root_raw(ctx, src, &cfg, ContentMode::Text);
            assert_eq!(diags[0].kind, ParseDiagnosticKind::EnvironmentModeError);
            assert_eq!(
                &src[diags[0].span.start..diags[0].span.end],
                r"\begin{matrix}"
            );
        }
    }

    #[test]
    fn duplicate_scripts_and_strict_abort_invariants() {
        let ctx = ParseContext::shared();
        let abort_permissive = ParseConfig {
            reject_unknown: false,
            abort_on_error: true,
            ..Default::default()
        };

        // Duplicate scripts emit `DuplicateScript` with the exact offending
        // script span.
        for (src, expected_msg, expected_slice) in [
            (r"x^2^3", "Double exponent: use braces to clarify", "^3"),
            (
                r"x^{2}^{3}",
                "Double exponent: use braces to clarify",
                "^{3}",
            ),
            (r"x_1_2", "Double subscripts: use braces to clarify", "_2"),
            (
                r"f'_a'",
                "Prime causes double exponent: use braces to clarify",
                "'",
            ),
            (
                r"\frac { a ^ { 2 } - 2 b ^ { 2 } ^ { 3 } } { 3 } + 1",
                "Double exponent: use braces to clarify",
                "^ { 3 }",
            ),
        ] {
            for cfg in [
                &ParseConfig::STRICT,
                &abort_permissive,
                &ParseConfig::LENIENT,
            ] {
                let res = crate::parse::parse_with_context_mode(ctx, src, cfg, ContentMode::Math);
                assert!(!res.diagnostics.is_empty(), "{src:?}");
                assert_eq!(
                    res.diagnostics[0].kind,
                    ParseDiagnosticKind::DuplicateScript,
                    "{src:?}: {:?}",
                    res.diagnostics
                );
                assert_eq!(res.diagnostics[0].message, expected_msg, "{src:?}");
                let span = res.diagnostics[0].span;
                assert_eq!(&src[span.start..span.end], expected_slice, "{src:?}");
                if cfg.abort_on_error {
                    assert!(
                        res.document().is_none(),
                        "abort_on_error=true must not return a document for {src:?}"
                    );
                }
            }
        }

        // Under `abort_on_error = true` (even when `reject_unknown = false`),
        // recovery `Error` nodes must not be emitted (except for `max_group_depth`).
        for src in [
            r"{ \ensuremath { \boldsymbol } q }",
            r"\frac { a ^ { 2 } - 2 b ^ { 2 } ^ { 3 } } { 3 } + 1",
            r"\mbox{\rm 1 \kern-2.27622ptI}",
        ] {
            let res = crate::parse::parse_with_context_mode(
                ctx,
                src,
                &abort_permissive,
                ContentMode::Math,
            );
            assert!(
                res.document().is_none(),
                "must not produce a recovery Error tree when abort_on_error=true: {src:?} -> {:?}",
                res.diagnostics
            );
            assert!(
                !res.diagnostics.is_empty(),
                "expected diagnostic for {src:?}"
            );
        }
    }

    #[test]
    fn rd_recursion_depth_guard_prevents_stack_overflow() {
        let ctx = ParseContext::shared();

        let nested_braced_frac = format!("{}x{}", r"\frac{1}{".repeat(1000), "}".repeat(1000));
        let nested_shorthand_frac = format!("{}x", r"\frac 1 ".repeat(1000));
        let nested_left_right = format!("{}x{}", r"\left(".repeat(1000), r"\right)".repeat(1000));

        for src in [
            nested_braced_frac.as_str(),
            nested_shorthand_frac.as_str(),
            nested_left_right.as_str(),
        ] {
            for config in [ParseConfig::STRICT, ParseConfig::LENIENT] {
                let res =
                    crate::parse::parse_with_context_mode(ctx, src, &config, ContentMode::Math);
                assert!(
                    res.document().is_none(),
                    "recursion overflow must abort without a partial tree (abort_on_error={})",
                    config.abort_on_error
                );
                assert!(
                    res.diagnostics
                        .iter()
                        .any(|d| d.kind == ParseDiagnosticKind::MaxRecursionDepthExceeded),
                    "expected MaxRecursionDepthExceeded diagnostic, got: {:?}",
                    res.diagnostics
                );
            }
        }
    }
}
