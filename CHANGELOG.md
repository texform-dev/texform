# Changelog

All notable changes to TeXForm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). A single version number covers the Rust crate ([crates.io](https://crates.io/crates/texform)), the Python package ([PyPI](https://pypi.org/project/texform/)), and the JavaScript package ([npm](https://www.npmjs.com/package/texform)).

## [0.2.0] - 2026-07-03

This release adds in-place document normalization to the Python and JavaScript bindings, teaches the parser and serializer to preserve whitespace and spacing faithfully, and makes parsing and transforming large formulas dramatically faster.

### Added

- `TransformEngine.transform` on the Python and JavaScript bindings, for normalizing a live `Document` in place. Every parsed document is stamped with a parse-context id, and a document produced by a different parser (or by `Document.from_syntax`) is rejected with a foreign-document error, so a document is only transformed by the engine that produced it.
- `:O` operator-name argspec content, so arguments to `\operatorname` and `\DeclareMathOperator` are modeled as math content and serialized compactly without special-casing command names.
- Typed serialization options for the Python bindings: `serialize()` and `Document.to_latex()` now accept a documented `SerializeOptions` TypedDict instead of an untyped `dict`, mirroring the TypeScript option interfaces.

### Changed

- **Breaking:** the public `Error` enum is now `#[non_exhaustive]`. Exhaustive `match` arms over `texform::Error` in downstream Rust must add a wildcard arm; in return, future error variants can be introduced without another breaking change.
- Parsing and transforming large formulas is dramatically faster. Release builds no longer run debug-only structural invariant sweeps, which were quadratic on wide formulas (a 60k-character formula now parses ~14× faster and transforms ~45× faster); source spans are carried as a positional tree (10–25% faster parsing); and rewrite rules are indexed by trigger name (31–48% faster transforms). Normalized output is byte-identical.

### Fixed

- Edge whitespace in text arguments is preserved: `\text{ or }` and `\textbf{ a }` no longer drop their leading and trailing spaces on parse, serialize, or transform.
- Adjacent math digits stay compact: multi-digit numbers such as `1093^2` no longer serialize as `1 0 9 3 ^ { 2 }` under the default spacing option, while letters and symbols still honor it.
- Tight argspec slot spacing is preserved, so no-leading-space slots — including linebreak dimensions and custom tight optional slots — stay tight through parse/serialize round trips.
- Whitespace is kept outside attribute wrappers during transforms.
- Inline math inside text-mode control sequences, and whitespace-only text arguments, are now accepted.
- The recovery parser is hardened against an unnecessary unwrap.
- The license file is included in the Python sdist.

## [0.1.0] - 2026-06-12

Initial public release of TeXForm — a LaTeX formula parser, editable document model, and normalization engine, available in Rust, Python, and JavaScript from a single Rust core.

### Added

- Knowledge-driven parser backed by 530+ command and environment specifications across the `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, and `textmacros` packages, with strict and lenient modes that preserve unknown commands and unparseable fragments as explicit nodes instead of failing the parse.
- Editable `Document` tree with validated, fallible edits and canonical LaTeX serialization that guarantees text idempotency over parse/serialize cycles.
- Profile-based transform engine with four normalization profiles — `Authoring`, `Faithful`, `Corpus`, and `Equiv` — covering author-facing cleanup, render-faithful expansion, corpus preparation, and formula-equivalence comparison.
- `validate_argspec` for checking xparse-style argument specifications.
- Python (PyPI `texform`, Python ≥ 3.10) and JavaScript/TypeScript (npm `texform`, WebAssembly) bindings exposing the same parser, document, and transform engine from the shared Rust core.
