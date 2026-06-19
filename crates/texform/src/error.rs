/// Error returned by transform and normalize APIs.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// No transform profile was selected when building the engine.
    MissingProfile,
    /// A rule name passed to the builder does not exist in the selected profile.
    UnknownRule(String),
    /// The parser context could not be built.
    ParserBuild(crate::parser::ParserBuildError),
    /// The transform plan could not be built.
    TransformBuild(TransformBuildError),
    /// Parsing failed before a complete document could be produced.
    Parse(crate::parse_result::ParseError),
    /// The document contains parse-error nodes and cannot be transformed.
    IncompleteTree,
    /// The document was not parsed by this transform engine's parser.
    ///
    /// [`TransformEngine::transform`](crate::TransformEngine::transform)
    /// accepts only documents extracted from parse results produced by the
    /// same engine's [`parser`](crate::TransformEngine::parser). Documents
    /// parsed by a different parser, created with
    /// [`Document::new`](crate::Document::new), or rebuilt with
    /// [`Document::from_syntax`](crate::Document::from_syntax) return this
    /// error.
    ForeignDocument,
    /// A transform rule failed while rewriting the tree.
    Transform(TransformError),
    /// The normalized document could not be serialized.
    Serialize(crate::serialize::SerializeError),
}

pub type NormalizeError = Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformBuildError {
    message: String,
}

impl TransformBuildError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for TransformBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TransformBuildError {}

impl From<texform_transform::TransformBuildError> for TransformBuildError {
    fn from(error: texform_transform::TransformBuildError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransformError {
    message: String,
}

impl TransformError {
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TransformError {}

impl From<texform_transform::TransformError> for TransformError {
    fn from(error: texform_transform::TransformError) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfile => f.write_str("engine profile is required"),
            Self::UnknownRule(name) => write!(f, "unknown transform rule: {name}"),
            Self::ParserBuild(error) => write!(f, "failed to build parser: {error}"),
            Self::TransformBuild(error) => write!(f, "failed to build transform plan: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::IncompleteTree => f.write_str("cannot transform a document with parse errors"),
            Self::ForeignDocument => {
                f.write_str("document was not parsed by this transform engine")
            }
            Self::Transform(error) => error.fmt(f),
            Self::Serialize(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<texform_transform::TransformBuildError> for Error {
    fn from(error: texform_transform::TransformBuildError) -> Self {
        Self::TransformBuild(error.into())
    }
}

impl From<crate::parse_result::ParseError> for Error {
    fn from(error: crate::parse_result::ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<texform_transform::TransformError> for Error {
    fn from(error: texform_transform::TransformError) -> Self {
        Self::Transform(error.into())
    }
}

impl From<crate::serialize::SerializeError> for Error {
    fn from(error: crate::serialize::SerializeError) -> Self {
        Self::Serialize(error)
    }
}
