# texform

> **The LaTeX math toolkit**

TeXForm parses, edits, and normalizes LaTeX math using a knowledge base of 600+ command and environment specifications across 8 LaTeX packages, validated against MathJax, KaTeX, and XeTeX.

This crate is the public TeXForm facade — the only crate with a stability guarantee. It exposes the full API surface: a parse-only `Parser`, an editable `Document` tree, a profile-based `TransformEngine`, deterministic serialization, and `validate_argspec`.

[Website](https://texform.dev) · [Documentation](https://texform.dev/docs) · [API reference](https://docs.rs/texform) · [Playground](https://play.texform.dev) · [Changelog](https://texform.dev/docs/changelog)

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

Profiles select the normalization target: `Authoring` (polished author-facing output), `Faithful` (render-faithful universal forms), `Corpus` (complete canonical training labels), and `Equiv` (an aggressive intermediate for equivalence comparison). `Equiv` may discard visually salient notation such as continued-fraction styling, so use its output for comparison, deduplication, or fingerprints rather than as training labels. See [Transforms](https://texform.dev/docs/transforms) for what each profile changes.

## Learn more

- [Quick start](https://texform.dev/docs/quick-start) — profiles, the document tree, and parse results in ten minutes
- [Guides](https://texform.dev/docs/normalize-a-corpus) — normalizing a corpus, editing documents, injecting custom commands, and more
- [LaTeX reference](https://texform.dev/latex) and [rule reference](https://texform.dev/rules) — what the parser understands and what each rewrite rule does
- [`texform-cli`](https://crates.io/crates/texform-cli) — the `texform` command-line tool

## Stability

`texform` follows semantic versioning and is the only public entry point. The `texform-*` crates it depends on are internal implementation details — they are published only because crates.io requires it, and their APIs may change in any release. Do not depend on them directly. See [Stability and versioning](https://texform.dev/docs/stability-and-versioning).

## License

Apache-2.0.
