use crate::Document;
use crate::ParseDiagnostic;

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

    pub fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        self.diagnostics.as_slice()
    }

    pub fn into_diagnostics(self) -> Vec<ParseDiagnostic> {
        self.diagnostics
    }

    pub fn has_errors(&self) -> bool {
        self.document.as_ref().is_some_and(Document::has_errors)
    }

    pub fn try_into_document(self) -> Result<(Document, Vec<ParseDiagnostic>), ParseError> {
        match (self.document, self.diagnostics) {
            (Some(document), diagnostics) if !document.has_errors() => Ok((document, diagnostics)),
            (document, diagnostics) => Err(ParseError {
                document: document.map(Box::new),
                diagnostics,
            }),
        }
    }

    pub fn into_parts(self) -> (Option<Document>, Vec<ParseDiagnostic>) {
        (self.document, self.diagnostics)
    }
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub document: Option<Box<Document>>,
    pub diagnostics: Vec<ParseDiagnostic>,
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

impl From<texform_core::parse::ParseError> for ParseError {
    fn from(error: texform_core::parse::ParseError) -> Self {
        let (document, diagnostics) = error.into_parts();
        Self {
            document: document.map(Document::from_core).map(Box::new),
            diagnostics,
        }
    }
}
