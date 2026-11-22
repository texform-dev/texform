//! Public parse results, spans, and diagnostics shared across parse entry points.

use serde::Serialize;

use crate::document::Document;

/// Machine-readable category of a parse diagnostic.
///
/// Each variant labels why a diagnostic was emitted, letting callers branch on
/// the cause without parsing the human-readable message. The kebab-case form
/// produced by [`ParseDiagnosticKind::as_str`] is the stable wire value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[serde(rename_all = "kebab-case")]
pub enum ParseDiagnosticKind {
    /// A control sequence usable as an infix operator appeared where its
    /// left/right operands could not be resolved unambiguously.
    AmbiguousInfix,
    /// A command or environment argument failed argspec validation.
    ArgumentValidation,
    /// A command was used in a content mode it is not allowed in.
    CommandModeError,
    /// An unescaped `%` started a comment that swallowed the rest of an
    /// argument, leaving it unclosed.
    CommentTruncatedArgument,
    /// An environment was used in a content mode it is not allowed in.
    EnvironmentModeError,
    /// An `\end{...}` name did not match the opening `\begin{...}`.
    EnvironmentNameMismatch,
    /// A source character could not be tokenized, so no document was produced.
    InvalidCharacter,
    /// A `\left ... \right` group had an invalid delimiter or a missing
    /// `\right`.
    LeftRightDelimiter,
    /// Group nesting exceeded the configured maximum depth.
    MaxGroupDepthExceeded,
    /// A low-level expected-vs-found mismatch with no more specific category.
    RawExpectedFound,
    /// Sub/superscript syntax appeared in text mode, where it is not allowed.
    TextScriptError,
    /// An environment was opened but its closing `\end{...}` was missing.
    UnclosedEnvironment,
    /// An inline math segment (`$ ... $`) was opened but never closed.
    UnclosedInlineMath,
    /// A math-shift `$` appeared where it is not expected inside a math formula.
    UnexpectedMathShift,
    /// A command name is not present in the knowledge base, under a config that
    /// rejects unknown names.
    UnknownCommand,
    /// An environment name is not present in the knowledge base, under a config
    /// that rejects unknown names.
    UnknownEnvironment,
}

impl ParseDiagnosticKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            ParseDiagnosticKind::AmbiguousInfix => "ambiguous-infix",
            ParseDiagnosticKind::ArgumentValidation => "argument-validation",
            ParseDiagnosticKind::CommandModeError => "command-mode-error",
            ParseDiagnosticKind::CommentTruncatedArgument => "comment-truncated-argument",
            ParseDiagnosticKind::EnvironmentModeError => "environment-mode-error",
            ParseDiagnosticKind::EnvironmentNameMismatch => "environment-name-mismatch",
            ParseDiagnosticKind::InvalidCharacter => "invalid-character",
            ParseDiagnosticKind::LeftRightDelimiter => "left-right-delimiter",
            ParseDiagnosticKind::MaxGroupDepthExceeded => "max-group-depth-exceeded",
            ParseDiagnosticKind::RawExpectedFound => "raw-expected-found",
            ParseDiagnosticKind::TextScriptError => "text-script-error",
            ParseDiagnosticKind::UnclosedEnvironment => "unclosed-environment",
            ParseDiagnosticKind::UnclosedInlineMath => "unclosed-inline-math",
            ParseDiagnosticKind::UnexpectedMathShift => "unexpected-math-shift",
            ParseDiagnosticKind::UnknownCommand => "unknown-command",
            ParseDiagnosticKind::UnknownEnvironment => "unknown-environment",
        }
    }
}

/// Byte-offset span within the original source string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct Span {
    /// Inclusive start byte offset
    pub start: usize,
    /// Exclusive end byte offset
    pub end: usize,
}

impl Span {
    /// Construct a byte span covering `[start, end)`.
    #[inline]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Construct a zero-width byte span at `pos`.
    #[inline]
    pub const fn point(pos: usize) -> Self {
        Self {
            start: pos,
            end: pos,
        }
    }

    /// Return the minimal span covering both `self` and `other`.
    #[inline]
    pub const fn merge(self, other: Self) -> Self {
        let start = if self.start < other.start {
            self.start
        } else {
            other.start
        };
        let end = if self.end > other.end {
            self.end
        } else {
            other.end
        };
        Self { start, end }
    }
}

/// Additional source span attached to a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
pub struct ParseDiagnosticContext {
    /// Human-readable label for this related span
    pub label: String,
    /// Source location referenced by the label
    pub span: Span,
}

/// Unified parse result carrying an optional document and zero or more diagnostics.
///
/// The design pairs an optional parsed tree with diagnostics: a partial document
/// may coexist with diagnostics, so consumers always receive as much
/// information as the parser could extract.
#[derive(Debug, Clone)]
pub struct ParseResult {
    /// Parsed document, present even when diagnostics exist for recovered input.
    pub document: Option<Document>,
    /// Zero or more diagnostics; empty on full success.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl ParseResult {
    /// Borrow the parsed document, if one was produced.
    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// Borrow parse diagnostics.
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    /// Consume the result and return only diagnostics.
    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    /// `true` when a recovered document contains one or more `Error` nodes.
    pub fn has_errors(&self) -> bool {
        self.document.as_ref().is_some_and(Document::has_errors)
    }

    /// Return the document and diagnostics when the document is editable.
    pub fn try_into_document(self) -> Result<(Document, Vec<ParseDiagnostic>), ParseError> {
        match (self.document, self.diagnostics) {
            (Some(document), diagnostics) if !document.has_errors() => Ok((document, diagnostics)),
            (document, diagnostics) => Err(ParseError {
                diagnostics,
                document: document.map(Box::new),
            }),
        }
    }

    /// Consume the result into its two public parts.
    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document, self.diagnostics)
    }
}

/// A single diagnostic produced during parsing.
///
/// Diagnostics carry both a human-readable message and structured
/// expected/found information for richer error reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "tsify", derive(tsify_next::Tsify))]
#[non_exhaustive]
pub struct ParseDiagnostic {
    /// Stable machine-readable diagnostic kind, when available
    pub kind: Option<ParseDiagnosticKind>,
    /// Human-readable error description
    pub message: String,
    /// Source location of the error
    pub span: Span,
    /// Tokens or patterns the parser expected at this point
    pub expected: Vec<String>,
    /// Token actually found, if any
    pub found: Option<String>,
    /// Additional related source ranges for richer diagnostics
    pub contexts: Vec<ParseDiagnosticContext>,
}

impl ParseDiagnostic {
    pub fn new(
        message: impl Into<String>,
        span: Span,
        expected: Vec<String>,
        found: Option<String>,
        contexts: Vec<ParseDiagnosticContext>,
    ) -> Self {
        Self {
            kind: None,
            message: message.into(),
            span,
            expected,
            found,
            contexts,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub diagnostics: Vec<ParseDiagnostic>,
    pub document: Option<Box<Document>>,
}

impl ParseError {
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    pub fn document(&self) -> Option<&Document> {
        self.document.as_deref()
    }

    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document.map(|document| *document), self.diagnostics)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.document.is_some() {
            f.write_str("parse produced an incomplete document")
        } else {
            f.write_str("parse produced no document")
        }
    }
}

impl std::error::Error for ParseError {}
