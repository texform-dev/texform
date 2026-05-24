# texform

Public npm package for TeXForm's JavaScript and TypeScript API.

```bash
npm install texform
```

```ts
import { Engine, Parser, serialize, validateArgspec } from "texform";

const parser = new Parser();
const parsed = parser.parse("\\frac{a}{b}");
const text = serialize(parsed.node);

const engine = new Engine({ profile: "authoring" });
const result = engine.normalize("a \\over b");

console.log(text);
console.log(result.normalized);
console.log(validateArgspec("m o"));
```

The package exposes Node and bundler entry points:

- `texform` resolves to a Node entry in Node.js and to a bundler entry in browser-oriented bundlers.
- `texform/node` forces the Node entry.
- `texform/bundler` forces the bundler entry.

The bundler entry initializes the WebAssembly module during module loading and
expects a modern bundler that supports top-level `await` and `.wasm` assets.

Before publishing, rebuild and sync the underlying WebAssembly bindings:

```bash
bun run prepare:publish
```
