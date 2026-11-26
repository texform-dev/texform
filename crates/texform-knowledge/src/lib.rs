extern crate self as texform_knowledge;

pub use texform_knowledge_macros::argspec;

pub mod builtin;
/// Built-in package lists: the import order and the default selection.
pub mod packages;
pub mod specs;
