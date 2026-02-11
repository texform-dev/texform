# Development Guide

## Prerequisites

- **Rust** (edition 2024)
- **wasm32-unknown-unknown** target: `rustup target add wasm32-unknown-unknown`
- **maturin** >= 1.9.4: `pip install "maturin>=1.9.4"`
- **wasm-pack**: `cargo install wasm-pack`

## Project Structure

```
crates/
├── texform-interface/   # Public types (SyntaxNode, etc.)
├── texform-core/        # Parser engine + high-level API (api.rs)
├── texform-specs/       # Command knowledge base (YAML, embedded at compile time)
├── texform-python/      # Python binding (PyO3 + maturin)
└── texform-wasm/        # WASM binding (wasm-bindgen + wasm-pack)
```

## Build & Test

### Rust

```bash
cargo test                # run all tests
cargo check               # check compilation only
```

### Python Binding

```bash
cd crates/texform-python
maturin develop

# verify
python -c "import pytexform; print(pytexform.parse(r'\frac{a}{b}'))"
```

### WASM Binding

```bash
# build for Node.js
wasm-pack build crates/texform-wasm --target nodejs

# build for bundlers (webpack, etc.)
wasm-pack build crates/texform-wasm --target bundler

# verify
node -e "const w = require('./crates/texform-wasm/pkg'); console.log(w.parse('\\\\frac{a}{b}'))"
```

## Embedded Resources

Command specs (`resources/specs/*.yaml`) are embedded into the binary at compile
time via `include_str!()`. The compiled `.so` (Python) and `.wasm` artifacts are
fully self-contained — no external files needed at runtime.

Changes to spec files require recompilation to take effect.

## Maintenance Notes

### TypeScript Type Declaration Sync

`crates/texform-wasm/src/lib.rs` contains a manual `typescript_custom_section`
that defines TypeScript types for `SyntaxNode` and its sub-types.

**When modifying types in `texform-interface/src/syntax_node.rs`, you must update
this section to match.**

This is a known limitation of tsify-next across crate boundaries: wasm-lld's
dead-code elimination drops `__wasm_bindgen_unstable` sections from dependency
crates when no exported function directly references their `WasmDescribe` impls.
See the comment in that file for details.

To verify after changes:

```bash
wasm-pack build crates/texform-wasm --target nodejs
cat crates/texform-wasm/pkg/texform_wasm.d.ts
```

### Feature Gate: `tsify`

`texform-interface` and `texform-core` support TypeScript type generation behind
an optional `tsify` feature. This feature is only enabled when compiling
`texform-wasm` and does not affect normal Rust builds or the Python binding.
