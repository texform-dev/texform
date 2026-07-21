//! Serialization of a [`Document`](crate::Document) back to LaTeX text.
//!
//! The canonical serializer is reached through [`Document::to_latex`](crate::Document::to_latex),
//! [`Document::to_latex_with`](crate::Document::to_latex_with), and the opt-in
//! token sidecar methods [`Document::to_tokenized_latex`](crate::Document::to_tokenized_latex)
//! and [`Document::to_tokenized_latex_with`](crate::Document::to_tokenized_latex_with).
//! Tokenized serialization records the canonical serializer's own output boundaries;
//! it is not a raw-string lexer, AST dump, or rendered glyph stream.

pub use texform_core::serialize::{
    SerializationToken, SerializationTokenKind, SerializeError, SerializeOptions, TokenizedLatex,
};
