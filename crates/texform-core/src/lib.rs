//! TeXForm core library — LaTeX formula parsing, AST, and transformation.
//!
//! # Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`context`] | [`ParseContext`](context::ParseContext): the main public API for configuring knowledge and parsing |
//! | [`api`] | High-level convenience functions ([`parse_latex`](api::parse_latex), batch probing) |
//! | [`ast`] | Mutable arena-backed AST for tree transforms |
//! | [`parser`] | Chumsky-based parser producing [`SyntaxNode`](texform_interface::syntax_node::SyntaxNode) trees |
//! | [`lexer`] | Logos-based lexer mapping LaTeX source to tokens |
//! | [`column_parser`] | Standalone parser for `{lcc}` column specifications |

pub mod api;
pub mod ast;
pub mod column_parser;
pub mod context;
pub mod lexer;
pub mod parser;
pub mod serialize;
pub mod transform;

mod knowledge;
