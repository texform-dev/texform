export function createBindings({
  Document: WasmDocument,
  Parser: WasmParser,
  TransformEngine: WasmTransformEngine,
  serialize: wasmSerialize,
  validateArgspec,
  listPackages: wasmListPackages,
}) {
  class TexformError extends Error {
    constructor(payload, fallback = "texform error") {
      super(payload?.message ?? fallback);
      this.name = "TexformError";
      this.kind = payload?.kind ?? "internal";
    }
  }

  class TexformParseError extends TexformError {
    constructor(payload) {
      super(payload, "parse failed");
      this.name = "TexformParseError";
      this.diagnostics = payload?.diagnostics ?? [];
      this.document = payload?.document ? new Document(payload.document) : null;
    }
  }

  class TexformEditError extends TexformError {
    constructor(payload) {
      super(payload, "edit failed");
      this.name = "TexformEditError";
    }
  }

  class TexformConfigError extends TexformError {
    constructor(payload) {
      super(payload, "invalid texform configuration");
      this.name = "TexformConfigError";
    }
  }

  class TexformTransformError extends TexformError {
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

  function wrapNode(node) {
    return node ? new Node(node) : null;
  }

  function wrapNodes(nodes) {
    return Array.from(nodes, (node) => new Node(node));
  }

  function unwrapNode(node) {
    return node instanceof Node ? node.inner : node;
  }

  function unwrapArgValue(value) {
    if (
      value &&
      typeof value === "object" &&
      (value.kind === "Math" || value.kind === "Text")
    ) {
      return { ...value, node: unwrapNode(value.node) };
    }
    return value;
  }

  function unwrapArgValues(values) {
    return Array.isArray(values) ? values.map(unwrapArgValue) : values;
  }

  function wrapArgRef(value) {
    if (
      value &&
      typeof value === "object" &&
      (value.kind === "Math" || value.kind === "Text")
    ) {
      return { ...value, node: wrapNode(value.node) };
    }
    return value;
  }

  class Parser {
    constructor(options) {
      this.inner = wrapTexformError(() => new WasmParser(options ?? undefined));
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
      return wrapTexformError(() => this.inner.lookup_command(name, mode));
    }

    lookupExplicitCommand(name, mode) {
      return wrapTexformError(() =>
        this.inner.lookup_explicit_command(name, mode),
      );
    }

    lookupCharacter(name, mode) {
      return wrapTexformError(() => this.inner.lookup_character(name, mode));
    }

    lookupEnv(name, mode) {
      return wrapTexformError(() => this.inner.lookup_env(name, mode));
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

  class TransformEngine {
    constructor(options) {
      this.inner = wrapTexformError(() => new WasmTransformEngine(options));
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
      return wrapTexformError(() =>
        this.inner.normalize(src, options ?? undefined),
      );
    }

    transform(document, config) {
      return wrapTexformError(() =>
        this.inner.transform(document.inner, config ?? undefined),
      );
    }

    lookupCommand(name, mode) {
      return wrapTexformError(() => this.inner.lookup_command(name, mode));
    }

    lookupExplicitCommand(name, mode) {
      return wrapTexformError(() =>
        this.inner.lookup_explicit_command(name, mode),
      );
    }

    lookupCharacter(name, mode) {
      return wrapTexformError(() => this.inner.lookup_character(name, mode));
    }

    lookupEnv(name, mode) {
      return wrapTexformError(() => this.inner.lookup_env(name, mode));
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

  class Document {
    constructor(inner) {
      this.inner = inner ?? new WasmDocument();
    }

    static fromSyntax(node) {
      return wrapTexformError(() => new Document(WasmDocument.fromSyntax(node)));
    }

    free() {
      this.inner.free();
    }

    [Symbol.dispose]() {
      this.free();
    }

    root() {
      return wrapTexformError(() => wrapNode(this.inner.root()));
    }

    hasErrors() {
      return wrapTexformError(() => this.inner.hasErrors());
    }

    isReadOnly() {
      return wrapTexformError(() => this.inner.isReadOnly());
    }

    errors() {
      return wrapTexformError(() => wrapNodes(this.inner.errors()));
    }

    findCommands(name) {
      return wrapTexformError(() => wrapNodes(this.inner.findCommands(name)));
    }

    findEnvironments(name) {
      return wrapTexformError(() =>
        wrapNodes(this.inner.findEnvironments(name)),
      );
    }

    createChar(value) {
      return wrapTexformError(() => wrapNode(this.inner.createChar(value)));
    }

    createText(value) {
      return wrapTexformError(() => wrapNode(this.inner.createText(value)));
    }

    createActiveSpace() {
      return wrapTexformError(() => wrapNode(this.inner.createActiveSpace()));
    }

    createGroup(mode) {
      return wrapTexformError(() => wrapNode(this.inner.createGroup(mode)));
    }

    createCommand(name, args) {
      return wrapTexformError(() =>
        wrapNode(
          this.inner.createCommand(name, unwrapArgValues(args) ?? undefined),
        ),
      );
    }

    createDeclarative(name, args) {
      return wrapTexformError(() =>
        wrapNode(
          this.inner.createDeclarative(
            name,
            unwrapArgValues(args) ?? undefined,
          ),
        ),
      );
    }

    createEnvironment(name, args, body) {
      return wrapTexformError(() =>
        wrapNode(
          this.inner.createEnvironment(
            name,
            unwrapArgValues(args) ?? undefined,
            unwrapNode(body),
          ),
        ),
      );
    }

    appendChild(parent, child) {
      return wrapTexformError(() =>
        this.inner.appendChild(unwrapNode(parent), unwrapNode(child)),
      );
    }

    insertChild(parent, index, child) {
      return wrapTexformError(() =>
        this.inner.insertChild(unwrapNode(parent), index, unwrapNode(child)),
      );
    }

    insertBefore(anchor, node) {
      return wrapTexformError(() =>
        this.inner.insertBefore(unwrapNode(anchor), unwrapNode(node)),
      );
    }

    insertAfter(anchor, node) {
      return wrapTexformError(() =>
        this.inner.insertAfter(unwrapNode(anchor), unwrapNode(node)),
      );
    }

    replaceWith(target, replacement) {
      return wrapTexformError(() =>
        this.inner.replaceWith(unwrapNode(target), unwrapNode(replacement)),
      );
    }

    wrap(target, wrapper) {
      return wrapTexformError(() =>
        wrapNode(this.inner.wrap(unwrapNode(target), unwrapNode(wrapper))),
      );
    }

    unwrap(group) {
      return wrapTexformError(() =>
        wrapNodes(this.inner.unwrap(unwrapNode(group))),
      );
    }

    extract(node) {
      return wrapTexformError(() =>
        wrapNode(this.inner.extract(unwrapNode(node))),
      );
    }

    remove(node) {
      return wrapTexformError(() => this.inner.remove(unwrapNode(node)));
    }

    clear(node) {
      return wrapTexformError(() => this.inner.clear(unwrapNode(node)));
    }

    setText(node, value) {
      return wrapTexformError(() => this.inner.setText(unwrapNode(node), value));
    }

    setChar(node, value) {
      return wrapTexformError(() => this.inner.setChar(unwrapNode(node), value));
    }

    setCommandName(node, name) {
      return wrapTexformError(() =>
        this.inner.setCommandName(unwrapNode(node), name),
      );
    }

    setArg(node, index, value) {
      return wrapTexformError(() =>
        this.inner.setArg(unwrapNode(node), index, unwrapArgValue(value)),
      );
    }

    toSyntax() {
      return wrapTexformError(() => this.inner.toSyntax());
    }

    nodeSpans() {
      return wrapTexformError(() => this.inner.nodeSpans());
    }

    toLatex(options) {
      return wrapTexformError(() => this.inner.toLatex(options ?? undefined));
    }
  }

  class Node {
    constructor(inner) {
      this.inner = inner;
    }

    free() {
      this.inner.free();
    }

    [Symbol.dispose]() {
      this.free();
    }

    get kind() {
      return wrapTexformError(() => this.inner.kind);
    }

    isCommand(name) {
      return wrapTexformError(() => this.inner.isCommand(name ?? undefined));
    }

    isChar(value) {
      return wrapTexformError(() => this.inner.isChar(value ?? undefined));
    }

    isError() {
      return wrapTexformError(() => this.inner.isError());
    }

    parent() {
      return wrapTexformError(() => wrapNode(this.inner.parent()));
    }

    get children() {
      return wrapTexformError(() => wrapNodes(this.inner.children));
    }

    nextSibling() {
      return wrapTexformError(() => wrapNode(this.inner.nextSibling()));
    }

    prevSibling() {
      return wrapTexformError(() => wrapNode(this.inner.prevSibling()));
    }

    ancestors() {
      return wrapTexformError(() => wrapNodes(this.inner.ancestors()));
    }

    descendants() {
      return wrapTexformError(() => wrapNodes(this.inner.descendants()));
    }

    get commandName() {
      return wrapTexformError(() => this.inner.commandName);
    }

    get envName() {
      return wrapTexformError(() => this.inner.envName);
    }

    get text() {
      return wrapTexformError(() => this.inner.text);
    }

    get char() {
      return wrapTexformError(() => this.inner.char);
    }

    primeCount() {
      return wrapTexformError(() => this.inner.primeCount());
    }

    errorParts() {
      return wrapTexformError(() => this.inner.errorParts());
    }

    contentMode() {
      return wrapTexformError(() => this.inner.contentMode());
    }

    groupKind() {
      return wrapTexformError(() => this.inner.groupKind());
    }

    argCount() {
      return wrapTexformError(() => this.inner.argCount());
    }

    arg(index) {
      return wrapTexformError(() => wrapArgRef(this.inner.arg(index)));
    }

    argSlots() {
      return wrapTexformError(() => this.inner.argSlots().map(wrapArgRef));
    }

    scriptBase() {
      return wrapTexformError(() => wrapNode(this.inner.scriptBase()));
    }

    subscript() {
      return wrapTexformError(() => wrapNode(this.inner.subscript()));
    }

    superscript() {
      return wrapTexformError(() => wrapNode(this.inner.superscript()));
    }

    infixLeft() {
      return wrapTexformError(() => wrapNode(this.inner.infixLeft()));
    }

    infixRight() {
      return wrapTexformError(() => wrapNode(this.inner.infixRight()));
    }

    envBody() {
      return wrapTexformError(() => wrapNode(this.inner.envBody()));
    }

    span() {
      return wrapTexformError(() => this.inner.span());
    }
  }

  const serialize = (node, options) =>
    wrapTexformError(() => wasmSerialize(node, options ?? undefined));
  const listPackages = () => wrapTexformError(() => wasmListPackages());

  return {
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
  };
}
