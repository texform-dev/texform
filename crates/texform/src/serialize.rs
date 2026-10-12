//! Serialization of a [`Document`](crate::Document) back to LaTeX text.
//!
//! The serializer is reached through [`Document::to_latex`](crate::Document::to_latex),
//! [`Document::to_latex_with`](crate::Document::to_latex_with), and the opt-in
//! token sidecar methods [`Document::to_tokenized_latex`](crate::Document::to_tokenized_latex)
//! and [`Document::to_tokenized_latex_with`](crate::Document::to_tokenized_latex_with).
//! Tokenized serialization records the serializer's own output boundaries;
//! it is not a raw-string lexer, AST dump, or rendered glyph stream.
//! A required control-word separator is the gap before an ASCII letter that would
//! extend an unterminated control word. Control symbols do not get that separator,
//! and text mode does not add any other spaces.
//!
//! Select [`Style::Spaced`] (byte-stable default) or [`Style::Compact`] with
//! [`SerializeOptions`]. Both styles preserve parsed structure and text
//! idempotency; compact layout may improve across releases:
//!
//! ```
//! use texform::{Parser, Style, SerializeOptions};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let parser = Parser::builder().build();
//! let (document, _) = parser.parse(r"x^2").try_into_document()?;
//! let options = SerializeOptions {
//!     style: Style::Compact,
//! };
//! assert_eq!(document.to_latex_with(&options)?, r"x^2");
//! # Ok(())
//! # }
//! ```

pub use texform_core::serialize::{
    SerializationToken, SerializationTokenKind, SerializeError, SerializeOptions, Style,
    TokenizedLatex,
};
