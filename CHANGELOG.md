# Changelog

All notable changes to TeXForm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). A single version number covers the Rust crate ([crates.io](https://crates.io/crates/texform)), the Python package ([PyPI](https://pypi.org/project/texform/)), and the JavaScript package ([npm](https://www.npmjs.com/package/texform)).

## [0.1.0] - 2026-06-12

Initial public release of TeXForm — a LaTeX formula parser, editable document model, and normalization engine, available in Rust, Python, and JavaScript from a single Rust core.

### Added

- Knowledge-driven parser backed by 530+ command and environment specifications across the `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, and `textmacros` packages, with strict and lenient modes that preserve unknown commands and unparseable fragments as explicit nodes instead of failing the parse.
- Editable `Document` tree with validated, fallible edits and canonical LaTeX serialization that guarantees text idempotency over parse/serialize cycles.
- Profile-based transform engine with four normalization profiles — `Authoring`, `Faithful`, `Corpus`, and `Equiv` — covering author-facing cleanup, render-faithful expansion, corpus preparation, and formula-equivalence comparison.
- `validate_argspec` for checking xparse-style argument specifications.
- Python (PyPI `texform`, Python ≥ 3.10) and JavaScript/TypeScript (npm `texform`, WebAssembly) bindings exposing the same parser, document, and transform engine from the shared Rust core.
