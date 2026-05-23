import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("../wasm/nodejs/texform_wasm.cjs");

export const Engine = wasm.Engine;
export const Parser = wasm.Parser;
export const validateArgspec = wasm.validate_argspec;
export const validate_argspec = wasm.validate_argspec;
