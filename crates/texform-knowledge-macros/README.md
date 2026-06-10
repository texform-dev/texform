# texform-knowledge-macros

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This procedural-macro crate provides `argspec!`, which parses and validates an argument-specification string literal at compile time and expands to the corresponding static data structure. It exists so that every argspec embedded in `texform-knowledge`'s generated records is checked by the same `texform-argspec` parser used at runtime — an invalid spec is a compile error, not a latent runtime bug.

Macro misuse cases are covered by `trybuild` UI tests in `texform-knowledge`.
