//! Parser-private semantics attached to Chumsky's rich errors.
use super::{ParseDiagnosticKind, grammar::TokenStream};
use crate::lexer::Token;
use chumsky::span::Span;
use chumsky::{
    error::{Error, Rich, RichPattern, RichReason},
    label::LabelError,
    prelude::SimpleSpan,
    util::MaybeRef,
};
use std::{fmt, ops::Deref};
/// Source information captured by a grammar branch, independent of Rich's merge span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectDiagnostic {
    pub(crate) span: SimpleSpan,
    pub(crate) environment: Option<EnvironmentDiagnostic>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EnvironmentDiagnostic {
    pub(crate) name: String,
    pub(crate) expected_name: String,
}
#[derive(Clone, Debug)]
pub(crate) struct ParseFailure<'a> {
    rich: Rich<'a, Token>,
    pub(crate) kind: Option<ParseDiagnosticKind>,
    pub(crate) is_control: bool,
    pub(crate) direct: Option<Box<DirectDiagnostic>>,
}
impl<'a> ParseFailure<'a> {
    fn from_rich(rich: Rich<'a, Token>) -> Self {
        Self {
            rich,
            kind: None,
            is_control: false,
            direct: None,
        }
    }
    pub(crate) fn custom(span: SimpleSpan, message: impl ToString) -> Self {
        Self::from_rich(Rich::custom(span, message))
    }
    pub(crate) fn at_source(mut self, span: SimpleSpan) -> Self {
        self.direct = Some(Box::new(DirectDiagnostic {
            span,
            environment: None,
        }));
        self
    }
    pub(crate) fn at_environment(
        mut self,
        span: SimpleSpan,
        name: String,
        expected_name: String,
    ) -> Self {
        self.direct = Some(Box::new(DirectDiagnostic {
            span,
            environment: Some(EnvironmentDiagnostic {
                name,
                expected_name,
            }),
        }));
        self
    }
    pub(crate) fn control(mut self) -> Self {
        self.is_control = true;
        self
    }
    pub(crate) fn with_context(mut self, label: &'static str, span: SimpleSpan) -> Self {
        <Self as LabelError<'a, TokenStream<'a>, &str>>::in_context(&mut self, label, span);
        self
    }

    /// Tree aggregation distinguishes custom reasons from expected/found errors.
    pub(crate) fn matches_tree_diagnostic(&self, other: &Self) -> bool {
        self.same_source(other)
            && match (self.reason(), other.reason()) {
                (RichReason::Custom(left), RichReason::Custom(right)) => left == right,
                (
                    RichReason::ExpectedFound {
                        expected: left,
                        found: left_found,
                    },
                    RichReason::ExpectedFound {
                        expected: right,
                        found: right_found,
                    },
                ) => {
                    left.iter()
                        .map(ToString::to_string)
                        .eq(right.iter().map(ToString::to_string))
                        && left_found.as_deref().map(ToString::to_string)
                            == right_found.as_deref().map(ToString::to_string)
                }
                _ => false,
            }
            && self.same_contexts(other)
    }

    /// Recovery keeps its existing rendered-reason comparison, even across reason variants.
    pub(crate) fn matches_recovery_diagnostic(&self, other: &Self) -> bool {
        self.same_source(other)
            && self.reason().to_string() == other.reason().to_string()
            && self.same_contexts(other)
    }

    fn same_source(&self, other: &Self) -> bool {
        self.kind == other.kind && self.direct == other.direct && self.span() == other.span()
    }

    fn same_contexts(&self, other: &Self) -> bool {
        self.contexts()
            .map(|(label, span)| (label.to_string(), *span))
            .eq(other
                .contexts()
                .map(|(label, span)| (label.to_string(), *span)))
    }

