<div align="center">

# TeXForm

**The LaTeX math toolkit**

TeXForm parses, edits, and normalizes LaTeX math using a knowledge base of 600+ command and environment specifications across 8 LaTeX packages, validated against MathJax, KaTeX, and XeTeX. One Rust core backs packages for Python, JavaScript, and Rust, plus a command-line tool.

[![crates.io](https://img.shields.io/crates/v/texform.svg)](https://crates.io/crates/texform)
[![PyPI](https://img.shields.io/pypi/v/texform.svg)](https://pypi.org/project/texform/)
[![npm](https://img.shields.io/npm/v/texform.svg)](https://www.npmjs.com/package/texform)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

[Website](https://texform.dev) · [Documentation](https://texform.dev/docs) · [Playground](https://play.texform.dev) · [LaTeX reference](https://texform.dev/latex) · [Changelog](CHANGELOG.md)

</div>

## Why TeXForm

Programs that process LaTeX formulas — cleaning OCR and math-recognition output, preparing training corpora, comparing and deduplicating formulas, building math editors — keep reinventing the same fragile layer: regular expressions and one-off heuristics that guess what `\frac`, `\over`, or `\quantity` mean. The hard part is not the string manipulation. It is knowing the argument structure of every command well enough to work on formulas as trees instead of text.

TeXForm provides that layer:

- **Parse against a knowledge base.** Formulas parse into syntax trees driven by xparse-style argument specifications for the commands and environments in `base`, `ams`, `physics`, `braket`, `bboldx`, `boldsymbol`, `textmacros`, and `color`. Unknown commands and unparseable fragments stay in the tree as explicit nodes with diagnostics.
- **Edit a document tree.** A parsed `Document` is a DOM-style tree you can query, edit, and serialize back to LaTeX. Edits are validated against the knowledge base and fail with structured errors, and serialization is text-idempotent.
- **Normalize with a profile.** Normalization has no single correct answer, so you pick a `Profile` for your downstream use, and a curated set of rewrite rules produces the canonical form for it. Every rule declares the forms it eliminates, and the engine checks that contract after every run.

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

`Equiv` adds rules that may discard visually salient notation choices, such as rewriting centered `\cfrac` forms to `\frac`. Use its output for comparison, deduplication, or fingerprints, not as a training label for the original image. See [Transforms](https://texform.dev/docs/transforms) for what each profile changes and the [rule reference](https://texform.dev/rules) for every rewrite rule.

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

The `texform` crate is the only public, stability-guaranteed Rust entry point; the `texform-*` crates it depends on are internal and may change in any release.

### Command line

```bash
cargo install texform-cli
```

```console
$ texform normalize --profile corpus 'a \over b'
\frac { a } { b }
```

The `texform` binary also parses, tokenizes, and queries the knowledge base, and `texform serve` offers normalization to other programs over a JSON-RPC protocol. See the [CLI guide](crates/texform-cli/README.md).

Output uses the byte-stable `spaced` style by default. For concise output such as `\frac{a}{b}+x_i^2`, select the `compact` style; see [Configure output style](https://texform.dev/docs/configure-output-style).

## Documentation

The documentation lives at [texform.dev](https://texform.dev):

- [Get started](https://texform.dev/docs/installation) — installation, quick start, JavaScript and WebAssembly setup, and the command line
- [Guides](https://texform.dev/docs/normalize-a-corpus) — normalizing a corpus, cleaning OCR output, comparing formulas, editing documents, and more
- [Concepts](https://texform.dev/docs/what-is-texform) — parsing, transforms, serialization, and how TeXForm compares with other tools
- [API reference](https://texform.dev/api) — Python, JavaScript, and Rust ([docs.rs](https://docs.rs/texform))
- [LaTeX reference](https://texform.dev/latex) — every supported command and environment, with its argument structure and renderer support
- [Rule reference](https://texform.dev/rules) — every rewrite rule with its level, fidelity, and examples
- [Playground](https://play.texform.dev) — try TeXForm in the browser

For contributors, this repository documents the internals:

- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate layout, pipeline, tree representations, and API guarantees
- [`TESTING.md`](TESTING.md) — how TeXForm is tested, from contract tests to corpus regression
- [`CHANGELOG.md`](CHANGELOG.md) — release history

## License

Apache-2.0. See [LICENSE](LICENSE).
