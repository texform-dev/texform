//! Top-level transform errors.

use crate::rewrite::{PlanBuildError, RewriteError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformError {
    Build(TransformBuildError),
    NotConverged { max_rounds: usize },
    Rewrite(RewriteError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransformBuildError {
    Rewrite(PlanBuildError),
}

impl std::fmt::Display for TransformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformError::NotConverged { max_rounds } => {
                write!(f, "transform did not converge within {max_rounds} rounds")
            }
            TransformError::Build(error) => error.fmt(f),
            TransformError::Rewrite(error) => error.fmt(f),
        }
    }
}

impl std::fmt::Display for TransformBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformBuildError::Rewrite(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for TransformError {}
impl std::error::Error for TransformBuildError {}