    pub(crate) fn into_rich(self) -> Rich<'a, Token> {
        self.rich
    }
    pub(crate) fn into_owned<'b>(self) -> ParseFailure<'b> {
        ParseFailure {
            rich: self.rich.into_owned(),
            kind: self.kind,
            is_control: self.is_control,
            direct: self.direct,
        }
    }
    pub(crate) fn shifted(mut self, offset: usize) -> Self {
        let shift = |s: SimpleSpan| SimpleSpan::new((), s.start + offset..s.end + offset);
        let mut rich = match self.rich.reason() {
            RichReason::Custom(message) => Rich::custom(shift(*self.span()), message),
            RichReason::ExpectedFound { expected, found } => <Rich<'a, Token> as LabelError<
                'a,
                TokenStream<'a>,
                RichPattern<'a, Token>,
            >>::expected_found(
                expected.iter().cloned(),
                found.clone(),
                shift(*self.span()),
            ),
        };
        for (label, span) in self.contexts() {
            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, RichPattern<'a, Token>>>::in_context(&mut rich, label.clone(), shift(*span));
        }
        if let Some(direct) = &mut self.direct {
            direct.span = shift(direct.span);
        }
        Self { rich, ..self }
    }
}
impl<'a> Deref for ParseFailure<'a> {
    type Target = Rich<'a, Token>;
    fn deref(&self) -> &Self::Target {
        &self.rich
    }
}
impl fmt::Display for ParseFailure<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.rich, f)
    }
}
impl<'a> Error<'a, TokenStream<'a>> for ParseFailure<'a> {
    #[inline]
    fn merge(self, other: Self) -> Self {
        // Rich prefers the first Custom reason, then the second Custom, then
        // merges ExpectedFound. Its left-hand contexts remain unchanged.
        let use_other = !matches!(self.reason(), RichReason::Custom(_))
            && matches!(other.reason(), RichReason::Custom(_));
        let (kind, is_control, direct) = if use_other {
            (other.kind, other.is_control, other.direct)
        } else {
            (self.kind, self.is_control, self.direct)
        };
        Self {
            rich: <Rich<'a, Token> as Error<'a, TokenStream<'a>>>::merge(self.rich, other.rich),
            kind,
            is_control,
            direct,
        }
    }
}
impl<'a, L: Into<RichPattern<'a, Token>>> LabelError<'a, TokenStream<'a>, L> for ParseFailure<'a> {
    #[inline]
    fn expected_found<E: IntoIterator<Item = L>>(
        expected: E,
        found: Option<MaybeRef<'a, Token>>,
        span: SimpleSpan,
    ) -> Self {
        Self::from_rich(
            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, L>>::expected_found(
                expected, found, span,
            ),
        )
    }
    #[inline]
    fn merge_expected_found<E: IntoIterator<Item = L>>(
        self,
        expected: E,
        found: Option<MaybeRef<'a, Token>>,
        span: SimpleSpan,
    ) -> Self {
        Self {
            rich: <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, L>>::merge_expected_found(
                self.rich, expected, found, span,
            ),
            ..self
        }
    }
    #[inline]
    fn replace_expected_found<E: IntoIterator<Item = L>>(
        self,
        expected: E,
        found: Option<MaybeRef<'a, Token>>,
        span: SimpleSpan,
    ) -> Self {
        Self::from_rich(
            <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, L>>::replace_expected_found(
                self.rich, expected, found, span,
            ),
        )
    }
    #[inline]
    fn label_with(&mut self, label: L) {
        <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, L>>::label_with(&mut self.rich, label);
        self.kind = None;
        self.is_control = false;
        self.direct = None;
    }
    #[inline]
    fn in_context(&mut self, label: L, span: SimpleSpan) {
        <Rich<'a, Token> as LabelError<'a, TokenStream<'a>, L>>::in_context(
            &mut self.rich,
            label,
            span,
        );
    }
}

pub(crate) fn custom_error<'a>(
    span: SimpleSpan,
    msg: impl ToString,
    kind: ParseDiagnosticKind,
) -> ParseFailure<'a> {
    with_diagnostic_kind(ParseFailure::custom(span, msg), kind)
}
pub(crate) fn with_default_diagnostic_kind<'a>(
    mut err: ParseFailure<'a>,
    kind: ParseDiagnosticKind,
) -> ParseFailure<'a> {
    err.kind.get_or_insert(kind);
    err
}
pub(crate) fn with_diagnostic_kind<'a>(
    mut err: ParseFailure<'a>,
    kind: ParseDiagnosticKind,
) -> ParseFailure<'a> {
    err.kind = Some(kind);
    err
}
#[cfg(test)]
mod tests {
    use super::*;
    type Failure = ParseFailure<'static>;
    fn generic(label: &'static str) -> Failure {
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::expected_found(
            [label],
            None,
            (2..3).into(),
        )
    }
    fn merge(left: Failure, right: Failure) -> Failure {
        <Failure as Error<'static, TokenStream<'static>>>::merge(left, right)
    }
    fn direct(message: &str) -> Failure {
        custom_error(
            (2..3).into(),
            message,
            ParseDiagnosticKind::CommandModeError,
        )
    }
    #[test]
    fn diagnostic_matching_preserves_tree_and_recovery_reason_rules() {
        let expected = generic("item");
        let custom = ParseFailure::custom(*expected.span(), expected.reason().to_string());
        assert!(!expected.matches_tree_diagnostic(&custom));
        assert!(expected.matches_recovery_diagnostic(&custom));

        // Existing tree matching compares rendered patterns, not RichPattern variants.
        let token = <Failure as LabelError<
            'static,
            TokenStream<'static>,
            RichPattern<'static, Token>,
        >>::expected_found(
            [RichPattern::Token(Token::Char('x').into())],
            None,
            (2..3).into(),
        );
        let label = generic("'x'");
        assert!(token.matches_tree_diagnostic(&label));
        assert!(token.matches_recovery_diagnostic(&label));
    }

