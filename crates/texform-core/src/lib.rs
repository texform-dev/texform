//! TeXForm core library — LaTeX formula parsing, AST, and transformation.
//!
//! # Modules
//!
//! | Module | Purpose |
//! |---|---|
//! | [`parse`] | [`Parser`](parse::Parser): the main public API for configuring knowledge and parsing |
//! | [`ast`] | Mutable arena-backed AST for tree transforms |
//! | [`parser`] | Chumsky-based parser producing [`SyntaxNode`](texform_interface::syntax_node::SyntaxNode) trees |
//! | [`lexer`] | Logos-based lexer mapping LaTeX source to tokens |
//! | [`column_parser`] | Standalone parser for `{lcc}` column specifications |

pub mod ast;
pub mod column_parser;
pub mod lexer;
pub mod parse;
pub mod parser;
pub mod serialize;
pub mod target_counter;

mod dimension;
pub mod knowledge;
