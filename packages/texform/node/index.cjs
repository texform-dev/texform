"use strict";

const wasm = require("../wasm/nodejs/texform_wasm.cjs");

exports.FlattenGroupsConfig = wasm.FlattenGroupsConfig;
exports.LowerAttributesConfig = wasm.LowerAttributesConfig;
exports.ParseContext = wasm.ParseContext;
exports.RewriteConfig = wasm.RewriteConfig;
exports.TransformConfig = wasm.TransformConfig;
exports.parse = wasm.parse;
exports.parseWithContextItems = wasm.parse_with_context_items;
exports.parse_with_context_items = wasm.parse_with_context_items;
exports.serialize = wasm.serialize;
exports.transform = wasm.transform;
exports.validateSpec = wasm.validate_spec;
exports.validate_spec = wasm.validate_spec;
