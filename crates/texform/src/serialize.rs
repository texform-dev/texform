//! Serialization of a [`Document`](crate::Document) back to LaTeX text.
//!
//! The canonical serializer is reached through [`Document::to_latex`](crate::Document::to_latex),
//! [`Document::to_latex_with`](crate::Document::to_latex_with), and the opt-in
//! token sidecar methods [`Document::to_tokenized_latex`](crate::Document::to_tokenized_latex)
//! and [`Document::to_tokenized_latex_with`](crate::Document::to_tokenized_latex_with).
//! Tokenized serialization records the canonical serializer's own output boundaries;
//! it is not a raw-string lexer, AST dump, or rendered glyph stream.
//!
//! Style axes live on the flat [`SerializeOptions`] struct:
//!
//! ```
//! use texform::{Parser, ScriptSpacing, SerializeOptions};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = Parser::builder().build()?;
//! let (document, _) = parser.parse(r"x^2").try_into_document()?;
//! let options = SerializeOptions {
//!     script_spacing: ScriptSpacing::Compact,
//!     ..SerializeOptions::default()
//! };
//! assert_eq!(document.to_latex_with(&options)?, r"x^{ 2 }");
//! # Ok(())
//! # }
//! ```

pub use texform_core::serialize::{
    AdjacentCharSpacing, CommandSpacing, EnvironmentNameSpacing, InfixGrouping,
    MathGroupInnerSpacing, ScriptOrder, ScriptSpacing, SerializationToken, SerializationTokenKind,
    SerializeError, SerializeOptions, TokenizedLatex,
};
