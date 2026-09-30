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

## Shared knowledge

`KnowledgeBase` is immutable. Package selection, custom `items`, and `removeCommands` / `removeEnvironments` / `removeDelimiterControls` belong to its constructor. Parser and engine options accept `knowledgeBase`; they no longer accept those knowledge options directly. Lookups and sorted `commands(mode)`, `environments(mode)`, `characters(mode)`, `delimiters()`, and `packages()` are methods on `KnowledgeBase`.

```ts
import { KnowledgeBase, Parser, TransformEngine, Document } from "texform";

const knowledgeBase = new KnowledgeBase({ packages: ["base", "ams"] });
const parser = new Parser({ knowledgeBase });
const engine = new TransformEngine({ profile: "corpus", knowledgeBase });
const document = parser.parse("x").document;
const variant = document.clone();
engine.transform(variant);
console.assert(variant.knowledgeBase().isSame(knowledgeBase));
console.assert(document.root().isSameNode(document.root()));
const restored = Document.fromSyntax(document.toSyntax(), { knowledgeBase });
engine.transform(restored);
```

All parsers, engines, and documents with omitted `knowledgeBase` share the default instance. Separately constructed knowledge bases have different identities even with the same options. Transform accepts complete documents sharing its knowledge-base instance, including constructed, restored, and cloned documents. `node.document()` returns a handle to its owning document; cloning a document gives its nodes a new identity while retaining the knowledge base. `new Document({ knowledgeBase, mode: "text" })` creates an empty text-mode document.

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

## Constructing documents

Constructors check the document's knowledge base before changing the tree. Arguments use live `Node` handles, source strings, booleans for star slots, or `null` for absent slots. Supply every argument slot, or only required slots to omit optional and star slots. Paired arguments accept `{ value, open, close }`; delimiters are strings such as `"("`, `"\\rangle"`, and `"."` (invisible).

```js
const doc = new Document();
const sqrt = doc.createCommand("sqrt", [null, "x"]);
const scripted = doc.createScripted(sqrt, "i", "2");
const group = doc.createDelimitedGroup("(", ")", [scripted]);
doc.appendChild(doc.root(), group);
console.log(doc.toLatex());
```

`createGroup`, `createDelimitedGroup`, and `createInlineMath` accept arrays of nodes or source strings; a string is parsed and all its nodes are spliced into the list, so one string may yield several children. `createEnvironment` accepts a body node, source string, array of nodes or source strings, or `null` for an empty body. Other constructors include `createPrime`, `createInfix`, and `createDeclarative`. A final `{ mode: "math" | "text" }` option selects the context of a detached subtree. It defaults to text for `createInlineMath`; math for `createDelimitedGroup`, `createScripted`, `createPrime`, and `createInfix`; the group's own mode for `createGroup`; and the document's root mode otherwise. `parseFragment(source, options)` returns a detached implicit group using the same knowledge base.

Invalid shapes throw `TexformConformanceError`, a `TexformEditError` subclass with `path` and `rule`. Invalid source arguments throw `TexformParseError` with diagnostics. `Document.fromSyntax` validates error-free trees against its knowledge base; trees containing error nodes remain read-only. Failed operations leave the document unchanged. `toLatex` and `serialize` check only structure, not conformance to a knowledge base.

## Editing and addressing

`node.path()` returns its current rooted path, or `null` for detached nodes. `document.nodeAt(path)` resolves a path and throws if it is missing. `node.slot()` describes its parent slot using a snake_case `kind` and a nullable `index`; roots and detached nodes return `null`. Paths can change after edits, so save them before modifying the tree.

```js
const original = parser.parse("x_i").document;
const base = original.root().children[0].scriptBase();
const variant = original.clone();
variant.setSuperscript(variant.nodeAt(base.path()), "2");
```

`setSubscript` and `setSuperscript` accept a node or source string and return the resulting wrapper. Omit the value, or pass `undefined` or `null`, to clear a script; clearing the last script returns the collapsed base. Passing an existing script base edits its parent wrapper. `cloneNode(node)` copies a subtree within a document; `importNode(node)` copies from the node's owning document and checks the destination knowledge base. Both return detached nodes.

Use `setArgDelimiters` for a Paired argument's boundaries, `setDelimiters` for a delimited group, and `setPrimeCount` for a positive prime count. `setArg` preserves existing Paired boundaries when given an ordinary value. `node.argKind(index)` exposes the argument form (also available as `node.arg(index).form`), and `node.isKnown()` returns a boolean for commands and environments or `null` otherwise.

## License

Apache-2.0.
