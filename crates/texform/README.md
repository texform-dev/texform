# texform

> **The missing foundation for LaTeX formula processing.**

TeXForm parses, edits, and transforms LaTeX math, built on a structured knowledge base of 600+ command and environment specifications across 7 LaTeX packages, validated against MathJax, KaTeX, and XeTeX.

This crate is the public TeXForm facade — the only crate with a stability guarantee. It exposes the full API surface: a parse-only `Parser`, an editable `Document` tree, a profile-based `TransformEngine`, canonical serialization, and `validate_argspec`.

## Quick start

```bash
cargo add texform
```

```rust
use texform::{Profile, TransformEngine};

// Normalize a formula into a canonical form chosen by profile.
let engine = TransformEngine::builder().profile(Profile::Corpus).build()?;
let normalized = engine.normalize(r"a \over b")?;
assert_eq!(normalized, r"\frac { a } { b }");

// Parse through the engine, transform the live document in place, then serialize.
let (mut document, _) = engine.parser().parse(r"a \over b").try_into_document()?;
engine.transform(&mut document)?;
assert_eq!(document.to_latex()?, r"\frac { a } { b }");
```

Profiles select the normalization target: `Authoring` (polished author-facing output), `Faithful` (render-faithful universal forms), `Corpus` (complete canonical training labels), and `Equiv` (an aggressive intermediate for equivalence comparison). `Equiv` adds rules such as rewriting centered `\cfrac` forms to `\frac`, discarding continued-fraction styling that `Corpus` retains. Its output is intended for comparison, deduplication, or fingerprints rather than training labels for the original images.

## Stability

`texform` follows semantic versioning and is the only public entry point. The `texform-*` crates it depends on are internal implementation details — they are published only because crates.io requires it, and their APIs may change in any release. Do not depend on them directly.

## Links

- [Repository README](../../README.md) — full README, examples, and contribution guide
- [API documentation](https://docs.rs/texform)
- [Architecture overview](../../ARCHITECTURE.md)
- [Playground](https://play.texform.dev) — try TeXForm in the browser

## License

Apache-2.0.
