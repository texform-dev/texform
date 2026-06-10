import type { ParsedSlot } from "./types.js";

let wasm: any = null;

function getWasm() {
  if (!wasm) {
    const wasmPath = new URL(
      "../../../crates/texform-wasm/pkg/texform_wasm.js",
      import.meta.url,
    );
    wasm = require(wasmPath.pathname);
  }
  return wasm;
}

export function parseArgSpec(spec: string): ParsedSlot[] | null {
  if (!spec || spec.trim() === "") return [];
  const result = getWasm().validate_argspec(spec);
  if (!result.valid) return null;
  if (typeof result.argCount !== "number") return null;
  return result.parsed as ParsedSlot[];
}
