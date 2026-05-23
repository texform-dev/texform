"use strict";

const wasm = require("../wasm/nodejs/texform_wasm.cjs");

exports.Engine = wasm.Engine;
exports.Parser = wasm.Parser;
exports.validateArgspec = wasm.validate_argspec;
exports.validate_argspec = wasm.validate_argspec;
