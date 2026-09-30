import { createRequire } from "node:module";
import { createBindings } from "../shared/create-bindings.js";

const require = createRequire(import.meta.url);
const wasm = require("../wasm/nodejs/texform_wasm.cjs");

const bindings = createBindings({
  Document: wasm.Document,
  KnowledgeBase: wasm.KnowledgeBase,
  Parser: wasm.Parser,
  TransformEngine: wasm.TransformEngine,
  serialize: wasm.serialize,
  validateArgspec: wasm.validate_argspec,
  listPackages: wasm.listPackages,
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
} = bindings;
