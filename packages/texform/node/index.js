import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("../wasm/nodejs/texform_wasm.cjs");

export const FlattenGroupsConfig = wasm.FlattenGroupsConfig;
export const LowerAttributesConfig = wasm.LowerAttributesConfig;
export const ParseContext = wasm.ParseContext;
export const RewriteConfig = wasm.RewriteConfig;
export const TransformConfig = wasm.TransformConfig;
export const parse = wasm.parse;
export const parseWithContextItems = wasm.parse_with_context_items;
export const parse_with_context_items = wasm.parse_with_context_items;
export const serialize = wasm.serialize;
export const transform = wasm.transform;
export const validateSpec = wasm.validate_spec;
export const validate_spec = wasm.validate_spec;
