import { createRequire } from "node:module";
import { createBindings } from "../shared/create-bindings.js";

const require = createRequire(import.meta.url);
const wasm = require("../wasm/nodejs/texform_wasm.cjs");

const bindings = createBindings({
  Document: wasm.Document,
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
