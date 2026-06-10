# texform-interface

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate defines the dependency-free shared types used across the TeXForm workspace. The most important is `SyntaxNode` (`src/syntax_node.rs`): the lossless, immutable parse snapshot that serves as the single serde DTO — it backs JSON snapshots, Python dictionaries, JavaScript objects, and test fixtures with the same tagged shape everywhere.

The optional `tsify` feature derives TypeScript type definitions for the WASM binding build.
