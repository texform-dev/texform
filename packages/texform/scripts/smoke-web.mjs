import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import * as web from "texform/web";

for (const call of [
  () => new web.KnowledgeBase(),
  () => new web.Parser(),
  () => new web.TransformEngine(),
  () => new web.Document(),
  () => web.Document.fromSyntax({}),
  () => web.serialize({}),
  () => web.validateArgspec("m"),
  () => web.listPackages(),
  () => web.listRules(),
]) {
  assert.throws(call, (error) => error instanceof web.TexformError && /await init\(\)/.test(error.message));
}

const failure = web.init({ wasm: new Uint8Array([0]) });
assert.equal(web.init(), failure);
await assert.rejects(failure);
assert.throws(() => new web.Parser(), web.TexformError);

const bytes = await readFile(new URL("../wasm/web/texform_wasm_bg.wasm", import.meta.url));
const ready = web.init({ wasm: bytes });
assert.equal(web.init(), ready);
await ready;
assert.equal(web.init(), ready);
const parser = new web.Parser();
const parsed = parser.parse(String.raw`\frac{a}{b}`);
assert.equal(parsed.document.toLatex(), String.raw`\frac { a } { b }`);
assert.equal(web.validateArgspec("m").argCount, 1);
assert.ok(web.listPackages().length > 0);
assert.ok(web.listRules().length > 0);
parsed.document.free();
parser.free();
console.log("web smoke passed");
