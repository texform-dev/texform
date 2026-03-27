# TeXForm

A LaTeX formula parser and formatter.

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

**Arguments:**

- `<input>` — LaTeX formula to parse (required)
- `--strict true|false` — Enable strict mode (default: `false`). In strict mode, unknown commands are rejected as errors.
- `--verbose` — Print the syntax tree as pretty JSON.
- `--packages <csv>` — Load an explicit comma-separated package list. Without this flag, the example uses the runtime default packages.
- `--command <name> <kind> <mode> <spec>` — Inject a temporary command item. Repeat to inject multiple commands.
- `--environment <name> <mode> <body_mode> <spec>` — Inject a temporary environment item. Repeat as needed.
- `--delimiter <name>` — Inject a temporary delimiter control. Repeat as needed.

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

On success, the CLI prints the syntax tree with the root node's byte span. On error, it renders
diagnostics with line/column numbers and underline indicators (powered by
[ariadne](https://crates.io/crates/ariadne)). If parsing produces diagnostics but still yields a
partial result, the CLI also prints that partial tree.

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

On success, the CLI prints `valid: true`, `arg_count`, and the structured `parsed` detail.

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
