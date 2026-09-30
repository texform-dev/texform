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

### Shared knowledge bases

Default parsers, engines, and documents share one immutable knowledge base. For custom packages or definitions, build a `KnowledgeBase` once and pass that same instance to each consumer; independently constructed instances are distinct even when their contents match. Package selection and record queries belong to the knowledge base.

```rust
use texform::{KnowledgeBase, Parser, Profile, TransformEngine};

let kb = KnowledgeBase::builder().packages(&["base", "ams"]).build()?;
let parser = Parser::builder().knowledge_base(kb.clone()).build();
let engine = TransformEngine::builder()
    .knowledge_base(kb)
    .profile(Profile::Corpus)
    .build()?;
let (mut document, _) = parser.parse(r"a \over b").try_into_document()?;
engine.transform(&mut document)?;
```

## Stability

`texform` follows semantic versioning and is the only public entry point. The `texform-*` crates it depends on are internal implementation details — they are published only because crates.io requires it, and their APIs may change in any release. Do not depend on them directly.

## Links

- [Repository README](../../README.md) — full README, examples, and contribution guide
- [API documentation](https://docs.rs/texform)
- [Architecture overview](../../ARCHITECTURE.md)
- [Playground](https://play.texform.dev) — try TeXForm in the browser

## Constructing documents

Documents share an immutable `KnowledgeBase`. Constructors accept `Arg` values: detached nodes, slot source strings, star Booleans, `Absent`, or `Arg::paired(value, open, close)?`. A signature can receive all slots or only its required slots. For example, `doc.create_command("sqrt", ["x".into()])?` fills its mandatory slot and leaves the optional slot empty. Use `doc.in_mode(ContentMode::Text)` for text-context construction and `doc.parse_fragment(source, mode)` for a detached source fragment.

A source string in a children list (groups, inline math, delimited groups, environment bodies) is parsed and its nodes are spliced into the list. Invalid shapes fail atomically with a structured `ConformanceError`; invalid source fragments retain parser diagnostics. Complete syntax snapshots are checked on import, while snapshots containing Error nodes remain structurally checked, read-only documents. `serialize` and `to_latex` check only structure, not conformance to a knowledge base. See [the architecture reference](../../ARCHITECTURE.md) for local constraints and serialization limits.

## License

Apache-2.0.
