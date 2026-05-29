#[derive(Debug)]
pub enum Error {
    MissingProfile,
    UnknownRule(String),
    ParserBuild(crate::parser::ParserBuildError),
    TransformBuild(texform_transform::TransformBuildError),
    Parse(crate::parse_result::ParseError),
    IncompleteTree,
    Transform(texform_transform::TransformError),
    Serialize(crate::serialize::SerializeError),
}

pub type NormalizeError = Error;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfile => f.write_str("engine profile is required"),
            Self::UnknownRule(name) => write!(f, "unknown transform rule: {name}"),
            Self::ParserBuild(error) => write!(f, "failed to build parser: {error}"),
            Self::TransformBuild(error) => write!(f, "failed to build transform plan: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::IncompleteTree => f.write_str("cannot transform a document with parse errors"),
            Self::Transform(error) => error.fmt(f),
            Self::Serialize(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<crate::parse_result::ParseError> for Error {
    fn from(error: crate::parse_result::ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<texform_transform::TransformError> for Error {
    fn from(error: texform_transform::TransformError) -> Self {
        Self::Transform(error)
    }
}

impl From<crate::serialize::SerializeError> for Error {
    fn from(error: crate::serialize::SerializeError) -> Self {
        Self::Serialize(error)
    }
}
