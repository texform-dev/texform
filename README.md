# TeXForm

A LaTeX formula parser, formatter, and transform engine.

## Quick Start

### Parse Example

Use the built-in CLI example to parse LaTeX formulas, inspect the syntax tree, and optionally inject
custom command/environment/delimiter items into a temporary parse context:

```bash
cargo run --example parse -p texform-core -- '<input>' [--strict true|false] [--verbose]
cargo run --example parse -p texform-core -- '<input>' --command <name> <kind> <mode> <spec>
cargo run --example parse -p texform-core -- '<input>' --environment <name> <mode> <body_mode> <spec>
cargo run --example parse -p texform-core -- '<input>' --delimiter <name>
```

**Examples:**

```bash
# Parse a simple fraction
cargo run --example parse -p texform-core -- '\frac{a}{b}'

# Parse with strict mode
cargo run --example parse -p texform-core -- '\frac{a}{b}' --strict true

# Inject a temporary command
cargo run --example parse -p texform-core -- '\probe{a}' --command probe prefix math 'm'

# Inject a temporary environment
cargo run --example parse -p texform-core -- \
  '\begin{probeenv}a\end{probeenv}' \
  --environment probeenv math math ''
```

### validate_spec Example

Validate an argparse string:

```bash
cargo run --example validate_spec -p texform-core -- '<spec>'
```

## Serialization

TeXForm provides a canonical serializer that converts the AST back to LaTeX text. It is designed as
an independent stage — it covers the full AST node vocabulary and makes no assumptions about whether
the input has been normalized by a transform pass.

### Text Idempotency

The serializer guarantees **text idempotency**:

```
serialize(parse(serialize(parse(src)))) == serialize(parse(src))
```

That is, parsing the canonical output and re-serializing always produces the same string. Note that
this is a *text-level* guarantee — `parse(serialize(ast))` is not required to recover the exact
same AST kind (e.g. `Explicit` vs `Implicit` group distinctions may not round-trip).

### Default Canonical Style

The default style targets the **corpus** and **equiv** use cases (MER data processing, formula
equivalence comparison) with the following design choices:

- **Strong disambiguation** — implicit boundaries are made explicit. For example, `\frac12`
  serializes as `\frac { 1 } { 2 }`, and `x^2_i` serializes as `x _ { i } ^ { 2 }`.
- **Token-level separation in math mode** — adjacent math character atoms are space-separated
  (`abc` → `a b c`), inspired by the `im2markup` tokenization style, but without replicating its
  semantic rewriting (e.g. `\sin` is never rewritten to `\operatorname{s i n}`).
- **Text mode preserved verbatim** — text content (via `\text{...}` etc.) is never split or
  re-spaced. `\text{abc}` stays `\text {abc}`, not `\text { a b c }`.
- **Single-line output** — no formatting newlines are inserted around `\begin`/`\end` or after `\\`.

### Configurable Options

The serializer exposes `SerializeOptions` for style customization via `serialize_with(ast, &options)`.
Key axes include:

| Option | Default | Effect |
|--------|---------|--------|
| `math.spacing.commands` | `Spaced` | Space between command and `{`/`[`: `\frac { a }` vs `\frac{ a }` |
| `math.spacing.group_inner_spacing` | `Padded` | Padding inside math braces: `{ a }` vs `{a}` |
| `math.spacing.adjacent_chars` | `Spaced` | Space between math chars: `a b c` vs `abc` |
| `math.scripts.spacing` | `Spaced` | Space around `_`/`^`: `x _ { i }` vs `x_{i}` |
| `math.scripts.order` | `SubFirst` | Fixed output order: `_` before `^` |
| `syntax.environments.name_spacing` | `Spaced` | `\begin {matrix}` vs `\begin{matrix}` |


## Language Bindings

TeXForm exposes two Rust-side entry layers:

- `texform-core::context` — the stateful public API for building a parse context, injecting temporary knowledge, querying metadata, and parsing repeatedly
- `texform-core::api` — convenience helpers for one-shot parsing and batch probing on top of the default/runtime context

`texform-core::knowledge` is an internal implementation module and is not the intended public integration surface.

### Python

```bash
uv sync --dev          # set up .venv and install deps
maturin develop        # build from repo root
```

```python
import pytexform
result = pytexform.parse(r'\frac{a}{b}')  # returns dict with node + span
```

### WASM / JavaScript

```bash
wasm-pack build crates/texform-wasm --target nodejs
```

```js
const { parse } = require("./crates/texform-wasm/pkg");
const result = parse("\\frac{a}{b}"); // returns object with node + span
```

Both bindings raise/throw structured errors with `diagnostics` and `partial_result` when parsing fails.

## Corpus Bench

The corpus bench lives in `crates/texform-bench` and reads Parquet datasets from `bench/data/`.

```bash
git lfs install && git lfs pull

# run all datasets
cargo bench -p texform-bench --bench parse_corpus

# run one dataset
cargo bench -p texform-bench --bench parse_corpus -- --dataset lf80m-benchmarks

# pre-commit probe: check one dataset first, then refresh all results if it changed or is missing
cargo bench -p texform-bench --bench parse_corpus -- --dataset lf80m-benchmarks --check
```

See [`bench/README.md`](bench/README.md) for dataset provenance and result locations.
