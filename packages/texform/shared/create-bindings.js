export function createBindings({
  Document: WasmDocument,
  KnowledgeBase: WasmKnowledgeBase,
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

  class TexformConformanceError extends TexformEditError {
    constructor(payload) {
      super(payload);
      this.name = "TexformConformanceError";
      this.path = payload.path;
      this.rule = payload.rule;
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
      if (error instanceof TexformError) throw error;
      if (
        error &&
        typeof error === "object" &&
        typeof error.kind === "string"
      ) {
        if (error.kind === "parse") {
          throw new TexformParseError(error);
        }
        if (error.kind === "conformance") {
          throw new TexformConformanceError(error);
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
    if (value instanceof Node) return value.inner;
    if (Array.isArray(value)) return value.map(unwrapArgValue);
    if (value && typeof value === "object" && "value" in value) {
      return { ...value, value: unwrapArgValue(value.value) };
    }
    return value;
  }

  function unwrapArgValues(values) {
    return Array.isArray(values) ? values.map(unwrapArgValue) : values;
  }

  // Rust validates option values; this rejects keys it never receives.
  function checkOptions(options, keys, what) {
    if (options == null) return {};
    if (
      typeof options !== "object" ||
      Array.isArray(options) ||
      Object.keys(options).some((key) => !keys.includes(key))
    )
      throw new TexformConfigError({
        kind: "config",
        message: `${what} accept only ${keys.join(", ")}`,
      });
    return options;
  }

  function constructionMode(options) {
    return checkOptions(options, ["mode"], "construction options").mode;
  }

  // wasm-bindgen wraps out-of-range integers, so non-u32 counts stop here.
  function primeCount(count, path) {
    if (Number.isSafeInteger(count) && count >= 0 && count <= 0xffffffff)
      return count;
    throw new TexformConformanceError({
      kind: "conformance",
      path,
      rule: "invalid_prime_count",
      message: "prime count must be a positive integer",
    });
  }

  function wrapArgRef(value) {
    if (
      value &&
      typeof value === "object" &&
      (value.kind === "Math" || value.kind === "Text" || value.kind === "OperatorName")
    ) {
      return { ...value, node: wrapNode(value.node) };
    }
    return value;
  }

  function unwrapKnowledgeBase(kb) {
    if (kb == null) return undefined;
    if (!(kb instanceof KnowledgeBase))
      throw new TexformConfigError({
        kind: "config",
        message: "knowledgeBase must be a KnowledgeBase",
      });
    return kb.inner.clone();
  }
  function withoutKnowledgeBase(options) {
    if (
      options == null ||
      typeof options !== "object" ||
      Array.isArray(options)
    )
      return options;
    const { knowledgeBase, ...rest } = options;
    return rest;
  }
  class KnowledgeBase {
    constructor(options) {
      this.inner = wrapTexformError(() =>
        options instanceof WasmKnowledgeBase
          ? options
          : new WasmKnowledgeBase(options),
      );
      Object.freeze(this);
    }
    free() {
      this.inner.free();
    }
    [Symbol.dispose]() {
      this.free();
    }
    isSame(other) {
      return this.inner.isSame(other.inner);
    }
    packages() {
      return wrapTexformError(() => this.inner.packages());
    }
    commands(mode) {
      return wrapTexformError(() => this.inner.commands(mode));
    }
    environments(mode) {
      return wrapTexformError(() => this.inner.environments(mode));
    }
    characters(mode) {
      return wrapTexformError(() => this.inner.characters(mode));
    }
    delimiters() {
      return wrapTexformError(() => this.inner.delimiters());
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

  class Parser {
    knowledgeBase() {
      return new KnowledgeBase(this.inner.knowledgeBase());
    }
    constructor(options) {
      this.inner = wrapTexformError(
        () =>
          new WasmParser(
            withoutKnowledgeBase(options),
            unwrapKnowledgeBase(options?.knowledgeBase),
          ),
      );
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

    defaultParseConfig() {
      return wrapTexformError(() => this.inner.defaultParseConfig());
    }
  }

  class TransformEngine {
    knowledgeBase() {
      return new KnowledgeBase(this.inner.knowledgeBase());
    }
    constructor(options) {
      this.inner = wrapTexformError(
        () =>
          new WasmTransformEngine(
            withoutKnowledgeBase(options),
            unwrapKnowledgeBase(options?.knowledgeBase),
          ),
      );
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

    defaultParseConfig() {
      return wrapTexformError(() => this.inner.defaultParseConfig());
    }

    defaultTransformConfig() {
      return wrapTexformError(() => this.inner.defaultTransformConfig());
    }

    normalize(src, options) {
      return wrapTexformError(() =>
        this.inner.normalize(src, options ?? undefined),
      );
    }

    normalizeWithReport(src, options) {
      return wrapTexformError(() =>
        this.inner.normalizeWithReport(src, options ?? undefined),
      );
    }

    transform(document, config) {
      return wrapTexformError(() =>
        this.inner.transform(document.inner, config ?? undefined),
      );
    }

    transformWithReport(document, config) {
      return wrapTexformError(() =>
        this.inner.transformWithReport(document.inner, config ?? undefined),
      );
    }
  }

  class Document {
    knowledgeBase() {
      return wrapTexformError(
        () => new KnowledgeBase(this.inner.knowledgeBase()),
      );
    }
    clone() {
      return wrapTexformError(() => new Document(this.inner.clone()));
    }
    constructor(options) {
      if (!(options instanceof WasmDocument))
        options = checkOptions(options, ["knowledgeBase", "mode"], "document options");
      this.inner = wrapTexformError(() =>
        options instanceof WasmDocument
          ? options
          : new WasmDocument(
              unwrapKnowledgeBase(options?.knowledgeBase),
              options?.mode,
            ),
      );
    }

    static fromSyntax(node, options) {
      options = checkOptions(options, ["knowledgeBase"], "fromSyntax options");
      return wrapTexformError(
        () =>
          new Document(
            WasmDocument.fromSyntax(
              node,
              unwrapKnowledgeBase(options?.knowledgeBase),
            ),
          ),
      );
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

    createChar(value, options) {
      return wrapTexformError(() => wrapNode(this.inner.createChar(value, constructionMode(options))));
    }

    createText(value, options) {
      return wrapTexformError(() => wrapNode(this.inner.createText(value, constructionMode(options))));
    }

    createActiveSpace(options) {
      return wrapTexformError(() => wrapNode(this.inner.createActiveSpace(constructionMode(options))));
    }

    createAlignmentTab(options) {
      return wrapTexformError(() => wrapNode(this.inner.createAlignmentTab(constructionMode(options))));
    }

    createGroup(mode, children = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createGroup(mode, unwrapArgValues(children), constructionMode(options))));
    }

    createDelimitedGroup(left, right, children = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createDelimitedGroup(left, right, unwrapArgValues(children), constructionMode(options))));
    }

    createInlineMath(children = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createInlineMath(unwrapArgValues(children), constructionMode(options))));
    }

    createScripted(base, sub = null, sup = null, options) {
      return wrapTexformError(() => wrapNode(this.inner.createScripted(unwrapArgValue(base), unwrapArgValue(sub), unwrapArgValue(sup), constructionMode(options))));
    }

    createPrime(count, options) {
      count = primeCount(count, "detached");
      return wrapTexformError(() => wrapNode(this.inner.createPrime(count, constructionMode(options))));
    }

    createInfix(name, left, right, args = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createInfix(name, unwrapArgValue(left), unwrapArgValue(right), unwrapArgValues(args), constructionMode(options))));
    }

    createCommand(name, args = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createCommand(name, unwrapArgValues(args), constructionMode(options))));
    }

    createDeclarative(name, args = [], options) {
      return wrapTexformError(() => wrapNode(this.inner.createDeclarative(name, unwrapArgValues(args), constructionMode(options))));
    }

    createEnvironment(name, args = [], body = null, options) {
      return wrapTexformError(() => wrapNode(this.inner.createEnvironment(name, unwrapArgValues(args), unwrapArgValue(body), constructionMode(options))));
    }
    parseFragment(source, options) {
      return wrapTexformError(() => wrapNode(this.inner.parseFragment(source, constructionMode(options))));
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
      return wrapTexformError(() =>
        this.inner.setText(unwrapNode(node), value),
      );
    }

    setChar(node, value) {
      return wrapTexformError(() =>
        this.inner.setChar(unwrapNode(node), value),
      );
    }

    setCommandName(node, name) {
      return wrapTexformError(() =>
        this.inner.setCommandName(unwrapNode(node), name),
      );
    }

    setEnvName(node, name) {
      return wrapTexformError(() => this.inner.setEnvName(unwrapNode(node), name));
    }

    nodeAt(path) {
      return wrapTexformError(() => wrapNode(this.inner.nodeAt(path)));
    }

    cloneNode(node) {
      return wrapTexformError(() => wrapNode(this.inner.cloneNode(unwrapNode(node))));
    }

    importNode(node) {
      return wrapTexformError(() => wrapNode(this.inner.importNode(unwrapNode(node))));
    }

    setSubscript(node, value) {
      return wrapTexformError(() => wrapNode(this.inner.setSubscript(unwrapNode(node), unwrapArgValue(value))));
    }

    setSuperscript(node, value) {
      return wrapTexformError(() => wrapNode(this.inner.setSuperscript(unwrapNode(node), unwrapArgValue(value))));
    }

    setArgDelimiters(node, index, open, close) {
      return wrapTexformError(() => this.inner.setArgDelimiters(unwrapNode(node), index, open, close));
    }

    setDelimiters(node, left, right) {
      return wrapTexformError(() => this.inner.setDelimiters(unwrapNode(node), left, right));
    }

    setPrimeCount(node, count) {
      count = primeCount(count, node.path() ?? "detached");
      return wrapTexformError(() => this.inner.setPrimeCount(unwrapNode(node), count));
    }

    setArg(node, index, value) {
      return wrapTexformError(() =>
        this.inner.setArg(unwrapNode(node), index, unwrapArgValue(value)),
      );
    }

    toColumnar() {
      return wrapTexformError(() => this.inner.toColumnar());
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

    toTokenizedLatex(options) {
      return wrapTexformError(() =>
        this.inner.toTokenizedLatex(options ?? undefined),
      );
    }
  }

  class Node {
    isSameNode(other) {
      return this.inner.isSameNode(unwrapNode(other));
    }
    document() {
      return new Document(this.inner.document());
    }
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

    path() {
      return wrapTexformError(() => this.inner.path());
    }

    slot() {
      return wrapTexformError(() => this.inner.slot());
    }

    isKnown() {
      return wrapTexformError(() => this.inner.isKnown());
    }

    argKind(index) {
      return wrapTexformError(() => this.inner.argKind(index));
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
  };
}
