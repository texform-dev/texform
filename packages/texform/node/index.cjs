"use strict";

const wasm = require("../wasm/nodejs/texform_wasm.cjs");

class TexformParseError extends Error {
  constructor(payload) {
    const diagnostics = payload?.diagnostics ?? [];
    super(diagnostics[0]?.message ?? "parse failed");
    this.name = "TexformParseError";
    this.diagnostics = diagnostics;
    this.partialResult = payload?.partialResult ?? null;
  }
}

function wrapParseError(callback) {
  try {
    return callback();
  } catch (error) {
    if (error && typeof error === "object" && "diagnostics" in error) {
      throw new TexformParseError(error);
    }
    throw error;
  }
}

function present(value) {
  return value == null ? undefined : value;
}

class Parser {
  constructor(options) {
    this.inner = new wasm.Parser(options ?? undefined);
  }
  free() {
    this.inner.free();
  }
  [Symbol.dispose]() {
    this.free();
  }
  parse(src, options) {
    return wrapParseError(() => this.inner.parse(src, options ?? undefined));
  }
  lookupCommand(name, mode) {
    return present(this.inner.lookup_command(name, mode));
  }
  lookupExplicitCommand(name, mode) {
    return present(this.inner.lookup_explicit_command(name, mode));
  }
  lookupCharacter(name, mode) {
    return present(this.inner.lookup_character(name, mode));
  }
  lookupEnv(name, mode) {
    return present(this.inner.lookup_env(name, mode));
  }
  isDelimiterControl(name) {
    return this.inner.is_delimiter_control(name);
  }
  knowsCommandName(name) {
    return this.inner.knows_command_name(name);
  }
  knowsEnvName(name) {
    return this.inner.knows_env_name(name);
  }
  knowsCharacterName(name) {
    return this.inner.knows_character_name(name);
  }
}

class Engine {
  constructor(options) {
    this.inner = new wasm.Engine(options);
  }
  free() {
    this.inner.free();
  }
  [Symbol.dispose]() {
    this.free();
  }
  parse(src, options) {
    return wrapParseError(() => this.inner.parse(src, options ?? undefined));
  }
  normalize(src, options) {
    return this.inner.normalize(src, options ?? undefined);
  }
  lookupCommand(name, mode) {
    return present(this.inner.lookup_command(name, mode));
  }
  lookupExplicitCommand(name, mode) {
    return present(this.inner.lookup_explicit_command(name, mode));
  }
  lookupCharacter(name, mode) {
    return present(this.inner.lookup_character(name, mode));
  }
  lookupEnv(name, mode) {
    return present(this.inner.lookup_env(name, mode));
  }
  isDelimiterControl(name) {
    return this.inner.is_delimiter_control(name);
  }
  knowsCommandName(name) {
    return this.inner.knows_command_name(name);
  }
  knowsEnvName(name) {
    return this.inner.knows_env_name(name);
  }
  knowsCharacterName(name) {
    return this.inner.knows_character_name(name);
  }
}

exports.Engine = Engine;
exports.Parser = Parser;
exports.TexformParseError = TexformParseError;
exports.serialize = wasm.serialize;
exports.validateArgspec = wasm.validate_argspec;
