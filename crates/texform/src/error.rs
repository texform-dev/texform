#[derive(Debug)]
pub enum Error {
    MissingProfile,
    ParserBuild(crate::parser::ParserBuildError),
    TransformBuild(texform_transform::TransformBuildError),
    Parse(texform_core::parse::ParseAstError),
    Transform(texform_transform::TransformError),
    Serialize(crate::serialize::SerializeError),
}

pub type NormalizeError = Error;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfile => f.write_str("engine profile is required"),
            Self::ParserBuild(error) => write!(f, "failed to build parser: {error}"),
            Self::TransformBuild(error) => write!(f, "failed to build transform plan: {error}"),
            Self::Parse(error) => error.fmt(f),
            Self::Transform(error) => error.fmt(f),
            Self::Serialize(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<texform_core::parse::ParseAstError> for Error {
    fn from(error: texform_core::parse::ParseAstError) -> Self {
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
