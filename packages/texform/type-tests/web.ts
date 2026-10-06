import { init, Parser, Document, listRules, type ParseResult } from "texform/web";
import * as automatic from "texform";

const ready: Promise<void> = init();
void ready;
void init({ wasm: new Uint8Array([0]) });
void init({ wasm: new ArrayBuffer(8) });
void init({ wasm: new WebAssembly.Module(new Uint8Array()) });
void init({ wasm: new Response() });
void init({ wasm: Promise.resolve(new Response()) });
void init({ wasm: new URL("https://example.com/texform.wasm") });
void init({ wasm: "/texform.wasm" });
const result: ParseResult = new Parser().parse("x");
const document: Document | null = result.document;
void document;
void listRules();
// @ts-expect-error init belongs to the explicit web entry, not the default types.
void automatic.init;
// @ts-expect-error callers use the public camelCase options rather than wasm-bindgen internals.
void init({ module_or_path: new Uint8Array() });
