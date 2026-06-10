# texform

> **The missing foundation for LaTeX formula processing.**

TeXForm parses, edits, and transforms LaTeX math, built on a structured knowledge base of 530+ command and environment specifications across 7 LaTeX packages, validated against MathJax, KaTeX, and XeTeX.

This crate is the public TeXForm facade — the only crate with a stability guarantee. It exposes the full API surface: a parse-only `Parser`, an editable `Document` tree, a profile-based `TransformEngine`, canonical serialization, and `validate_argspec`.

## Quick start

```bash
cargo add texform
```

```rust
use texform::{Parser, Profile, TransformEngine};

// Normalize a formula into a canonical form chosen by profile.
let engine = TransformEngine::builder().profile(Profile::Corpus).build()?;
let result = engine.normalize(r"a \over b")?;
assert_eq!(result.normalized, r"\frac { a } { b }");

// Parse, inspect diagnostics, and serialize back to LaTeX.
let parser = Parser::builder().build()?;
let parsed = parser.parse(r"\frac{x}{y}");
if let Some(document) = parsed.document() {
    println!("{}", document.to_latex()?);
}
```

Profiles select the normalization target: `Authoring` (polished author-facing output), `Faithful` (same rendered formula, macros expanded), `Corpus` (training-data normalization), and `Equiv` (formula equivalence comparison).

## Stability

`texform` follows semantic versioning and is the only public entry point. The `texform-*` crates it depends on are internal implementation details — they are published only because crates.io requires it, and their APIs may change in any release. Do not depend on them directly.

## Links

- [GitHub repository](https://github.com/texform-dev/texform) — full README, examples, and contribution guide
- [API documentation](https://docs.rs/texform)
- [Architecture overview](https://github.com/texform-dev/texform/blob/main/ARCHITECTURE.md)
- [Playground](https://play.texform.dev) — try TeXForm in the browser

<!-- Full documentation: https://texform.dev (docsite goes live after 0.1.0) -->

## License

Apache-2.0.
