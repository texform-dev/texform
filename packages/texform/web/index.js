import initWasm, {
  Document as WasmDocument,
  KnowledgeBase as WasmKnowledgeBase,
  Parser as WasmParser,
  TransformEngine as WasmTransformEngine,
  listPackages as wasmListPackages,
  listRules as wasmListRules,
  serialize as wasmSerialize,
  validate_argspec,
} from "../wasm/web/texform_wasm.js";
import { createBindings } from "../shared/create-bindings.js";

let initialized = false;
let initialization;

/** Initialize once; a failed attempt can be retried with a new source. */
export function init(options = {}) {
  if (!initialization) {
    initialization = Promise.resolve()
      .then(() =>
        initWasm(options.wasm === undefined ? undefined : { module_or_path: options.wasm }),
      )
      .then(() => {
        initialized = true;
      })
      .catch((error) => {
        initialization = undefined;
        throw error;
      });
  }
  return initialization;
}

const bindings = createBindings({
  isInitialized: () => initialized,
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
