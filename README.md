# TeXForm

A LaTeX formula parser and formatter.

## Quick Start

### Parse Example

Use the built-in CLI example to parse LaTeX formulas and inspect the syntax tree:

```bash
cargo run --example parse -p texform-core -- '<input>' [--strict true|false]
```

**Arguments:**

- `<input>` — LaTeX formula to parse (required)
- `--strict true|false` — Enable strict mode (default: `false`). In strict mode, unknown commands are rejected as errors.

**Examples:**

```bash
# Parse a simple fraction
cargo run --example parse -p texform-core -- '\frac{a}{b}'

# Parse with strict mode
cargo run --example parse -p texform-core -- '\frac{a}{b}' --strict true
```

On success, the CLI prints the syntax tree with the root node's byte span. On error, it renders diagnostics with line/column numbers and underline indicators (powered by [ariadne](https://crates.io/crates/ariadne)).

### validate_spec Example

Validate an xparse-style argument spec string:

```bash
cargo run --example validate_spec -p texform-core -- '<spec>'
```

**Examples:**

```bash
cargo run --example validate_spec -p texform-core -- 'm o'
cargo run --example validate_spec -p texform-core -- 's m'
```

### parse_once_with_spec Example

One-shot parse with a temporary command/environment spec:

```bash
# command target
cargo run --example parse_once_with_spec -p texform-core -- \
  command probe prefix math 'm' '\probe{a}' --strict true

# environment target
cargo run --example parse_once_with_spec -p texform-core -- \
  environment probeenv math math '' '\begin{probeenv}a\end{probeenv}'
```

## Language Bindings

TeXForm provides Python and WASM bindings via a high-level API (`texform-core/src/api.rs`).

### Python

```bash
cd crates/texform-python
maturin develop
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

See [DEVELOP.md](DEVELOP.md) for full build instructions and maintenance notes.
