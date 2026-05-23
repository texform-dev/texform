import { copyFile, mkdir, rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(packageRoot, "../..");

const copies = [
  ["crates/texform-wasm/pkg/texform_wasm.js", "wasm/nodejs/texform_wasm.cjs"],
  ["crates/texform-wasm/pkg/texform_wasm_bg.wasm", "wasm/nodejs/texform_wasm_bg.wasm"],
  ["crates/texform-wasm/pkg/texform_wasm.d.ts", "wasm/nodejs/texform_wasm.d.ts"],
  ["crates/texform-wasm/pkg/texform_wasm_bg.wasm.d.ts", "wasm/nodejs/texform_wasm_bg.wasm.d.ts"],
  ["crates/texform-wasm/pkg-web/texform_wasm.js", "wasm/web/texform_wasm.js"],
  ["crates/texform-wasm/pkg-web/texform_wasm_bg.wasm", "wasm/web/texform_wasm_bg.wasm"],
  ["crates/texform-wasm/pkg-web/texform_wasm.d.ts", "wasm/web/texform_wasm.d.ts"],
  ["crates/texform-wasm/pkg-web/texform_wasm_bg.wasm.d.ts", "wasm/web/texform_wasm_bg.wasm.d.ts"],
];

await rm(resolve(packageRoot, "wasm"), { recursive: true, force: true });

for (const [from, to] of copies) {
  const source = resolve(repoRoot, from);
  const target = resolve(packageRoot, to);
  await mkdir(dirname(target), { recursive: true });
  await copyFile(source, target);
}
