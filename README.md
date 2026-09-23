<div align="center">

# TeXForm

**The missing foundation for LaTeX formula processing.**

TeXForm parses, edits, and transforms LaTeX math, built on a structured knowledge base of 600+ command and environment specifications across 7 LaTeX packages, validated against MathJax, KaTeX, and XeTeX. One Rust core, available in Rust, Python, and JavaScript.

[![crates.io](https://img.shields.io/crates/v/texform.svg)](https://crates.io/crates/texform)
[![PyPI](https://img.shields.io/pypi/v/texform.svg)](https://pypi.org/project/texform/)
[![npm](https://img.shields.io/npm/v/texform.svg)](https://www.npmjs.com/package/texform)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

[Playground](https://play.texform.dev) · [Architecture](ARCHITECTURE.md) · [Changelog](CHANGELOG.md)

</div>

## Why TeXForm

Programs that process LaTeX formulas — cleaning OCR training data, comparing formulas for equivalence, preparing pretraining corpora, building math editors — keep reinventing the same fragile layer: regular expressions and one-off heuristics that guess what `\frac`, `\over`, or `\quantity` mean. The hard part was never the string manipulation. It is knowing the argument structure of every command well enough to work on formulas as syntax trees instead of text.

TeXForm is that missing layer. It parses formulas against a real command knowledge base, gives you an editable document tree, and normalizes formulas into canonical forms tuned for specific downstream uses.

## What TeXForm gives you

<table>
<tr>
<th align="center"><a href="ARCHITECTURE.md#knowledge-and-argument-specifications">Knowledge-driven parser</a></th>
<th align="center"><a href="ARCHITECTURE.md#editing-model">Editable document tree</a></th>
<th align="center"><a href="crates/texform-transform/README.md">Profile-based transform engine</a></th>
</tr>
<tr>
<td valign="top">

Formulas parse into real syntax trees, driven by xparse-style argument specifications for every command in `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, and `textmacros`. Unknown commands and unparseable fragments survive as explicit nodes — under your control, never a crash.

</td>
<td valign="top">

Parsing produces a `Document` — a DOM-style tree you can query, mutate, and serialize back to LaTeX. Edits are validated and return structured errors; no panic ever reaches the caller. The canonical serializer guarantees text idempotency.

</td>
<td valign="top">

Normalization has no single correct answer, so TeXForm never imposes one true form. You pick a `Profile`, and a curated rule pipeline produces the canonical form for that use case — from polished authoring output to aggressive corpus normalization.

</td>
</tr>
</table>

## Normalization at a glance

Each `Profile` targets one downstream scenario:

| Profile | Target scenario |
| --- | --- |
| `Authoring` | Polished author-facing output; stylistic choices kept. |
| `Faithful` | Same rendered formula; convenience macros expanded into universal forms. |
| `Corpus` | Complete canonical labels for training data; reading-preserving layout variants may collapse. |
| `Equiv` | An aggressive intermediate for equivalence comparison, deduplication, or fingerprints. |

The same input, normalized under different profiles (real engine output):

| Input | `Authoring` | `Corpus` | `Corpus`, rendered |
| --- | --- | --- | --- |
| `a \over b` | `\frac { a } { b }` | `\frac { a } { b }` | $\frac { a } { b }$ |
| `\substack{a \\ b}` | `\substack { a \\ b }` | `\begin {subarray} {c} a \\ b \end {subarray}` | $\begin {subarray} {c} a \\ b \end {subarray}$ |
| `\dv{f}{x}` | `\dv { f } { x }` | `\frac { \mathrm { d } f } { \mathrm { d } x }` | $\frac { \mathrm { d } f } { \mathrm { d } x }$ |
| `\ket{\psi}` | `\ket { \psi }` | `\left \vert \psi \right \rangle` | $\left \vert \psi \right \rangle$ |

Every profile modernizes legacy syntax like `\over`. `Authoring` keeps the author's shorthand; `Corpus` expands it into universal forms that render on any vanilla MathJax or KaTeX deployment with no extra packages — including right here on GitHub, where inputs like `\dv` and `\ket` would not render at all.

`Equiv` adds rules that may discard visually salient notation choices. For example, it rewrites centered `\cfrac` forms to `\frac`, while `Corpus` retains continued-fraction styling. Use `Equiv` output as an intermediate for comparison, deduplication, or fingerprints; it is not intended as a training label for the original image.

## What's underneath

Behind a profile, normalization runs as a multi-phase pipeline, not a find-and-replace pass:

- **Rewrite** applies a curated rule set in a fixed-point loop — modernizing legacy syntax, canonicalizing aliases, and expanding semantic macros, depending on the levels the profile selects.
- **LowerAttributes** canonicalizes font and style markup, so `{\bf x}` and `\mathbf{x}` converge to a single form with declarative scope tracked correctly.
- **FlattenGroups** strips redundant braces behind semantic and spacing guards, so flattening never changes script binding, environment cell boundaries, or atom spacing unless you opt in.
- **FinalizeAst** canonicalizes the tree representation by merging adjacent prime nodes and normalizing text sequences. It runs before FlattenGroups and again after flattening to handle newly adjacent nodes.

The execution order is LowerAttributes → Rewrite → LowerAttributes → FinalizeAst → FlattenGroups → FinalizeAst. Per-run options can disable phases; the second FinalizeAst pass runs only when FlattenGroups is enabled. Eliminated-form contracts are checked after the full pipeline.

Two assets carry most of the weight, and neither existed as a reusable artifact before:

- **The argspec knowledge base.** Machine-readable, xparse-style argument signatures for every supported command and environment — validated by rendering against MathJax, KaTeX, and XeTeX rather than transcribed from documentation.
- **The curated rewrite rule set.** Every rule declares its `RuleLevel`, its worst-case `RuleFidelity`, and the forms it eliminates. The engine enforces that eliminated-form contract after every run, and corpus regression re-checks it against real-world datasets.

## Quick start

### Python

```bash
pip install texform
```

```python
import texform

engine = texform.TransformEngine(profile="corpus")
assert engine.normalize(r"a \over b") == r"\frac { a } { b }"

parsed = engine.parse(r"a \over b")
if parsed["document"] is not None:
    document = parsed["document"]
    engine.transform(document)
    assert document.to_latex() == r"\frac { a } { b }"
```

### JavaScript / TypeScript

```bash
npm install texform
```

```ts
import { TransformEngine } from "texform";

const engine = new TransformEngine({ profile: "corpus" });
console.assert(engine.normalize("a \\over b") === "\\frac { a } { b }");

const parsed = engine.parse("a \\over b");
if (parsed.document) {
  engine.transform(parsed.document);
  console.assert(parsed.document.toLatex() === "\\frac { a } { b }");
}
```

### Rust

```bash
cargo add texform
```

```rust
use texform::{Profile, TransformEngine};

let engine = TransformEngine::builder().profile(Profile::Corpus).build()?;
let normalized = engine.normalize(r"a \over b")?;
assert_eq!(normalized, r"\frac { a } { b }");

let (mut document, _) = engine.parser().parse(r"a \over b").try_into_document()?;
engine.transform(&mut document)?;
assert_eq!(document.to_latex()?, r"\frac { a } { b }");
```

> **Note:** the `texform` crate is the only public, stability-guaranteed Rust entry point; the `texform-*` crates it depends on are internal and may change in any release. See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the crate layout and the public API guarantees.

The Python and JavaScript bindings expose the same parser, document, and engine from the single Rust core. See the [PyPI package notes](python/texform/README.md) and the [npm package notes](packages/texform/README.md) for language-specific details.

### Command line

```bash
cargo install texform-cli
```

```console
$ texform normalize --profile corpus 'a \over b'
\frac { a } { b }
```

The `texform` binary also parses, tokenizes, and queries the knowledge base, and `texform serve` offers normalization to other programs over a JSON-RPC protocol. See the [CLI guide](crates/texform-cli/README.md).

## Links

- [Playground](https://play.texform.dev) — try TeXForm in the browser
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate layout, pipeline, tree representations, API guarantees
- [`TESTING.md`](TESTING.md) — how TeXForm is tested, from contract tests to corpus regression
- [`texform-cli`](crates/texform-cli/README.md) — command-line usage and the `serve` normalizer protocol
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## License

Apache-2.0. See [LICENSE](LICENSE).
