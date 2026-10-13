# texform

> **The LaTeX math toolkit**

JavaScript and TypeScript bindings for [TeXForm](https://texform.dev), which parses, edits, and normalizes LaTeX math using a knowledge base of command and environment specifications. Powered by WebAssembly, with TypeScript types included.

[Website](https://texform.dev) · [Documentation](https://texform.dev/docs) · [JavaScript API reference](https://texform.dev/api/js) · [Playground](https://play.texform.dev) · [Changelog](https://texform.dev/docs/changelog)

```bash
npm install texform
```

## Quick start

```ts
import { TransformEngine } from "texform";

// Normalize a formula into a canonical form chosen by profile.
const engine = new TransformEngine({ profile: "corpus" });
const normalized = engine.normalize("a \\over b");
console.assert(normalized === "\\frac { a } { b }");

// Parse through the engine, transform the live document in place, then serialize.
const parsed = engine.parse("a \\over b");
if (parsed.document) {
  engine.transform(parsed.document);
  console.assert(parsed.document.toLatex() === "\\frac { a } { b }");
}
```

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`. See [Transforms](https://texform.dev/docs/transforms) for what each profile changes.

## JavaScript-specific notes

- The JavaScript API mirrors the Rust facade one-to-one, with JavaScript naming: methods and fields are camelCase (`toLatex`, `validateArgspec` returns `argCount`), and missing values are `null`.
- `normalize` returns a string; `transform` updates the document and returns `undefined`.
- Parse, transform, normalize, and serialize take camelCase overlay objects. Unknown keys, snake_case keys, and wrong types throw `TexformConfigError` with a field path. Serializer options accept `style: "spaced" | "compact"`.
- Pass the same `KnowledgeBase` to each parser, engine, or document that should interoperate; omitting it uses a shared default instance.
- Parse and edit errors throw structured exceptions (`TexformParseError` and friends); no Rust panic ever crosses the boundary.
- TypeScript declarations are bundled — no separate `@types` package.

## Loading WebAssembly

The default `texform` import resolves to `texform/node` in Node.js and `texform/bundler` in browser-oriented bundlers. The bundler entry initializes WebAssembly at module load time and requires top-level `await` support. Use `texform/web` when initialization should wait until the API is needed, for lazy loading or Web Workers:

```ts
import { init, Parser } from "texform/web";

await init();
const parser = new Parser();
const parsed = parser.parse("x^2");
```

Both browser entries locate the `.wasm` asset with the standard `new URL(..., import.meta.url)` form, which Vite, webpack 5, and Parcel 2 support. With Vite, exclude the package from dependency prebundling so that the glue retains its asset URL:

```ts
import { defineConfig } from "vite";

export default defineConfig({
  optimizeDeps: { exclude: ["texform"] },
});
```

Rollup and esbuild require an asset plugin, or you can supply the source explicitly with `await init({ wasm })`. See [JavaScript and WebAssembly](https://texform.dev/docs/javascript-and-wasm) for entry points, bundler setup, and handle lifetimes.

## Learn more

- [Quick start](https://texform.dev/docs/quick-start) — profiles, the document tree, and parse results in ten minutes
- [Edit a document](https://texform.dev/docs/edit-a-document) — construct, edit, and locate nodes in the document tree
- [Analyze formula structure at scale](https://texform.dev/docs/analyze-structure) — export documents as columnar tables
- [Use TeXForm with MathJax or KaTeX](https://texform.dev/docs/use-with-a-renderer) — normalize formulas so a vanilla renderer can display them
- [JavaScript API reference](https://texform.dev/api/js) — every class, method, and error

## License

Apache-2.0.