    #[test]
    fn context_attachment_preserves_origin_and_distinguishes_diagnostics() {
        let original = direct("inner error").at_source((7..9).into());
        let contextual = original.clone().with_context("argument", (5..10).into());
        assert_eq!(contextual.span(), original.span());
        assert_eq!(contextual.direct, original.direct);
        assert_eq!(contextual.kind, original.kind);
        assert!(!contextual.matches_tree_diagnostic(&original));
        assert!(!contextual.matches_recovery_diagnostic(&original));
        let repeated = contextual.clone().with_context("argument", (5..10).into());
        assert!(contextual.matches_tree_diagnostic(&repeated));
        assert!(contextual.matches_recovery_diagnostic(&repeated));
    }
    #[test]
    fn metadata_follows_selected_reason_in_both_merge_directions() {
        for error in [
            merge(generic("item"), direct("first")),
            merge(direct("first"), generic("item")),
            merge(direct("first"), direct("second")),
        ] {
            assert_eq!(error.kind, Some(ParseDiagnosticKind::CommandModeError));
            assert!(matches!(error.reason(), RichReason::Custom(message) if message == "first"));
        }
        let error = merge(
            with_diagnostic_kind(generic("first"), ParseDiagnosticKind::ArgumentValidation),
            generic("second"),
        );
        assert_eq!(error.kind, Some(ParseDiagnosticKind::ArgumentValidation));
        assert_eq!(error.expected().count(), 2);
    }
    #[test]
    fn merge_preserves_rich_context_selection_and_expected_found_order() {
        let mut left = generic("left");
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::in_context(
            &mut left,
            "left context",
            (1..4).into(),
        );
        let mut right = direct("right reason");
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::in_context(
            &mut right,
            "right context",
            (0..4).into(),
        );
        let merged = merge(left, right);
        assert_eq!(
            merged
                .contexts()
                .map(|(label, _)| label.to_string())
                .collect::<Vec<_>>(),
            ["left context"]
        );
        assert_eq!(merged.kind, Some(ParseDiagnosticKind::CommandModeError));
        let generic =
            with_diagnostic_kind(generic("first"), ParseDiagnosticKind::ArgumentValidation);
        let merged =
            <Failure as LabelError<'static, TokenStream<'static>, &str>>::merge_expected_found(
                generic,
                ["second", "first"],
                None,
                (2..3).into(),
            );
        assert_eq!(
            merged
                .expected()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(merged.kind, Some(ParseDiagnosticKind::ArgumentValidation));
    }
    #[test]
    fn replacement_and_labels_clear_stale_semantics() {
        let replaced =
            <Failure as LabelError<'static, TokenStream<'static>, &str>>::replace_expected_found(
                direct("old").control(),
                ["new"],
                None,
                (8..9).into(),
            );
        assert_eq!(replaced.kind, None);
        assert!(!replaced.is_control);
        assert_eq!(*replaced.span(), (8..9).into());
        let mut labelled = direct("old").control();
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::label_with(
            &mut labelled,
            "new",
        );
        assert_eq!(labelled.kind, None);
        assert!(!labelled.is_control);
    }
    #[test]
    fn ownership_and_offset_preserve_semantics_and_contexts() {
        let mut error = direct("diagnostic").control();
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::in_context(
            &mut error,
            "argument",
            (1..4).into(),
        );
        let shifted = error.clone().into_owned().shifted(10);
        assert_eq!(*error.span(), (2..3).into());
        assert_eq!(*shifted.span(), (12..13).into());
        assert_eq!(*shifted.contexts().next().unwrap().1, (11..14).into());
        assert_eq!(shifted.kind, error.kind);
        assert!(shifted.is_control);
    }
    #[test]
    fn direct_source_and_environment_names_follow_the_selected_reason() {
        let environment = custom_error(
            (8..12).into(),
            "closing environment rejected",
            ParseDiagnosticKind::EnvironmentNameMismatch,
        )
        .at_environment((8..12).into(), "align".into(), "matrix".into());
        for error in [
            merge(generic("item"), environment.clone()),
            merge(environment.clone(), generic("item")),
            merge(
                environment.clone(),
                direct("other").at_source((30..35).into()),
            ),
        ] {
            let shifted = error.into_owned().shifted(100);
            let source = shifted.direct.as_ref().expect("selected direct source");
            assert_eq!(source.span, (108..112).into());
            assert_eq!(source.environment.as_ref().unwrap().name, "align");
            assert_eq!(source.environment.as_ref().unwrap().expected_name, "matrix");
        }
        let mut labelled = environment.clone();
        <Failure as LabelError<'static, TokenStream<'static>, &str>>::label_with(
            &mut labelled,
            "new",
        );
        assert!(labelled.direct.is_none());
        let replaced =
            <Failure as LabelError<'static, TokenStream<'static>, &str>>::replace_expected_found(
                environment,
                ["new"],
                None,
                (0..1).into(),
            );
        assert!(replaced.direct.is_none());
    }
}
