# texform-wasm

wasm-bindgen bindings that back the [`texform` package on npm](https://www.npmjs.com/package/texform). Not published to crates.io.

This crate compiles to the WebAssembly module wrapped by the npm package in [`packages/texform/`](../../packages/texform/), which adds the Node/bundler dual entry points and the public TypeScript declarations. Bindings layer strictly on top of the `texform` facade: live `Document` and `Node` handles delegate to the shared Rust core, and errors surface as structured JavaScript exceptions.

## Local development

Rebuild the WASM artifacts and sync them into the npm package:

```bash
bun run --cwd packages/texform prepare:publish
```

This runs `wasm-pack build` for both the `nodejs` and `web` targets and copies the output into `packages/texform/wasm/`. Verify the TypeScript surface afterwards with `bun run --cwd packages/texform check`.
