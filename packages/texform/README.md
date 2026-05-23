# texform

Public npm package for TeXForm's JavaScript and TypeScript API.

```bash
npm install texform
```

```ts
import { Parser } from "texform";

const parser = new Parser();
const result = parser.parse("\\frac{a}{b}");
console.log(result.node);
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
