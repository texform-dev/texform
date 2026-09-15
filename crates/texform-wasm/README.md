# texform-wasm

wasm-bindgen bindings that back the [`texform` package on npm](https://www.npmjs.com/package/texform). Not published to crates.io.

This crate compiles to the WebAssembly module wrapped by the npm package in [`packages/texform/`](../../packages/texform/), which adds the Node/bundler dual entry points and the public TypeScript declarations. Live `Document` and `Node` handles expose the shared facade model, and errors surface as structured JavaScript exceptions. Configuration objects are camelCase overlays read through the shared `texform::bindings` path; unknown keys and type errors become `TexformConfigError`.

## Local development

Rebuild the WASM artifacts and sync them into the npm package:

```bash
bun run --cwd packages/texform prepare:publish
```

This runs `wasm-pack build` for both the `nodejs` and `web` targets and copies the output into `packages/texform/wasm/`. Run `bun install --frozen-lockfile` at the repository root first if dependencies are not installed.

## API changes and validation

Keep exported WASM shapes, `packages/texform/shared/create-bindings.js`, and `packages/texform/types/index.d.ts` consistent. Changes to shared binding DTOs or `crates/texform-interface/src/syntax_node.rs` may also change the public TypeScript surface. Update declarations and examples, rebuild both WASM targets, then run:

```bash
bun run --cwd packages/texform check
bun run --cwd packages/texform smoke:node
```

`check` compiles declarations and type tests with TypeScript; it does not execute WASM. `smoke:node` checks the real Node wrapper, including config validation, errors, and transform behavior. Use both for API or runtime changes. Host Rust tests alone do not cover the JavaScript boundary; browser-specific changes also need a browser check.
