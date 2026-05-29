import init, {
  Document,
  Engine as WasmEngine,
  Node,
  Parser as WasmParser,
  serialize as wasmSerialize,
  validate_argspec,
} from "../wasm/web/texform_wasm.js";
import wasmUrl from "../wasm/web/texform_wasm_bg.wasm?url";

await init({ module_or_path: new URL(wasmUrl, import.meta.url) });

export class TexformParseError extends Error {
  constructor(payload) {
    const diagnostics = payload?.diagnostics ?? [];
    super(diagnostics[0]?.message ?? "parse failed");
    this.name = "TexformParseError";
    this.diagnostics = diagnostics;
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

export class Parser {
  constructor(options) {
    this.inner = new WasmParser(options ?? undefined);
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

export class Engine {
  constructor(options) {
    this.inner = new WasmEngine(options);
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

export { Document, Node };
export const serialize = wasmSerialize;
export { validate_argspec as validateArgspec };
