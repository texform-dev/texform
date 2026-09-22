# texform

JavaScript and TypeScript bindings for [TeXForm](../../README.md), a LaTeX formula parser, editor, and normalizer built on a structured command knowledge base. Powered by WebAssembly, with TypeScript types included.

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

Profiles select the normalization target: `"authoring"`, `"faithful"`, `"corpus"`, and `"equiv"`.

## JavaScript-specific notes

- `normalize` returns a string. `transform` updates the document and returns `undefined`.
- `normalizeWithReport` returns `{ normalized, report }`. `transformWithReport` returns the report object. Both accept the same camelCase overlays as the plain methods. Output text and errors follow the ordinary transform contract. Report fields, phase divisions, and counters are diagnostic and are not a stable compatibility promise.
- The package ships two entry points for loading the WebAssembly module. The default `texform` import resolves to the Node entry in Node.js and to the bundler entry in browser-oriented bundlers; `texform/node` and `texform/bundler` force one explicitly.
- The bundler entry initializes the WebAssembly module at module load time and expects a modern bundler with support for top-level `await` and `.wasm` assets (e.g. Vite, webpack 5).
- All names follow JavaScript conventions: methods and fields are camelCase (`toLatex`, `validateArgspec` returns `argCount`), and missing values are `null`.
- Parse, transform, normalize, and serialize take camelCase overlay objects. `null` / `undefined` / omitted means not set. Unknown keys, snake_case keys, arrays in object positions, and wrong scalar types throw `TexformConfigError` with a field path. Enum string values stay snake_case (`"sub_first"`).
- `parser.defaultParseConfig()`, `engine.defaultParseConfig()`, and `engine.defaultTransformConfig()` return the complete defaults actually in force.
- Parse and edit errors throw structured exceptions (`TexformParseError` and friends); no Rust panic ever crosses the boundary.
- TypeScript declarations are bundled — no separate `@types` package.

## Learn more

The JavaScript API mirrors the Rust facade one-to-one. For the full picture — the editable document tree, transform profiles, and the architecture — see the [repository README](../../README.md).

## License

Apache-2.0.
