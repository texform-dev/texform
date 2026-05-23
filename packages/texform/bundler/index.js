import init, {
  FlattenGroupsConfig,
  LowerAttributesConfig,
  ParseContext,
  RewriteConfig,
  TransformConfig,
  parse,
  parse_with_context_items,
  serialize,
  transform,
  validate_spec,
} from "../wasm/web/texform_wasm.js";
import wasmUrl from "../wasm/web/texform_wasm_bg.wasm";

await init({ module_or_path: new URL(wasmUrl, import.meta.url) });

export {
  FlattenGroupsConfig,
  LowerAttributesConfig,
  ParseContext,
  RewriteConfig,
  TransformConfig,
  parse,
  parse_with_context_items,
  serialize,
  transform,
  validate_spec,
};

export { parse_with_context_items as parseWithContextItems };
export { validate_spec as validateSpec };
