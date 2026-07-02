import init, {
  Document as WasmDocument,
  Parser as WasmParser,
  TransformEngine as WasmTransformEngine,
  listPackages as wasmListPackages,
  serialize as wasmSerialize,
  validate_argspec,
} from "../wasm/web/texform_wasm.js";
import wasmUrl from "../wasm/web/texform_wasm_bg.wasm?url";
import { createBindings } from "../shared/create-bindings.js";

await init({ module_or_path: new URL(wasmUrl, import.meta.url) });

const bindings = createBindings({
  Document: WasmDocument,
  Parser: WasmParser,
  TransformEngine: WasmTransformEngine,
  serialize: wasmSerialize,
  validateArgspec: validate_argspec,
  listPackages: wasmListPackages,
});

export const {
  TexformError,
  TexformParseError,
  TexformEditError,
  TexformConfigError,
  TexformTransformError,
  Parser,
  TransformEngine,
  Document,
  Node,
  serialize,
  validateArgspec,
  listPackages,
} = bindings;
