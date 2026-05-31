# TeXForm

A LaTeX formula parser, formatter, and transform engine.

For a high-level map of the crates, the processing pipeline, the three tree representations, and the public API guarantees, see [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Quick Start

### Normalize Example

```rust
let engine = texform::TransformEngine::builder()
    .profile(texform::Profile::Authoring)
    .build()?;

let result = engine.normalize(r"a \over b")?;
assert_eq!(result.normalized, r"\frac { a } { b }");
```

### Parse Example

```rust
let parser = texform::Parser::builder().build()?;
let result = parser.parse(r"\frac{x}{y}");

for diagnostic in result.diagnostics() {
    eprintln!("{}", diagnostic.message);
}

let Some(document) = result.document() else {
    return Ok(());
};

println!("{}", document.to_latex()?);
```

Standalone [`Parser`](crates/texform/src/parser.rs) defaults to [`ParseConfig::LENIENT`]: unknown commands are preserved as `known: false` nodes and diagnostics are collected across the whole input (suitable for playgrounds and exploration).

`Parser::parse` has three observable states:

- `None`: no document was produced; diagnostics describe the failure.
- `Some(document)` without error nodes: the document is editable.
- `Some(document)` with error nodes: the document is partial and read-only.

Empty input is a valid empty document.

### Document Edit Example

```rust
let mut document = texform::Document::new();
let x = document.create_char('x')?;
document.append_child(document.root().id(), x)?;

assert_eq!(document.to_latex()?, "x");
```

### Parse Configuration

[`ParseConfig`](crates/texform-core/src/parse/config.rs) has two orthogonal axes; **`true` means stricter** on both:

| Field | `true` (strict) | `false` (lenient) |
|-------|-----------------|-------------------|
| `reject_unknown` | Unknown command/env names become diagnostics | Preserved as `known: false` nodes |
| `abort_on_error` | Stop at the first error per item | Continue parsing to collect all diagnostics (slower) |

Named extremes: [`ParseConfig::STRICT`](crates/texform-core/src/parse/config.rs) (both `true`) and [`ParseConfig::LENIENT`](crates/texform-core/src/parse/config.rs) (both `false`, also `Default::default()`). Mixed settings use struct-update syntax:

```rust
use texform::ParseConfig;

// Reject unknown names but still collect every diagnostic.
let cfg = ParseConfig { reject_unknown: true, ..Default::default() };
```

[`TransformEngine`](crates/texform/src/transform_engine.rs) wraps the same internal [`Parser`](crates/texform/src/parser.rs) with a **strict default** (`ParseConfig::STRICT`) shared by `engine.parser().parse()` and `engine.normalize()`. Use a standalone `Parser` for lenient exploration; use `TransformEngineBuilder::default_parse_config` to change the engine-wide default. `TransformEngine` requires a [`Profile`](crates/texform-transform/src/config.rs) because normalization has no neutral canonical form — that choice is intentional and separate from parse strictness.

### validate_argspec Example

Validate an argspec string:

```rust
let result = texform::validate_argspec("m o");
assert!(result.ok);
```

## Serialization

TeXForm provides a canonical serializer through [`Document::to_latex`](crates/texform/src/document.rs). It is designed as an independent stage: it covers the full public document vocabulary and makes no assumptions about whether the input has been normalized by a transform pass.

Use `Document` as the main public tree API:

```rust
let latex = document.to_latex()?;
let syntax = document.to_syntax();
```

`SyntaxNode` remains available as a lossless serde and transport snapshot. It is not the editing API; convert it with `Document::from_syntax` before making tree edits.

### Text Idempotency

The serializer guarantees **text idempotency**:

```
serialize(parse(serialize(parse(src)))) == serialize(parse(src))
```

That is, parsing the canonical output and re-serializing always produces the same string. Note that this is a *text-level* guarantee — `parse(serialize(ast))` is not required to recover the exact same AST kind (e.g. `Explicit` vs `Implicit` group distinctions may not round-trip).

### Default Canonical Style

The default style targets the **corpus** and **equiv** use cases (MER data processing, formula equivalence comparison) with the following design choices:

- **Strong disambiguation** — implicit boundaries are made explicit. For example, `\frac12` serializes as `\frac { 1 } { 2 }`, and `x^2_i` serializes as `x _ { i } ^ { 2 }`.
- **Token-level separation in math mode** — adjacent math character atoms are space-separated (`abc` → `a b c`), inspired by the `im2markup` tokenization style, but without replicating its semantic rewriting (e.g. `\sin` is never rewritten to `\operatorname{s i n}`).
- **Text mode preserved verbatim** — text content (via `\text{...}` etc.) is never split or re-spaced. `\text{abc}` stays `\text {abc}`, not `\text { a b c }`.
- **Single-line output** — no formatting newlines are inserted around `\begin`/`\end` or after `\\`.

### Configurable Options

The serializer exposes `SerializeOptions` for style customization via `document.to_latex_with(&options)`. Key axes include:

| Option | Default | Effect |
|--------|---------|--------|
| `math.spacing.commands` | `Spaced` | Space between command and `{`/`[`: `\frac { a }` vs `\frac{ a }` |
| `math.spacing.group_inner_spacing` | `Padded` | Padding inside math braces: `{ a }` vs `{a}` |
| `math.spacing.adjacent_chars` | `Spaced` | Space between math chars: `a b c` vs `abc` |
| `math.scripts.spacing` | `Spaced` | Space around `_`/`^`: `x _ { i }` vs `x_{i}` |
| `math.scripts.order` | `SubFirst` | Fixed output order: `_` before `^` |
| `syntax.environments.name_spacing` | `Spaced` | `\begin {matrix}` vs `\begin{matrix}` |

## Transform

TeXForm includes a phase-oriented AST transform engine in `texform-transform`. It normalizes a parsed AST so downstream consumers — formula equivalence, MER tokenization, LLM pretraining corpora, or polished authoring output — can work against a stable canonical form without re-implementing LaTeX semantics per use case.

### Pipeline

The engine runs the following ordered phases. `Profile` / `BuildConfig` decide which rewrite rules are compiled into the plan, while `TransformConfig` controls per-run switches such as phase gates, FlattenGroups behavior, and max iterations:

1. **LowerAttributes (pre)** — canonicalize declarative-scope commands and registered prefix wrappers.
2. **Rewrite** — apply rewrite rules in a fixed-point loop.
3. **LowerAttributes (post)** — re-canonicalize attribute markers produced by rewrite rules.
4. **FlattenGroups** — remove redundant explicit and implicit groups after earlier phases have finished producing canonical structure.

### Rule Classes

Every transform rule belongs to exactly one **class**. The class captures the rule's intent rather than its mechanism, and consumers choose which classes to apply by selecting a profile.

| Class      | Intent |
|------------|--------|
| `Standard` | Uncontroversial author-facing standardization: legacy-syntax modernization, typo fixes, alias canonicalization, cross-package anchor unification. Does not collapse stylistic choices that an author may legitimately make. |
| `Expand`   | Corpus-oriented normal form: rewrites convenience commands, semantic macros, package-specific commands, and spacing primitives into more universal structures. Output remains readable LaTeX and preserves layout information. |
| `Drop`     | Removes non-ink, metadata, and layout hints a corpus should not learn — linebreak preferences, invisible layout nodes, and similar caller-opt-in deletions. |
| `Equiv`    | Aggressive normalization tuned for equivalence comparison; may sacrifice common idioms or author intent for higher recall. Rewrites rather than deletes. |

A rule's class is decided by its immediate rewrite intent, not by what later rules might do to the result.

### Profiles

`Profile` bundles rule classes for common downstream scenarios:

| Profile       | Classes                                  | Target scenario                                              |
|---------------|------------------------------------------|--------------------------------------------------------------|
| `Authoring`   | `Standard`                               | Polished author-facing formatting; stylistic choices kept.   |
| `Corpus`      | `Standard` + `Expand`                    | MER input or LLM pretraining corpus; layout info preserved.  |
| `CorpusDrop`  | `Standard` + `Expand` + `Drop`           | Stronger corpus cleaning; drops linebreak/layout hints.      |
| `Equiv`       | `Standard` + `Expand` + `Drop` + `Equiv` | Aggressive normalization for formula equivalence comparison. |

See `crates/texform-transform/src/rewrite/rules/README.md` for rule authoring conventions.

## Language Bindings

`texform` is the single public, stability-guaranteed Rust entry point. The facade exposes a parse-only `Parser`, an editable `Document`, a normalizing `TransformEngine`, `validate_argspec`, and analysis helpers — integrate against `texform` only.

All other crates (`texform-core`, `texform-transform`, `texform-knowledge`, ...) are internal implementation details with no stability guarantee; their APIs may change without notice. In particular, the facade's `Parser` is the stable wrapper around the lower-level `texform-core::parse::ParseContext` — depend on the wrapper, not the internals.

### Python

```bash
uv sync --dev          # set up .venv and install deps
uv run maturin develop # build from repo root
```

```python
import texform

parser = texform.Parser()
parsed = parser.parse(r"\frac{a}{b}")
document = parsed["document"]
text = document.to_latex() if document is not None else ""

engine = texform.TransformEngine(profile="authoring")
result = engine.normalize(r"a \over b")

assert text == r"\frac { a } { b }"
assert result["normalized"] == r"\frac { a } { b }"
```

### WASM / JavaScript

```bash
npm install texform
```

```ts
import { Parser, TransformEngine, validateArgspec } from "texform";

const parser = new Parser();
const parsed = parser.parse("\\frac{a}{b}");
const text = parsed.document?.toLatex() ?? "";

const engine = new TransformEngine({ profile: "authoring" });
const result = engine.normalize("a \\over b");

console.assert(text === "\\frac { a } { b }");
console.assert(result.normalized === "\\frac { a } { b }");
```

For local development, regenerate the underlying WASM package and sync it into the npm package before publishing:

```bash
bun run --cwd packages/texform prepare:publish
```

Python and JavaScript parse calls return the same shape as Rust: a `document` value plus `diagnostics`. When recovery succeeds, the partial tree is the returned document itself.

## Corpus Regression

The corpus regression suite lives in `crates/texform-regression` and reads Parquet datasets from `regression/data/`.

```bash
git lfs install && git lfs pull

cargo run --release -p texform-regression --bin parser_regression
```

Current summaries are written to `regression/results/parser_regression/summary.json`.

See [`regression/README.md`](regression/README.md) for dataset provenance and result locations.
