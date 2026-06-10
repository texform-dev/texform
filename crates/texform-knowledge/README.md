# texform-knowledge

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate is TeXForm's command and environment knowledge base: which names are known, in which package, in which mode, and with what argument shape. At build time, `build.rs` compiles the YAML specifications in [`resources/specs/`](https://github.com/texform-dev/texform/tree/main/resources/specs) into static Rust records (`src/builtin/generated.rs`), so lookups at runtime are allocation-free table reads.

Records cover commands, environments, characters, and delimiters across the built-in packages (`base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, `textmacros`). Argument shapes are expressed in the `texform-argspec` language and validated at compile time by the `argspec!` macro from `texform-knowledge-macros`.
