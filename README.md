# TeXForm

A LaTeX formula parser and formatter.

## Quick Start

### Simple Parser CLI

Use the built-in CLI example to parse LaTeX formulas and inspect the syntax tree:

```bash
cargo run --example simple_parser_cli -p texform-core -- '<input>' [--strict true|false]
```

**Arguments:**

- `<input>` — LaTeX formula to parse (required)
- `--strict true|false` — Enable strict mode (default: `false`). In strict mode, unknown commands are rejected as errors.

**Examples:**

```bash
# Parse a simple fraction
cargo run --example simple_parser_cli -p texform-core -- '\frac{a}{b}'

# Parse with strict mode
cargo run --example simple_parser_cli -p texform-core -- '\frac{a}{b}' --strict true

# Parse an infix command
cargo run --example simple_parser_cli -p texform-core -- 'a \over b'
```

On success, the CLI prints the syntax tree with the root node's byte span. On error, it renders diagnostics with line/column numbers and underline indicators (powered by [ariadne](https://crates.io/crates/ariadne)).