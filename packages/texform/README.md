# texform

Public npm package for TeXForm's JavaScript and TypeScript API.

```bash
npm install texform
```

```ts
import { Document, Engine, Parser, validateArgspec } from "texform";

const parser = new Parser();
const result = parser.parse(String.raw`\frac{x}{y}`);

if (result.document) {
  console.log(result.document.toLatex());
}

const document = new Document();
const root = document.root();
const x = document.createChar("x");
document.appendChild(root, x);

console.log(document.toLatex());

const engine = new Engine({ profile: "authoring" });
const normalized = engine.normalize("a \\over b");

console.log(normalized.normalized);
console.log(validateArgspec("m o"));
```

`serialize(node, options)` remains as a compatibility helper for `SyntaxNode` snapshots. New code should use `Document.fromSyntax(node).toLatex(options)` or `document.toLatex(options)`.

The package exposes Node and bundler entry points:

- `texform` resolves to a Node entry in Node.js and to a bundler entry in browser-oriented bundlers.
- `texform/node` forces the Node entry.
- `texform/bundler` forces the bundler entry.

The bundler entry initializes the WebAssembly module during module loading and expects a modern bundler that supports top-level `await` and `.wasm` assets.

Before publishing, rebuild and sync the underlying WebAssembly bindings:

```bash
bun run prepare:publish
```
