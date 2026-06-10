import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const wasm = require("../wasm/nodejs/texform_wasm.cjs");

export class TexformError extends Error {
  constructor(payload, fallback = "texform error") {
    super(payload?.message ?? fallback);
    this.name = "TexformError";
    this.kind = payload?.kind ?? "internal";
  }
}

export class TexformParseError extends TexformError {
  constructor(payload) {
    super(payload, "parse failed");
    this.name = "TexformParseError";
    this.diagnostics = payload?.diagnostics ?? [];
    this.document = payload?.document ? new Document(payload.document) : null;
  }
}

export class TexformEditError extends TexformError {
  constructor(payload) {
    super(payload, "edit failed");
    this.name = "TexformEditError";
  }
}

export class TexformConfigError extends TexformError {
  constructor(payload) {
    super(payload, "invalid texform configuration");
    this.name = "TexformConfigError";
  }
}

export class TexformTransformError extends TexformError {
  constructor(payload) {
    super(payload, "transform failed");
    this.name = "TexformTransformError";
  }
}

function wrapTexformError(callback) {
  try {
    return callback();
  } catch (error) {
    if (error && typeof error === "object" && typeof error.kind === "string") {
      if (error.kind === "parse") {
        throw new TexformParseError(error);
      }
      if (error.kind === "edit") {
        throw new TexformEditError(error);
      }
      if (error.kind === "config") {
        throw new TexformConfigError(error);
      }
      if (error.kind === "transform") {
        throw new TexformTransformError(error);
      }
      throw new TexformError(error);
    }
    throw error;
  }
}

function wrapParseResult(result) {
  return {
    ...result,
    document: result.document ? new Document(result.document) : null,
  };
}

export class Parser {
  constructor(options) {
    this.inner = wrapTexformError(() => new wasm.Parser(options ?? undefined));
  }

  free() {
    this.inner.free();
  }

  [Symbol.dispose]() {
    this.free();
  }

  parse(src, options) {
    return wrapTexformError(() =>
      wrapParseResult(this.inner.parse(src, options ?? undefined)),
    );
  }

  lookupCommand(name, mode) {
    return this.inner.lookup_command(name, mode);
  }

  lookupExplicitCommand(name, mode) {
    return this.inner.lookup_explicit_command(name, mode);
  }

  lookupCharacter(name, mode) {
    return this.inner.lookup_character(name, mode);
  }

  lookupEnv(name, mode) {
    return this.inner.lookup_env(name, mode);
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

export class TransformEngine {
  constructor(options) {
    this.inner = wrapTexformError(() => new wasm.TransformEngine(options));
  }

  free() {
    this.inner.free();
  }

  [Symbol.dispose]() {
    this.free();
  }

  parse(src, options) {
    return wrapTexformError(() =>
      wrapParseResult(this.inner.parse(src, options ?? undefined)),
    );
  }

  normalize(src, options) {
    return wrapTexformError(() => this.inner.normalize(src, options ?? undefined));
  }

  lookupCommand(name, mode) {
    return this.inner.lookup_command(name, mode);
  }

  lookupExplicitCommand(name, mode) {
    return this.inner.lookup_explicit_command(name, mode);
  }

  lookupCharacter(name, mode) {
    return this.inner.lookup_character(name, mode);
  }

  lookupEnv(name, mode) {
    return this.inner.lookup_env(name, mode);
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

export class Document {
  constructor(inner) {
    this.inner = inner ?? new wasm.Document();
  }

  static fromSyntax(node) {
    return wrapTexformError(() => new Document(wasm.Document.fromSyntax(node)));
  }

  free() {
    this.inner.free();
  }

  [Symbol.dispose]() {
    this.free();
  }

  root() {
    return wrapTexformError(() => this.inner.root());
  }

  hasErrors() {
    return wrapTexformError(() => this.inner.hasErrors());
  }

  isReadOnly() {
    return wrapTexformError(() => this.inner.isReadOnly());
  }

  errors() {
    return wrapTexformError(() => this.inner.errors());
  }

  findCommands(name) {
    return wrapTexformError(() => this.inner.findCommands(name));
  }

  findEnvironments(name) {
    return wrapTexformError(() => this.inner.findEnvironments(name));
  }

  createChar(value) {
    return wrapTexformError(() => this.inner.createChar(value));
  }

  createText(value) {
    return wrapTexformError(() => this.inner.createText(value));
  }

  createActiveSpace() {
    return wrapTexformError(() => this.inner.createActiveSpace());
  }

  createGroup(mode) {
    return wrapTexformError(() => this.inner.createGroup(mode));
  }

  createCommand(name, args) {
    return wrapTexformError(() => this.inner.createCommand(name, args ?? undefined));
  }

  createDeclarative(name, args) {
    return wrapTexformError(() => this.inner.createDeclarative(name, args ?? undefined));
  }

  createEnvironment(name, args, body) {
    return wrapTexformError(() =>
      this.inner.createEnvironment(name, args ?? undefined, body),
    );
  }

  appendChild(parent, child) {
    return wrapTexformError(() => this.inner.appendChild(parent, child));
  }

  insertChild(parent, index, child) {
    return wrapTexformError(() => this.inner.insertChild(parent, index, child));
  }

  insertBefore(anchor, node) {
    return wrapTexformError(() => this.inner.insertBefore(anchor, node));
  }

  insertAfter(anchor, node) {
    return wrapTexformError(() => this.inner.insertAfter(anchor, node));
  }

  replaceWith(target, replacement) {
    return wrapTexformError(() => this.inner.replaceWith(target, replacement));
  }

  wrap(target, wrapper) {
    return wrapTexformError(() => this.inner.wrap(target, wrapper));
  }

  unwrap(group) {
    return wrapTexformError(() => this.inner.unwrap(group));
  }

  extract(node) {
    return wrapTexformError(() => this.inner.extract(node));
  }

  remove(node) {
    return wrapTexformError(() => this.inner.remove(node));
  }

  clear(node) {
    return wrapTexformError(() => this.inner.clear(node));
  }

  setText(node, value) {
    return wrapTexformError(() => this.inner.setText(node, value));
  }

  setChar(node, value) {
    return wrapTexformError(() => this.inner.setChar(node, value));
  }

  setCommandName(node, name) {
    return wrapTexformError(() => this.inner.setCommandName(node, name));
  }

  setArg(node, index, value) {
    return wrapTexformError(() => this.inner.setArg(node, index, value));
  }

  toSyntax() {
    return wrapTexformError(() => this.inner.toSyntax());
  }

  toLatex(options) {
    return wrapTexformError(() => this.inner.toLatex(options ?? undefined));
  }
}
export const Node = wasm.Node;
export const serialize = (node, options) =>
  wrapTexformError(() => wasm.serialize(node, options ?? undefined));
export const validateArgspec = wasm.validate_argspec;
