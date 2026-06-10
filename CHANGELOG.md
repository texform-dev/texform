# Changelog

All notable changes to TeXForm are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html). A single version number covers the Rust crate ([crates.io](https://crates.io/crates/texform)), the Python package ([PyPI](https://pypi.org/project/texform/)), and the JavaScript package ([npm](https://www.npmjs.com/package/texform)).

## [Unreleased]

## [0.1.0] - TBD

Initial release.

### Added

- Knowledge-driven LaTeX formula parser covering the `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, and `textmacros` packages, with strict/lenient parse configuration, unknown-command preservation, and first-class error recovery.
- Editable `Document` tree with validated, fallible editing and canonical LaTeX serialization carrying a text-idempotency guarantee.
- Profile-based transform engine with `Authoring`, `Faithful`, `Corpus`, and `Equiv` normalization profiles.
- `validate_argspec` for validating xparse-style argument specifications.
- Python bindings (PyPI `texform`, Python ≥ 3.10) and JavaScript/TypeScript bindings (npm `texform`, WebAssembly) over the same Rust core.
