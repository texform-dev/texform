//! Outcome of a parse call.
//!
//! [`ParseResult`] is what [`Parser::parse`](crate::Parser::parse) returns: an
//! optional [`Document`] plus the [`ParseDiagnostic`]s
//! emitted while parsing. Use [`ParseResult::try_into_document`] when downstream
//! code needs a complete, editable tree; it returns [`ParseError`] when the
//! parse produced no document or a document containing parse-error nodes.

use crate::Document;
use crate::ParseDiagnostic;

/// Result of parsing a LaTeX formula.
///
/// When parsing succeeds far enough to produce a [`Document`], that document
/// keeps the identity of the parser context that produced it. A document
/// extracted from a result returned by [`TransformEngine::parser`](crate::TransformEngine::parser)
/// can be edited and then transformed in place by that same engine.
#[derive(Debug, Clone)]
pub struct ParseResult {
    document: Option<Document>,
    diagnostics: Vec<ParseDiagnostic>,
}

impl ParseResult {
    pub(crate) fn from_core(result: texform_core::parse::ParseResult) -> Self {
        let (document, diagnostics) = result.into_parts();
        Self {
            document: document.map(Document::from_core),
            diagnostics,
        }
    }

    #[doc(hidden)]
    pub fn __from_parts_for_tests(
        document: Option<Document>,
        diagnostics: Vec<ParseDiagnostic>,
    ) -> Self {
        Self {
            document,
            diagnostics,
        }
    }

    /// Borrow the parsed document, if one was produced.
    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// Diagnostics emitted while parsing.
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    /// Consume the result and return only its diagnostics.
    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    /// Whether the parsed document contains parse-error nodes.
    pub fn has_errors(&self) -> bool {
        self.document.as_ref().is_some_and(Document::has_errors)
    }

    /// Consume the result and return a complete document plus diagnostics.
    ///
    /// The returned document keeps the parser identity needed by
    /// [`TransformEngine::transform`](crate::TransformEngine::transform). If
    /// parsing produced no document or produced a document with parse-error
    /// nodes, this returns [`ParseError`] instead.
    pub fn try_into_document(self) -> Result<(Document, Vec<ParseDiagnostic>), ParseError> {
        match (self.document, self.diagnostics) {
            (Some(document), diagnostics) if !document.has_errors() => Ok((document, diagnostics)),
            (document, diagnostics) => Err(ParseError {
                document: document.map(Box::new),
                diagnostics,
            }),
        }
    }

    /// Consume the result into the optional document and diagnostics.
    ///
    /// If a document is present, it keeps the parser identity needed by
    /// [`TransformEngine::transform`](crate::TransformEngine::transform).
    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document, self.diagnostics)
    }
}

/// Parse failure that may still carry a partial document.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Partial document, if parsing produced one before failing.
    pub document: Option<Box<Document>>,
    /// Diagnostics emitted while parsing.
    pub diagnostics: Vec<ParseDiagnostic>,
}

impl ParseError {
    /// Diagnostics emitted while parsing.
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    /// Borrow the partial document, if one was produced.
    pub fn document(&self) -> Option<&Document> {
        self.document.as_deref()
    }

    /// Consume the error and return only its diagnostics.
    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    /// Consume the error into the optional partial document and diagnostics.
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

impl From<texform_core::parse::ParseError> for ParseError {
    fn from(error: texform_core::parse::ParseError) -> Self {
        let (document, diagnostics) = error.into_parts();
        Self {
            document: document.map(Document::from_core).map(Box::new),
            diagnostics,
        }
    }
}
