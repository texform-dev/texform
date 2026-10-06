import init, {
  Document as WasmDocument,
  KnowledgeBase as WasmKnowledgeBase,
  Parser as WasmParser,
  TransformEngine as WasmTransformEngine,
  listPackages as wasmListPackages,
  listRules as wasmListRules,
  serialize as wasmSerialize,
  validate_argspec,
} from "../wasm/web/texform_wasm.js";
import wasmUrl from "../wasm/web/texform_wasm_bg.wasm?url";
import { createBindings } from "../shared/create-bindings.js";

await init({ module_or_path: new URL(wasmUrl, import.meta.url) });

const bindings = createBindings({
  Document: WasmDocument,
  KnowledgeBase: WasmKnowledgeBase,
  Parser: WasmParser,
  TransformEngine: WasmTransformEngine,
  serialize: wasmSerialize,
  validateArgspec: validate_argspec,
  listPackages: wasmListPackages,
  listRules: wasmListRules,
});

export const {
  TexformError,
  TexformParseError,
  TexformEditError,
  TexformConformanceError,
  TexformConfigError,
  TexformTransformError,
  KnowledgeBase,
  Parser,
  TransformEngine,
  Document,
  Node,
  serialize,
  validateArgspec,
  listPackages,
  listRules,
} = bindings;
