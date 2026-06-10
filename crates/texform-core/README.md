# texform-core

Internal implementation crate for [texform](https://crates.io/crates/texform). Do not depend on this crate directly — its API has no stability guarantees and may change in any release. Use the `texform` facade crate instead.

This crate holds the heart of TeXForm: the lexer and chumsky-based parser (`src/parse/`), the internal `Ast` arena (`src/ast.rs`), the public `Document` DOM layer (`src/document.rs`), and the canonical serializer (`src/serialize.rs`).

The parser consults `texform-knowledge` for command and environment signatures (described in the `texform-argspec` language) and emits `SyntaxNode` snapshots defined in `texform-interface`. `Document` wraps the panic-contract `Ast` arena with a fallible, validated editing API; the facade re-exports it as the public tree type.

See [`ARCHITECTURE.md`](https://github.com/texform-dev/texform/blob/main/ARCHITECTURE.md) for how the trees and the pipeline fit together.
