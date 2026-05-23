import init, {
  Engine,
  Parser,
  validate_argspec,
} from "../wasm/web/texform_wasm.js";
import wasmUrl from "../wasm/web/texform_wasm_bg.wasm";

await init({ module_or_path: new URL(wasmUrl, import.meta.url) });

export {
  Engine,
  Parser,
  validate_argspec,
};

export { validate_argspec as validateArgspec };
