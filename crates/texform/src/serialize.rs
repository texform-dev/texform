//! Serialization of a [`Document`](crate::Document) back to LaTeX text.
//!
//! The canonical serializer is reached through [`Document::to_latex`](crate::Document::to_latex)
//! and [`Document::to_latex_with`](crate::Document::to_latex_with); this module
//! re-exports the configuration [`SerializeOptions`] and the failure type
//! [`SerializeError`]. Serialization guarantees text idempotency: re-parsing and
//! re-serializing canonical output always yields the same string.

pub use texform_core::serialize::{SerializeError, SerializeOptions};
