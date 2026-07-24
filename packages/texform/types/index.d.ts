/**
 * An argument slot of a {@link Node}'s argument list: either a present
 * {@link Argument} or an empty slot (`null`/`undefined`).
 *
 * Optional argument slots that were not supplied in the source surface as the
 * empty value rather than being dropped, so a slot's index stays stable.
 */
export type ArgumentSlot = Argument | null | undefined;

/**
 * Parser strictness configuration, as a plain object with camelCase keys.
 *
 * The two boolean axes are orthogonal: {@link ParseConfigInput.rejectUnknown}
 * controls how unknown names are handled, while
 * {@link ParseConfigInput.abortOnError} controls whether the parser stops at
 * the first error or keeps recovering. Neither implies the other, and neither
 * is equivalent to the resulting tree's {@link Document.hasErrors} signal.
 *
 * @example
 * ```ts
 * new Parser().parse(String.raw`\foobar{x}`, {
 *   rejectUnknown: true,
 *   abortOnError: true,
 * });
 * ```
 */
export interface ParseConfigInput {
  /**
   * Reject unknown command and environment names. Default `false`.
   *
   * When `true`, an unknown command or environment becomes an
   * `unknown-command` / `unknown-environment` diagnostic. When `false`, unknown
   * names are preserved as `known: false` nodes for lenient exploration.
   */
  rejectUnknown?: boolean;
  /**
   * Stop at the first error per item instead of recovering. Default `false`.
   *
   * When `true` (strict), the parser stops at the first error per item and
   * produces no recovery `Error` nodes. When `false` (lenient), it keeps
   * collecting diagnostics and emits `Error` placeholder nodes. The
   * max-group-depth guard emits an `Error` node regardless of this setting.
   * This is a parse-strictness knob, distinct from {@link Document.hasErrors},
   * which is a property of the resulting tree.
   */
  abortOnError?: boolean;
  /**
   * Maximum group nesting depth before the parser aborts a group. Default
   * `128`. Exceeding it emits a `max-group-depth-exceeded` diagnostic and an
   * `Error` node unconditionally.
   */
  maxGroupDepth?: number;
}

/**
 * The category of a {@link ParseDiagnostic}, or `null` when the parser emits a
 * diagnostic without a specific category.
 *
 * Each variant names a distinct recoverable or rejected parse condition, such
 * as an unknown command, an argument-validation failure, or an unclosed inline
 * math span.
 */
export type ParseDiagnosticKind =
  | "ambiguous-infix"
  | "argument-validation"
  | "command-mode-error"
  | "comment-truncated-argument"
  | "environment-mode-error"
  | "environment-name-mismatch"
  | "left-right-delimiter"
  | "max-group-depth-exceeded"
  | "raw-expected-found"
  | "text-script-error"
  | "unclosed-inline-math"
  | "unexpected-math-shift"
  | "unknown-command"
  | "unknown-environment";

/**
 * How aggressively a transform rule may rewrite, layered from least to most
 * destructive of stylistic detail.
 *
 * A rule level is the first profile that accepts a rule's output as a
 * suitable product; it is orthogonal to a rule's equivalence guarantee.
 */
export type RuleLevel = "authoring" | "faithful" | "corpus" | "equiv";

/**
 * Normalization profile selecting the canonical form for one downstream
 * scenario: `authoring` (polished author-facing output), `faithful` (same
 * rendered formula, convenience macros expanded), `corpus` (training-data
 * normalization, layout hints dropped), or `equiv` (aggressive
 * canonicalization for equivalence comparison).
 *
 * @see {@link TransformEngineOptions}
 */
export type TransformProfile = "authoring" | "faithful" | "corpus" | "equiv";

/**
 * A source byte range, half-open as `[start, end)`, indexing into the original
 * LaTeX input.
 */
export interface Span {
  /** Inclusive start byte offset. */
  start: number;
  /** Exclusive end byte offset. */
  end: number;
}

/**
 * One nested context frame attached to a {@link ParseDiagnostic}, locating the
 * enclosing construct (such as the command or environment) the error occurred
 * within.
 */
export interface ParseDiagnosticContext {
  /** Human-readable label for the enclosing construct. */
  label: string;
  /** Source span of the enclosing construct. */
  span: Span;
}

/**
 * One diagnostic emitted while parsing, describing a recovered or rejected
 * condition.
 *
 * Diagnostics are a separate channel from `Error` placeholder nodes: a
 * diagnostic does not by itself make an otherwise editable tree read-only.
 * Within a parser-produced result, however, a tree that
 * {@link Document.hasErrors} always comes with at least one diagnostic.
 *
 * @see {@link ParseResult}
 */
export interface ParseDiagnostic {
  /** The diagnostic category, or `null` when uncategorized. */
  kind: ParseDiagnosticKind | null;
  /** Human-readable description of the problem. */
  message: string;
  /** Source span the diagnostic points at. */
  span: Span;
  /** Tokens the parser expected at this position; may be empty. */
  expected: string[];
  /** The token actually found, or `null`. */
  found: string | null;
  /** Enclosing-construct frames, outermost-relevant first; may be empty. */
  contexts: ParseDiagnosticContext[];
}

/**
 * Content mode as it appears in a {@link SyntaxNode} snapshot: `"Math"` or
 * `"Text"` (capitalized snapshot form).
 *
 * @see {@link RuntimeContentMode}
 */
export type SyntaxContentMode = "Math" | "Text";

/**
 * Content mode as reported by the runtime tree API: `"math"` or `"text"`
 * (lowercase form used by {@link Node.contentMode} and the `mode` arguments of
 * lookup methods).
 *
 * @see {@link SyntaxContentMode}
 */
export type RuntimeContentMode = "math" | "text";

/**
 * A delimiter token in {@link SyntaxNode} snapshot form: the literal `"None"`,
 * a single character (`{ Char }`), or a control sequence (`{ Control }`).
 *
 * @see {@link DelimiterValue}
 */
export type Delimiter = "None" | { Char: string } | { Control: string };

/**
 * A delimiter token in runtime view form, with a `kind` discriminator and a
 * payload. Returned by {@link Node.groupKind} for `Delimited` groups.
 *
 * @see {@link Delimiter}
 */
export type DelimiterValue =
  | { kind: "None" }
  | { kind: "Char"; value: string }
  | { kind: "Control"; value: string };

/**
 * The kind of a group in {@link SyntaxNode} snapshot form: an `Explicit` brace
 * group, an `Implicit` group, a `Delimited` group carrying its `left`/`right`
 * delimiters, or an `InlineMath` group.
 *
 * @see {@link GroupKindRef}
 */
export type GroupKind =
  | "Explicit"
  | "Implicit"
  | { Delimited: { left: Delimiter; right: Delimiter } }
  | "InlineMath";

/**
 * The kind of a group in runtime view form, with a `kind` discriminator.
 * Returned by {@link Node.groupKind}; the discriminator keeps its capitalized
 * form (`'Explicit'`, `'Delimited'`, ...). A `Delimited` group carries
 * {@link DelimiterValue} `left` and `right` fields.
 *
 * @see {@link GroupKind}
 */
export type GroupKindRef =
  | { kind: "Explicit" }
  | { kind: "Implicit" }
  | { kind: "Delimited"; left: DelimiterValue; right: DelimiterValue }
  | { kind: "InlineMath" };

/**
 * The syntactic kind of a command argument in {@link SyntaxNode} snapshot form
 * (mandatory, optional, starred, group, or a delimited/paired form carrying its
 * `open`/`close` delimiters).
 *
 * @see {@link ArgSpecFormInfo}
 */
export type ArgumentKind =
  | "Mandatory"
  | "Optional"
  | "Star"
  | "Group"
  | { Delimited: { open: Delimiter; close: Delimiter } }
  | { Paired: { open: Delimiter; close: Delimiter } };

/**
 * The lossless, immutable parse snapshot — the single serde wire format.
 *
 * `SyntaxNode` is a plain object (not a class instance) that backs JSON
 * snapshots, transport across the binding, and test fixtures. It can represent
 * a partial parse: unparseable fragments survive as `Error` nodes. It carries
 * no editing behavior; bridge it into an editable tree with
 * {@link Document.fromSyntax}, and recover one with {@link Document.toSyntax}.
 * The two conversions are symmetric over every node kind, including `Error` and
 * `Prime`.
 *
 * @see {@link Document}
 */
export type SyntaxNode =
  | { Root: { mode: SyntaxContentMode; children: SyntaxNode[] } }
  | { Group: { mode: SyntaxContentMode; kind: GroupKind; children: SyntaxNode[] } }
  | { Command: { name: string; args: ArgumentSlot[]; known: boolean } }
  | { Infix: { name: string; args: ArgumentSlot[]; left: SyntaxNode; right: SyntaxNode } }
  | { Declarative: { name: string; args: ArgumentSlot[] } }
  | { Environment: { name: string; args: ArgumentSlot[]; known: boolean; body: SyntaxNode } }
  | { Scripted: { base: SyntaxNode; subscript?: SyntaxNode; superscript?: SyntaxNode } }
  | { Prime: { count: number } }
  | { Error: { message: string; snippet: string } }
  | { Text: string }
  | { Char: string }
  | "ActiveSpace";

/**
 * A present command/environment argument in {@link SyntaxNode} snapshot form,
 * pairing its syntactic {@link ArgumentKind} with its parsed
 * {@link ArgumentValue}.
 */
export interface Argument {
  /** The argument's syntactic kind (mandatory, optional, delimited, ...). */
  kind: ArgumentKind;
  /** Whether the argspec slot was prefixed with `!`. */
  no_leading_space?: boolean;
  /** The argument's parsed value. */
  value: ArgumentValue;
}

/**
 * The parsed value carried by an {@link Argument} in snapshot form: math or
 * text content, a delimiter, a control-sequence name, a dimension, an integer,
 * a key=value list, a column spec, or a boolean flag.
 *
 * @see {@link ArgRef}
 */
export type ArgumentValue =
  | { MathContent: SyntaxNode }
  | { TextContent: SyntaxNode }
  | { OperatorNameContent: SyntaxNode }
  | { Delimiter: Delimiter }
  | { CSName: string }
  | { Dimension: string }
  | { Integer: string }
  | { KeyVal: string }
  | { Column: string }
  | { Boolean: boolean };

/**
 * The runtime kind of a {@link Node}, read from the lowercase
 * {@link Node.kind} property.
 *
 * Note the casing convention: node kinds are lowercase strings (`'command'`,
 * `'scripted'`), unlike the capitalized discriminators inside argument and
 * group views (`arg.kind === 'Math'`, `groupKind().kind === 'Explicit'`).
 */
export type NodeKind =
  | "root"
  | "group"
  | "command"
  | "infix"
  | "declarative"
  | "environment"
  | "scripted"
  | "prime"
  | "text"
  | "char"
  | "activeSpace"
  | "error";

/**
 * A command/environment argument in runtime view form, returned by
 * {@link Node.arg} and {@link Node.argSlots}.
 *
 * Content kinds (`Math`, `Text`) carry a live `node` {@link Node} handle; leaf
 * kinds (`Delimiter`, `CSName`, `Dimension`, `Integer`, `KeyVal`, `Column`,
 * `Boolean`) carry a `value`. The `kind` discriminator keeps its capitalized
 * form.
 *
 * @see {@link ArgumentValue}
 */
export type ArgRef =
  | { kind: "Math"; node: Node }
  | { kind: "Text"; node: Node }
  | { kind: "Delimiter"; value: DelimiterValue }
  | { kind: "CSName"; value: string }
  | { kind: "Dimension"; value: string }
  | { kind: "Integer"; value: string }
  | { kind: "KeyVal"; value: string }
  | { kind: "Column"; value: string }
  | { kind: "Boolean"; value: boolean };

/**
 * An argument value supplied to the `create*` and {@link Document.setArg}
 * staging methods. It shares the shape of {@link ArgRef}: content kinds carry a
 * `node`, leaf kinds carry a `value`.
 *
 * @see {@link ArgRef}
 */
export type ArgValueInput = ArgRef;

/**
 * The outcome of {@link Parser.parse} (and {@link TransformEngine.parse}): an
 * optional {@link Document} plus the diagnostics emitted while parsing.
 *
 * There are exactly three honest states, and `parse` never throws on malformed
 * input nor fabricates a placeholder tree:
 *
 * @remarks
 * - **Hard failure** — `document` is `null`; the failure is described entirely
 *   by `diagnostics`.
 * - **Clean parse** — `document` is a complete, editable tree with
 *   `hasErrors() === false`. Empty input (`''`) lands here, not in `null`.
 * - **Partial parse** — `document` is present with `hasErrors() === true`;
 *   recovery preserved the unparseable parts as read-only `Error` nodes.
 *
 * @see {@link Document}
 */
export interface ParseResult {
  /** The parsed document, or `null` on a hard failure with no tree. */
  document: Document | null;
  /** Diagnostics emitted while parsing; may be empty on a clean parse. */
  diagnostics: ParseDiagnostic[];
}

/** Closed semantic categories emitted by canonical tokenized serialization. */
export type SerializationTokenKind =
  | "control_sequence"
  | "character"
  | "delimiter"
  | "text"
  | "raw"
  | "error";

/** One non-empty canonical serialization fragment. */
export interface SerializationToken {
  text: string;
  /** UTF-8 byte offset, not a JavaScript UTF-16 string index. */
  startByte: number;
  /** UTF-8 byte offset, not a JavaScript UTF-16 string index. */
  endByte: number;
  kind: SerializationTokenKind;
  mode: "math" | "text";
}

/** Canonical LaTeX and tokens produced by the same serializer traversal. */
export interface TokenizedLatex {
  latex: string;
  tokens: SerializationToken[];
}

/**
 * The editable LaTeX document tree — the working format you read, mutate,
 * serialize, and transform.
 *
 * Reads go through read-only {@link Node} handles; edits are validated eagerly
 * and throw {@link TexformEditError} on misuse, so no internal failure ever
 * corrupts the tree. A tree that {@link Document.hasErrors} is read-only and
 * fixed that way at construction: every editing method then throws.
 *
 * `Document` is a live WASM-backed class instance, not plain data. Documents
 * obtained from {@link TransformEngine.parse} remember their parser context and
 * can be transformed in place by that same engine; documents built with
 * `new Document()` or {@link Document.fromSyntax} can be edited and serialized
 * but not transformed. For the conceptual model, see the Parsing guide.
 *
 * @see {@link Node}
 * @see {@link TransformEngine}
 * @example
 * ```ts
 * import { Document } from 'texform';
 *
 * const doc = new Document();
 * doc.toLatex(); // ''
 * ```
 */
export class Document {
  /**
   * Construct an empty math-mode document holding a single empty root.
   *
   * This is a complete tree (`hasErrors()` is `false`) — the same legal state
   * an empty parse produces. The document is editable and serializable, but it
   * is not associated with a parser context and cannot be passed to
   * {@link TransformEngine.transform}.
   */
  constructor();
  /**
   * Build a document from a {@link SyntaxNode} snapshot.
   *
   * Invalid external syntax is rejected rather than corrupting the tree.
   * `fromSyntax` and {@link Document.toSyntax} are symmetric over every node
   * kind, including `Error` and `Prime`. The imported document is not
   * associated with a parser context, so it cannot be passed to
   * {@link TransformEngine.transform}.
   *
   * @param node - A `SyntaxNode` object, typically produced by `toSyntax()`.
   * @returns The reconstructed editable document.
   * @example
   * ```ts
   * import { Document, Parser } from 'texform';
   *
   * const result = new Parser().parse(String.raw`\frac{x}{y}`);
   * if (!result.document) throw new Error('parse failed');
   * const syntax = result.document.toSyntax();
   * const doc = Document.fromSyntax(syntax);
   * ```
   */
  static fromSyntax(node: SyntaxNode): Document;
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Return the root {@link Node} of the tree.
   *
   * The root is unique and parentless; it is the document's top-level container
   * node — a `Root` node, not a `Group` — and its children are the top-level
   * content.
   *
   * @returns The root node handle.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\frac{x}{y}`).document;
   * doc.root().kind; // 'root'
   * ```
   */
  root(): Node;
  /**
   * Whether the tree contains any `Error` placeholder node.
   *
   * This is a cheap O(1) signal, independent of the
   * {@link ParseConfigInput.abortOnError} parse-strictness knob and separate
   * from structural validity. A tree that has errors is read-only.
   *
   * @returns `true` if any `Error` node is present.
   * @example
   * ```ts
   * new Parser().parse(String.raw`\sqrt[`).document.hasErrors(); // true
   * ```
   */
  hasErrors(): boolean;
  /**
   * Whether the document rejects edits.
   *
   * Read-only-ness is fixed at construction and is equivalent to
   * {@link Document.hasErrors}: a tree containing error nodes cannot be edited,
   * so its error count cannot change.
   *
   * @returns `true` if every editing method will throw.
   */
  isReadOnly(): boolean;
  /**
   * Return the `Error` placeholder {@link Node} handles in the tree.
   *
   * @returns An array of the `Error` nodes; empty when `hasErrors()` is false.
   */
  errors(): Node[];
  /**
   * Return every command {@link Node} in the tree whose name equals `name`.
   *
   * @param name - The command name to match, without the leading backslash.
   * @returns The matching command node handles.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\frac{x}{y} + \frac{a}{b}`).document;
   * doc.findCommands('frac').length; // 2
   * ```
   */
  findCommands(name: string): Node[];
  /**
   * Return every environment {@link Node} in the tree whose name equals `name`.
   *
   * @param name - The environment name to match.
   * @returns The matching environment node handles.
   */
  findEnvironments(name: string): Node[];
  /**
   * Stage a detached single-character node owned by this document.
   *
   * The node is not in the tree until attached with an edit method such as
   * {@link Document.appendChild}.
   *
   * @param value - The single character.
   * @returns The staged node handle.
   */
  createChar(value: string): Node;
  /**
   * Stage a detached text node owned by this document.
   *
   * @param value - The text content.
   * @returns The staged node handle.
   */
  createText(value: string): Node;
  /**
   * Stage a detached active-space node (an explicit space token).
   *
   * @returns The staged node handle.
   */
  createActiveSpace(): Node;
  /**
   * Stage a detached, empty brace group with the given content mode.
   *
   * @param mode - The group's content mode, `"math"` or `"text"`.
   * @returns The staged node handle.
   */
  createGroup(mode: RuntimeContentMode): Node;
  /**
   * Stage a detached command node with the given name and arguments.
   *
   * @param name - The command name, without the leading backslash.
   * @param args - The argument values, or `null`/omitted for none. Each entry
   *   is an {@link ArgValueInput} object: content kinds carry a `node`, leaf
   *   kinds carry a `value`.
   * @returns The staged node handle.
   * @example
   * ```ts
   * const doc = new Document();
   * const inner = doc.createChar('x');
   * const cmd = doc.createCommand('sqrt', [{ kind: 'Math', node: inner }]);
   * doc.appendChild(doc.root(), cmd);
   * doc.toLatex(); // '\\sqrt { x }'
   * ```
   */
  createCommand(name: string, args?: ArgValueInput[] | null): Node;
  /**
   * Stage a detached declarative command node (such as a font declaration).
   *
   * @param name - The declarative command name.
   * @param args - The argument values, or `null`/omitted for none.
   * @returns The staged node handle.
   */
  createDeclarative(name: string, args?: ArgValueInput[] | null): Node;
  /**
   * Stage a detached environment node wrapping `body`.
   *
   * @param name - The environment name.
   * @param args - The argument values, or `null`/`undefined` for none.
   * @param body - The environment body, which must be a group {@link Node}.
   * @returns The staged node handle.
   */
  createEnvironment(name: string, args: ArgValueInput[] | null | undefined, body: Node): Node;
  /**
   * Append `child` as the last child of `parent`.
   *
   * @param parent - The container node to append into.
   * @param child - The node to append.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * either node is foreign or missing, `parent` is not a container, or the move
   * would create a cycle.
   */
  appendChild(parent: Node, child: Node): void;
  /**
   * Insert `child` at position `index` among `parent`'s children.
   *
   * @param parent - The container node.
   * @param index - The zero-based insertion index.
   * @param child - The node to insert.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * either node is foreign or missing, `parent` is not a container, `index` is
   * out of range, or the move would create a cycle.
   */
  insertChild(parent: Node, index: number, child: Node): void;
  /**
   * Insert `node` immediately before the sibling `anchor`.
   *
   * @param anchor - The sibling to insert before.
   * @param node - The node to insert.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * either node is foreign or missing, `anchor` has no parent (such as the
   * root), or the move would create a cycle.
   */
  insertBefore(anchor: Node, node: Node): void;
  /**
   * Insert `node` immediately after the sibling `anchor`.
   *
   * @param anchor - The sibling to insert after.
   * @param node - The node to insert.
   * @remarks Same failure conditions as {@link Document.insertBefore}.
   */
  insertAfter(anchor: Node, node: Node): void;
  /**
   * Replace `target` in place with `replacement`.
   *
   * @param target - The node to replace.
   * @param replacement - The node to put in its place.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * either node is foreign or missing, `target` is the root, or the move would
   * create a cycle.
   */
  replaceWith(target: Node, replacement: Node): void;
  /**
   * Wrap `target` inside the container `wrapper`.
   *
   * `wrapper` takes `target`'s place in the tree and `target` becomes its
   * child.
   *
   * @param target - The node to wrap.
   * @param wrapper - The container node to wrap it in.
   * @returns The `wrapper` node, now positioned in the tree.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * either node is foreign or missing, `wrapper` is not a container, or
   * `target` is the root.
   */
  wrap(target: Node, wrapper: Node): Node;
  /**
   * Unwrap a group, splicing its children into its parent in place.
   *
   * @param group - The group node to dissolve.
   * @returns The freed child node handles, now in the parent.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `group` is foreign, missing, not a group, or the root.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`{x y}`).document;
   * const group = doc.root().descendants().find((n) => n.kind === 'group');
   * doc.unwrap(group);
   * doc.toLatex(); // 'x y'
   * ```
   */
  unwrap(group: Node): Node[];
  /**
   * Detach the subtree rooted at `node` and return it as a staged node.
   *
   * The subtree is removed from its parent but kept alive, so it can be
   * re-attached elsewhere.
   *
   * @param node - The subtree root to detach.
   * @returns The detached node, now staged.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or the root.
   */
  extract(node: Node): Node;
  /**
   * Remove the subtree rooted at `node` and discard it.
   *
   * @param node - The subtree root to remove.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or the root.
   */
  remove(node: Node): void;
  /**
   * Remove every child of the container `node`.
   *
   * @param node - The container to empty.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or not a container.
   */
  clear(node: Node): void;
  /**
   * Set the content of a text node.
   *
   * @param node - The text node to update.
   * @param value - The new text content.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or not a text node.
   */
  setText(node: Node, value: string): void;
  /**
   * Set the character of a char node.
   *
   * @param node - The char node to update.
   * @param value - The new single character.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or not a char node.
   */
  setChar(node: Node, value: string): void;
  /**
   * Rename a command node.
   *
   * @param node - The command node to rename.
   * @param name - The new command name.
   * @remarks Throws {@link TexformEditError} if the document is read-only, or
   * `node` is foreign, missing, or not a command node.
   */
  setCommandName(node: Node, name: string): void;
  /**
   * Set the argument at `index` of a command or environment node.
   *
   * @param node - The command or environment node.
   * @param index - The zero-based argument slot index.
   * @param value - The new argument value.
   * @remarks Throws {@link TexformEditError} if the document is read-only,
   * `node` is foreign or missing, there is no argument slot at `index`, or
   * `value` does not match the slot shape.
   */
  setArg(node: Node, index: number, value: ArgValueInput): void;
  /**
   * Convert the tree to a {@link SyntaxNode} object for serde and transport.
   *
   * This is the structured-data channel, distinct from the LaTeX text channel
   * {@link Document.toLatex}.
   *
   * @returns The lossless snapshot of the tree.
   * @example
   * ```ts
   * doc.toSyntax(); // { Root: { ... } }
   * ```
   */
  toSyntax(): SyntaxNode;
  /**
   * Export the parse-time span side table as a list of `{id, span}` entries.
   *
   * Ids follow the parser's tree-path scheme rooted at `root`: `.child.N` for
   * container children, `.arg.N.content` for content-carrying argument slots,
   * `.left` / `.right` for infix operands, `.body` for environment bodies, and
   * `.base` / `.sub` / `.sup` for script slots. Nodes without a recorded span
   * are omitted. Spans reflect the original parse and are not updated by edits.
   *
   * @returns The span entries in tree order.
   */
  nodeSpans(): NodeSpanEntry[];
  /**
   * Serialize the tree back to LaTeX text using the canonical serializer.
   *
   * The serializer guarantees text idempotency: re-parsing and re-serializing
   * the output yields the same string. `Error` nodes round-trip their captured
   * source snippet verbatim, and pure prime superscripts serialize compactly as
   * `f'` or `f''`. There is intentionally no method named `serialize` on
   * `Document`. For the full option axes, see the Serialization guide and
   * {@link SerializeOptions}.
   *
   * @param options - A {@link SerializeOptions} object, or omit/`null` for the
   *   default (spaced) style.
   * @returns The canonical LaTeX string.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`x^2`).document;
   * doc.toLatex();                                              // 'x ^ { 2 }'
   * doc.toLatex({ math: { scripts: { spacing: 'compact' } } }); // 'x^{ 2 }'
   * ```
   */
  toLatex(options?: SerializeOptions | null): string;
  /**
   * Serialize canonical LaTeX together with typed output tokens.
   *
   * Token spans use UTF-8 byte offsets, not JavaScript UTF-16 indices. Empty
   * error snippets produce no zero-width token; use {@link Document.hasErrors}
   * to detect whether the document contains error nodes.
   */
  toTokenizedLatex(options?: SerializeOptions | null): TokenizedLatex;
}

/**
 * One entry of {@link Document.nodeSpans}: a node identifier paired with its
 * recorded source {@link Span}.
 */
export interface NodeSpanEntry {
  /** Tree path such as `root.child.0.arg.1.content`. */
  id: string;
  /** The node's source byte span recorded by the parser. */
  span: Span;
}

/**
 * A read-only handle into a {@link Document}.
 *
 * `Node` carries navigation and accessor members but no editing methods — all
 * edits go through the owning document. It is a cheap reference, not a copy of
 * the subtree; reads delegate back into the document. Obtain handles from
 * {@link Document.root}, {@link Document.errors}, {@link Document.findCommands},
 * the `create*` staging constructors, and the navigation members below.
 *
 * @see {@link Document}
 */
export class Node {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * The node's kind, read as a property (not a method).
   *
   * Its value is one of the lowercase {@link NodeKind} strings (`'root'`,
   * `'group'`, `'command'`, `'scripted'`, ...).
   *
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\frac{x}{y}`).document;
   * doc.root().kind; // 'root'
   * ```
   */
  readonly kind: NodeKind;
  /**
   * Whether the node is a command; with `name`, whether it is that specific
   * command.
   *
   * @param name - Optional command name to match; omit/`null` matches any
   *   command.
   * @returns `true` if the node is a (matching) command.
   * @example
   * ```ts
   * node.isCommand();       // any command
   * node.isCommand('frac'); // specifically \frac
   * ```
   */
  isCommand(name?: string | null): boolean;
  /**
   * Whether the node is a char; with `value`, whether it holds that specific
   * character.
   *
   * @param value - Optional character to match; omit/`null` matches any char.
   * @returns `true` if the node is a (matching) char.
   */
  isChar(value?: string | null): boolean;
  /**
   * Whether the node is an `Error` placeholder.
   *
   * @returns `true` for an `Error` node.
   */
  isError(): boolean;
  /**
   * The parent {@link Node}, or `null` for the root.
   *
   * @returns The parent handle, or `null`.
   */
  parent(): Node | null;
  /**
   * The child {@link Node}s, as a read-only property.
   *
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\frac{x}{y} + a`).document;
   * doc.root().children.map((n) => n.kind); // ['command', 'char', 'char']
   * ```
   */
  readonly children: Node[];
  /**
   * The following sibling {@link Node}, or `null`.
   *
   * @returns The next sibling handle, or `null`.
   */
  nextSibling(): Node | null;
  /**
   * The preceding sibling {@link Node}, or `null`.
   *
   * @returns The previous sibling handle, or `null`.
   */
  prevSibling(): Node | null;
  /**
   * The ancestor {@link Node}s from parent upward.
   *
   * @returns The ancestor handles, nearest first.
   */
  ancestors(): Node[];
  /**
   * The descendant {@link Node}s in document order.
   *
   * @returns The descendant handles.
   */
  descendants(): Node[];
  /**
   * The command name for a `command` / `declarative` node, else `null`
   * (read-only property).
   */
  readonly commandName: string | null;
  /** The environment name for an `environment` node, else `null` (read-only property). */
  readonly envName: string | null;
  /** The string value for a `text` node, else `null` (read-only property). */
  readonly text: string | null;
  /** The character for a `char` node, else `null` (read-only property). */
  readonly char: string | null;
  /**
   * The prime count (greater than zero) for a `prime` node, else `null`.
   *
   * @returns The number of consecutive prime marks, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`f''`).document;
   * const prime = doc.root().descendants().find((n) => n.kind === 'prime');
   * prime.primeCount(); // 2
   * ```
   */
  primeCount(): number | null;
  /**
   * For an `error` node, its captured `{ message, snippet }`, else `null`.
   *
   * @returns The error message and the original source snippet, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\sqrt[`).document;
   * const err = doc.root().descendants().find((n) => n.isError());
   * err.errorParts(); // { message: 'unclosed bracket argument', snippet: '\\sqrt[' }
   * ```
   */
  errorParts(): { message: string; snippet: string } | null;
  /**
   * The runtime content mode (`"math"` or `"text"`) where applicable, else
   * `null`.
   *
   * @returns The node's content mode, or `null`.
   */
  contentMode(): RuntimeContentMode | null;
  /**
   * The {@link GroupKindRef} for a `group` node, else `null`.
   *
   * The result carries a capitalized `kind` discriminator (`'Explicit'`,
   * `'Implicit'`, `'InlineMath'`, `'Delimited'`). A `Delimited` group also
   * carries `left` and `right` {@link DelimiterValue}s.
   *
   * @returns The group kind view, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`{x}`).document;
   * const group = doc.root().descendants().find((n) => n.kind === 'group');
   * group.groupKind(); // { kind: 'Explicit' }
   * ```
   */
  groupKind(): GroupKindRef | null;
  /**
   * The number of argument slots for a command-like node.
   *
   * @returns The slot count.
   */
  argCount(): number;
  /**
   * The argument at `index`, or `null` if the slot is empty.
   *
   * @param index - The zero-based slot index.
   * @returns The {@link ArgRef} for the slot, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`\frac{x}{y}`).document;
   * const frac = doc.root().descendants().find((n) => n.isCommand('frac'));
   * frac.argCount(); // 2
   * frac.arg(0);     // { kind: 'Math', node: <Node> }
   * ```
   */
  arg(index: number): ArgRef | null;
  /**
   * The full array of argument slots, with `null` for any empty slot.
   *
   * @returns The {@link ArgRef} per slot, `null` where empty.
   */
  argSlots(): Array<ArgRef | null>;
  /**
   * The base of a `scripted` node, else `null`.
   *
   * @returns The base node handle, or `null`.
   */
  scriptBase(): Node | null;
  /**
   * The subscript slot of a `scripted` node, else `null`.
   *
   * @returns The subscript node handle, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`x_i^2`).document;
   * const sc = doc.root().descendants().find((n) => n.kind === 'scripted');
   * [sc.subscript().kind, sc.superscript().kind]; // ['char', 'char']
   * ```
   */
  subscript(): Node | null;
  /**
   * The superscript slot of a `scripted` node, else `null`.
   *
   * @returns The superscript node handle, or `null`.
   */
  superscript(): Node | null;
  /**
   * The left operand of an `infix` node, else `null`.
   *
   * @returns The left operand handle, or `null`.
   */
  infixLeft(): Node | null;
  /**
   * The right operand of an `infix` node, else `null`.
   *
   * @returns The right operand handle, or `null`.
   */
  infixRight(): Node | null;
  /**
   * The body group of an `environment` node, else `null`.
   *
   * @returns The body group handle, or `null`.
   */
  envBody(): Node | null;
  /**
   * The source byte {@link Span} for the node, or `null` if it has no recorded
   * span.
   *
   * @returns The span, or `null`.
   * @example
   * ```ts
   * const doc = new Parser().parse(String.raw`a + b`).document;
   * doc.root().children[0].span(); // { start: 0, end: 1 }
   * ```
   */
  span(): Span | null;
}

/**
 * Base class for every error the library throws.
 *
 * `TexformError` extends the built-in `Error`, so a single `catch` with an
 * `instanceof TexformError` check catches all library errors. The {@link kind}
 * discriminator identifies which subsystem raised it.
 *
 * @see {@link TexformParseError}
 * @see {@link TexformEditError}
 * @see {@link TexformConfigError}
 * @see {@link TexformTransformError}
 */
export class TexformError extends Error {
  /**
   * Discriminator naming the subsystem that raised the error: `"parse"`,
   * `"edit"`, `"config"`, `"transform"`, or `"internal"`.
   */
  readonly kind: "parse" | "edit" | "config" | "transform" | "internal";
}

/**
 * Thrown when an operation requires a complete tree but parsing produced none.
 *
 * {@link TransformEngine.normalize} throws this on input that cannot produce a
 * complete tree. Note that {@link Parser.parse} itself never throws — it
 * returns a {@link ParseResult} instead.
 */
export class TexformParseError extends TexformError {
  /** The diagnostics describing why parsing could not produce a complete tree. */
  diagnostics: ParseDiagnostic[];
  /** The partial {@link Document} that was recovered, or `null` if none. */
  document: Document | null;
}

/**
 * Thrown by {@link Document} editing methods on an invalid edit: editing a
 * read-only (error) tree, detaching or removing the root, an out-of-bounds
 * index, or mixing nodes across documents.
 */
export class TexformEditError extends TexformError {}

/**
 * Thrown on invalid construction input, such as an unknown package name or an
 * unknown transform profile passed to {@link Parser} or
 * {@link TransformEngine}.
 */
export class TexformConfigError extends TexformError {}

/**
 * Thrown on a transform-engine failure, such as an eliminated-form contract
 * violation, or passing a foreign document to
 * {@link TransformEngine.transform}.
 */
export class TexformTransformError extends TexformError {}

/**
 * The content modes a command, environment, or character is allowed in:
 * `"math"`, `"text"`, or `"both"`.
 */
export type AllowedMode = "math" | "text" | "both";

/**
 * The syntactic role of a command in the knowledge base: `"prefix"` (takes its
 * arguments after the control sequence), `"infix"` (binds operands on both
 * sides, like `\over`), or `"declarative"` (a scope-affecting declaration).
 */
export type CommandKind = "prefix" | "infix" | "declarative";

/**
 * The value kind one parsed argument slot accepts, with a `type` discriminator.
 *
 * `content` carries the {@link RuntimeContentMode} its body parses in; the
 * other variants are leaf kinds (operatorname, delimiter, csname, dimension,
 * integer, keyval, column, star).
 *
 * @see {@link ParsedArgSpecSlot}
 */
export type ArgSpecKindInfo =
  | { type: "content"; mode: RuntimeContentMode }
  | { type: "operatorname" }
  | { type: "delimiter" }
  | { type: "csname" }
  | { type: "dimension" }
  | { type: "integer" }
  | { type: "keyval" }
  | { type: "column" }
  | { type: "star" };

/**
 * A single delimiter token in an argspec form: a literal `char` (such as `(`)
 * or a `control-seq` (such as `langle`).
 */
export type DelimiterTokenInfo =
  | { type: "char"; value: string }
  | { type: "control-seq"; value: string };

/**
 * The syntactic form one parsed argument slot takes, with a `type`
 * discriminator: a `standard` argument, a `star` flag, a brace `group`, a
 * `delimited` argument bounded by an `open`/`close` pair, or a `paired` argument
 * accepting any of several interchangeable delimiter `pairs`.
 *
 * @see {@link ParsedArgSpecSlot}
 */
export type ArgSpecFormInfo =
  | { type: "standard" }
  | { type: "star" }
  | { type: "group" }
  | { type: "delimited"; open: DelimiterTokenInfo; close: DelimiterTokenInfo }
  | { type: "paired"; pairs: Array<{ open: DelimiterTokenInfo; close: DelimiterTokenInfo }> };

/**
 * One parsed argument slot of an argspec, as reported by
 * {@link validateArgspec} and by the parser's command/environment lookups.
 */
export interface ParsedArgSpecSlot {
  /** Whether the argument is mandatory (`true`) or optional (`false`). */
  required: boolean;
  /** Whether leading space before the argument is disallowed. */
  noLeadingSpace: boolean;
  /** Whether an absent optional argument yields a null value. */
  nullable: boolean;
  /** The value kind the slot accepts. */
  kind: ArgSpecKindInfo;
  /** The syntactic form the slot takes. */
  form: ArgSpecFormInfo;
}

/**
 * The knowledge-base entry for a command, returned by
 * {@link Parser.lookupCommand} and related lookups.
 */
export interface CommandInfo {
  /** The command name, without the leading backslash. */
  name: string;
  /** The command's syntactic role (prefix, infix, declarative). */
  kind: CommandKind;
  /** The modes the command is allowed in. */
  allowedMode: AllowedMode;
  /** The raw xparse-style argument-specification string. */
  specString: string;
  /** The knowledge packages that define this command. */
  fromPackages: string[];
  /** Free-form classification tags. */
  tags: string[];
  /** The parsed argument slots, mirroring {@link validateArgspec} output. */
  args: ParsedArgSpecSlot[];
}

/**
 * The knowledge-base entry for an environment, returned by
 * {@link Parser.lookupEnv}.
 */
export interface EnvInfo {
  /** The environment name. */
  name: string;
  /** The modes the environment is allowed in. */
  allowedMode: AllowedMode;
  /** The content mode the environment body is parsed in. */
  bodyMode: RuntimeContentMode;
  /** The raw xparse-style argument-specification string. */
  specString: string;
  /** The knowledge packages that define this environment. */
  fromPackages: string[];
  /** Free-form classification tags. */
  tags: string[];
  /** The parsed argument slots for the environment's arguments. */
  args: ParsedArgSpecSlot[];
}

/**
 * Rendering attributes for a special character in the knowledge base.
 */
export interface CharacterAttributesInfo {
  /** The MathML `mathvariant` the character renders with, or `null`. */
  mathvariant: string | null;
}

/**
 * The knowledge-base entry for a special character, returned by
 * {@link Parser.lookupCharacter}.
 */
export interface CharacterInfo {
  /** The character name, without the leading backslash. */
  name: string;
  /** The modes the character is allowed in. */
  allowedMode: AllowedMode;
  /** The Unicode code point the character maps to, as a string. */
  unicodeValue: string;
  /** Rendering attributes such as `mathvariant`. */
  attributes: CharacterAttributesInfo;
  /** The knowledge package that defines this character. */
  package: string;
}

/**
 * One custom knowledge entry injected into a {@link Parser} or
 * {@link TransformEngine} through the `items` option, discriminated by
 * `target`.
 *
 * A `command` entry carries its kind, allowed mode, and `argspec`; an
 * `environment` entry additionally carries its `bodyMode`; a `delimiter` entry
 * registers a delimiter-control name.
 */
export type ContextItem =
  | {
      target: "command";
      name: string;
      kind: CommandKind;
      allowedMode: AllowedMode;
      argspec: string;
      tags?: string[];
    }
  | {
      target: "environment";
      name: string;
      allowedMode: AllowedMode;
      bodyMode: RuntimeContentMode;
      argspec: string;
      tags?: string[];
    }
  | {
      target: "delimiter";
      name: string;
    };

/**
 * Spacing around control sequences in serialized output: `"spaced"` pads around
 * them; `"minimal"` emits only the separation LaTeX requires.
 */
export type CommandSpacing = "spaced" | "minimal";

/**
 * Inner spacing of math groups: `"padded"` writes `{ x }`; `"compact"` writes
 * `{x}`.
 */
export type MathGroupInnerSpacing = "padded" | "compact";

/**
 * Spacing between adjacent characters: `"spaced"` writes `a b c`; `"compact"`
 * writes `abc`.
 */
export type AdjacentCharSpacing = "spaced" | "compact";

/**
 * Spacing around script operators: `"spaced"` writes `x _ { i } ^ { 2 }`;
 * `"compact"` writes `x_{ i }^{ 2 }`.
 */
export type ScriptSpacing = "spaced" | "compact";

/**
 * Order of subscript and superscript in serialized output: `"sub_first"` writes
 * `x _ { i } ^ { 2 }`; `"sup_first"` writes `x ^ { 2 } _ { i }`. The values stay
 * snake_case to match the Python binding.
 */
export type ScriptOrder = "sub_first" | "sup_first";

/**
 * Spacing after `\begin` / `\end`: `"spaced"` writes `\begin {matrix}`;
 * `"compact"` writes `\begin{matrix}`.
 */
export type EnvironmentNameSpacing = "spaced" | "compact";

/**
 * Math spacing options for the serializer. Omitted keys keep their default.
 *
 * @see {@link MathSerializeOptions}
 */
export interface MathSpacingOptions {
  /** Spacing around control sequences. Default `"spaced"`. */
  commands?: CommandSpacing;
  /** Inner spacing of math groups. Default `"padded"`. */
  groupInnerSpacing?: MathGroupInnerSpacing;
  /** Spacing between adjacent characters. Default `"spaced"`. */
  adjacentChars?: AdjacentCharSpacing;
}

/**
 * Math script options for the serializer. Omitted keys keep their default.
 *
 * @see {@link MathSerializeOptions}
 */
export interface MathScriptOptions {
  /** Spacing around script operators. Default `"spaced"`. */
  spacing?: ScriptSpacing;
  /** Order of subscript and superscript. Default `"sub_first"`. */
  order?: ScriptOrder;
}

/**
 * Math-mode serialization options, grouping spacing and script axes.
 *
 * @see {@link SerializeOptions}
 */
export interface MathSerializeOptions {
  /** Spacing axes (commands, group inner spacing, adjacent chars). */
  spacing?: MathSpacingOptions;
  /** Script axes (spacing and order). */
  scripts?: MathScriptOptions;
}

/**
 * Serialization options for environment markup.
 *
 * @see {@link SyntaxSerializeOptions}
 */
export interface EnvironmentSerializeOptions {
  /** Spacing after `\begin` / `\end`. Default `"spaced"`. */
  nameSpacing?: EnvironmentNameSpacing;
}

/**
 * Serialization options for syntactic (non-math-spacing) constructs.
 *
 * @see {@link SerializeOptions}
 */
export interface SyntaxSerializeOptions {
  /** Environment serialization options. */
  environments?: EnvironmentSerializeOptions;
}

/**
 * Options controlling serialized LaTeX output style.
 *
 * A nested object keyed by camelCase names. An unrecognized key — including a
 * snake_case key meant for the Python binding — is silently ignored, so the
 * corresponding axis keeps its default. Passed to {@link Document.toLatex} and
 * {@link serialize}. For a task-oriented walkthrough, see the Serialization
 * guide.
 *
 * @example
 * ```ts
 * const result = new Parser().parse(String.raw`x_i^2`);
 * if (!result.document) throw new Error('parse failed');
 * const syntax = result.document.toSyntax();
 * serialize(syntax, { math: { scripts: { order: 'sup_first' } } }); // 'x ^ { 2 } _ { i }'
 * ```
 */
export interface SerializeOptions {
  /** Math-mode spacing and script options. */
  math?: MathSerializeOptions;
  /** Syntactic (environment) options. */
  syntax?: SyntaxSerializeOptions;
}

/**
 * Phase-oriented report describing how a normalization run changed the tree.
 *
 * Returned by {@link TransformEngine.transform} and carried on the `report`
 * field of {@link TransformResult}. Field names are camelCase
 * (`iterations`, `rules`, `finalizeAst`, `flattenGroups`, `lowerAttributes`),
 * the JavaScript view of the same data the Python binding exposes in
 * snake_case.
 */
export interface TransformReport {
  /** Fixed-point rewrite iterations for this normalization run. */
  iterations: number;
  /** Rewrite rules that were attempted, sorted by stable rule key. */
  rules: Array<{ key: string; appliedCount: number; skippedCount: number }>;
  /** The FinalizeAst phase report (local AST cleanup after rewriting). */
  finalizeAst: {
    steps: {
      /** Adjacent `Prime` nodes merged into one. */
      mergeAdjacentPrimes: {
        appliedCount: number;
      };
      /** Text-sequence merge, whitespace collapse, and empty-text cleanup. */
      normalizeTextSequences: {
        appliedCount: number;
      };
    };
  };
  /** The LowerAttributes phase report (font/style canonicalization). */
  lowerAttributes: {
    /** Use the `(attr, value)` pair as the stable lower-attribute unit key. */
    attributes: Array<{
      attr: string;
      value: string;
      /** Input forms consumed by LowerAttributes. */
      consumed: AttributeFormCounts;
      /** Consumed forms that did not change the emitted attribute state. */
      redundant: AttributeFormCounts;
      /** Canonical forms emitted by LowerAttributes. */
      emitted: AttributeFormCounts;
    }>;
    eliminatedEmptySegments: number;
  };
  /** The FlattenGroups phase report (redundant-brace removal). */
  flattenGroups: {
    /** How many flatten actions of each kind fired. */
    actions: {
      removedEmpty: number;
      replacedSingleChild: number;
      inlinedMultiChild: number;
      unwrappedSlot: number;
    };
    /** Preserve-guard hit counters; each guard blocks a flatten action in a specific context. */
    guards: {
      preserveGroupContainingDeclarativeCommand: number;
      preserveGroupInScriptBaseSlot: number;
      preserveGroupInsideEnvBody: number;
      preserveGroupContainingInfix: number;
      preserveGroupAdjacentToCommandLike: number;
      preserveGroupAsArgumentOfCommand: number;
      preserveGroupAfterScriptedCommandLike: number;
      preserveEmptyGroup: number;
      preserveGroupWithLoneAtomSpacingChar: number;
      preserveGroupStartingWithAtomSpacingChar: number;
      preserveGroupContainingDelimitedPair: number;
    };
  };
}

/**
 * Counts of an attribute form split by carrier: `declaratives` (scope-affecting
 * declarations such as `{\bf ...}`) and `prefixes` (prefix commands such as
 * `\mathbf{...}`).
 */
export interface AttributeFormCounts {
  /** Count carried by declarative-form markup. */
  declaratives: number;
  /** Count carried by prefix-command markup. */
  prefixes: number;
}

/**
 * The result of {@link TransformEngine.normalize}: the canonical LaTeX string
 * plus the phase-oriented {@link TransformReport}.
 */
export interface TransformResult {
  /** The canonical LaTeX after parsing and normalization. */
  normalized: string;
  /** The report describing which phases and rules changed the tree. */
  report: TransformReport;
}

/**
 * The outcome of {@link validateArgspec}: whether the spec is well-formed and,
 * when valid, its parsed slots.
 *
 * On success `valid` is `true` and `argCount` / `parsed` describe the slots; on
 * failure `valid` is `false` and `error` carries the reason.
 */
export interface ValidateArgspecResult {
  /** Whether the specification parsed successfully. */
  valid: boolean;
  /** The error message when invalid, otherwise `null`. */
  error: string | null;
  /** The number of argument slots when valid, otherwise `null`. */
  argCount: number | null;
  /** The per-slot breakdown when valid, otherwise `null`. */
  parsed: ParsedArgSpecSlot[] | null;
}

/**
 * Per-run switches for the LowerAttributes phase.
 *
 * @see {@link TransformConfigInput}
 */
export interface LowerAttributesConfigInput {
  /** Whether the phase runs. Defaults to the profile's setting. */
  enabled?: boolean;
}

/**
 * Per-run switches for the fixed-point Rewrite phase.
 *
 * @see {@link TransformConfigInput}
 */
export interface RewriteConfigInput {
  /** Whether the phase runs. When `false`, legacy syntax is left untouched. */
  enabled?: boolean;
  /** Cap on Rewrite fixed-point passes. */
  maxIterations?: number;
}

/**
 * Per-run switches for the FinalizeAst phase.
 *
 * @see {@link TransformConfigInput}
 */
export interface FinalizeAstConfigInput {
  /** Whether the phase runs. Defaults to enabled in every public profile. */
  enabled?: boolean;
}

/**
 * Per-run switches for the FlattenGroups phase (redundant-brace removal).
 *
 * `enabled` governs whether the phase runs; each `preserve*` guard, when
 * `true`, keeps a group matching the named structural condition instead of
 * flattening it. Omitted keys fall back to the profile's defaults — for
 * example, `corpus` turns several guards off.
 *
 * @see {@link TransformConfigInput}
 */
export interface FlattenGroupsConfigInput {
  /** Whether the phase runs. */
  enabled?: boolean;
  /** Keep a group that is empty. */
  preserveEmptyGroup?: boolean;
  /** Keep a group adjacent to a command-like node. */
  preserveGroupAdjacentToCommandLike?: boolean;
  /** Keep a group following a scripted command-like node. */
  preserveGroupAfterScriptedCommandLike?: boolean;
  /** Keep a group containing a declarative command. */
  preserveGroupContainingDeclarativeCommand?: boolean;
  /** Keep a group containing a delimited pair. */
  preserveGroupContainingDelimitedPair?: boolean;
  /** Keep a group containing an infix operator. */
  preserveGroupContainingInfix?: boolean;
  /** Keep a group occupying a script base slot. */
  preserveGroupInScriptBaseSlot?: boolean;
  /** Keep a group inside an environment body. */
  preserveGroupInsideEnvBody?: boolean;
  /** Keep a group that starts with an atom-spacing character. */
  preserveGroupStartingWithAtomSpacingChar?: boolean;
  /** Keep a group whose sole child is an atom-spacing character. */
  preserveGroupWithLoneAtomSpacingChar?: boolean;
}

/**
 * The nested, per-phase transform configuration, overriding a profile's
 * transform defaults.
 *
 * Each field controls one pipeline phase. This is the transform-only shape; it
 * does not accept parser-strictness keys (those belong to
 * {@link NormalizeOptions}).
 *
 * @see {@link TransformOptions}
 */
export interface TransformConfigInput {
  /** LowerAttributes phase switches. */
  lowerAttributes?: LowerAttributesConfigInput;
  /** Rewrite phase switches. */
  rewrite?: RewriteConfigInput;
  /** FinalizeAst phase switches. */
  finalizeAst?: FinalizeAstConfigInput;
  /** FlattenGroups phase switches. */
  flattenGroups?: FlattenGroupsConfigInput;
}

/**
 * Options accepted by {@link TransformEngine.transform}. An alias of
 * {@link TransformConfigInput} — the same nested per-phase shape, with no parse
 * options.
 */
export type TransformOptions = TransformConfigInput;

/**
 * Normalization profile passed to {@link TransformEngineOptions}: `"authoring"`,
 * `"faithful"`, `"corpus"`, or `"equiv"`.
 *
 * @see {@link TransformProfile}
 */
export type Profile = "authoring" | "faithful" | "corpus" | "equiv";

/**
 * Knowledge-base options shared by {@link Parser} and {@link TransformEngine}
 * construction.
 *
 * Omit `packages` to load the default runtime packages, not every package in
 * the catalog. Use `listPackages()` to see the available names; an unknown name
 * throws {@link TexformConfigError}.
 */
export interface ParserOptions {
  /** Package names to load; omit to load the default runtime packages. */
  packages?: string[];
  /** Custom command/environment/delimiter-control knowledge to inject. */
  items?: ContextItem[];
  /** Command names to drop from the loaded knowledge. */
  removeCommands?: string[];
  /** Environment names to drop from the loaded knowledge. */
  removeEnvironments?: string[];
  /** Delimiter-control names to drop from the loaded knowledge. */
  removeDelimiterControls?: string[];
}

/**
 * Construction options for {@link TransformEngine}: the knowledge-base options
 * plus the required normalization {@link Profile} and optional rule disabling.
 */
export interface TransformEngineOptions extends ParserOptions {
  /** The normalization profile (required). Unknown values throw {@link TexformConfigError}. */
  profile: Profile;
  /** Rewrite rule keys to disable, such as `"physics/dv-to-frac-d"`. */
  disableRules?: string[];
}

/**
 * Per-run options for {@link TransformEngine.normalize}, overriding the
 * profile's defaults.
 *
 * A single flat object (the JavaScript binding has no `TransformConfig` class).
 * It extends {@link ParseConfigInput}, so it also accepts the parser-strictness
 * keys (`rejectUnknown`, `abortOnError`, `maxGroupDepth`), which apply to the
 * parse that precedes normalization.
 *
 * @example
 * ```ts
 * const engine = new TransformEngine({ profile: 'corpus' });
 * engine.normalize(String.raw`a \over b`, {
 *   maxIterations: 50,
 *   flattenGroups: { enabled: false },
 * });
 * ```
 */
export interface NormalizeOptions extends ParseConfigInput {
  /** FlattenGroups phase switches. */
  flattenGroups?: FlattenGroupsConfigInput;
  /** FinalizeAst phase switches. */
  finalizeAst?: FinalizeAstConfigInput;
  /** Whether the fixed-point Rewrite phase runs. Default `true`. */
  rewriteEnabled?: boolean;
  /** Whether font/style canonicalization runs. Default `true`. */
  lowerAttributesEnabled?: boolean;
  /** Cap on Rewrite fixed-point passes. Default `100`. */
  maxIterations?: number;
}

/**
 * A knowledge-driven LaTeX parser.
 *
 * `Parser` turns LaTeX source into a {@link ParseResult} by consulting the
 * knowledge base for command and environment signatures. It does not normalize
 * — use {@link TransformEngine} for that. Parsing never throws on malformed
 * input; failures surface in the result. For the conceptual model, see the
 * Parsing guide.
 *
 * @see {@link TransformEngine}
 * @example
 * ```ts
 * import { Parser } from 'texform';
 *
 * const parser = new Parser();
 * const restricted = new Parser({ packages: ['base', 'ams'] });
 * ```
 */
export class Parser {
  /**
   * Construct a parser, optionally restricting packages or injecting and
   * removing knowledge entries.
   *
   * @param options - A {@link ParserOptions} object, or omit/`null` to load
   *   the default runtime packages with no customization.
   */
  constructor(options?: ParserOptions | null);
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Whether `name` is a delimiter-control command (such as `langle`).
   *
   * @param name - The command name, without the leading backslash.
   * @returns `true` if it is a delimiter control.
   * @example
   * ```ts
   * new Parser().isDelimiterControl('langle'); // true
   * ```
   */
  isDelimiterControl(name: string): boolean;
  /**
   * Whether a command named `name` is known in any mode.
   *
   * @param name - The command name, without the leading backslash.
   * @returns `true` if the command is known.
   * @example
   * ```ts
   * new Parser().knowsCommandName('frac'); // true
   * ```
   */
  knowsCommandName(name: string): boolean;
  /**
   * Whether an environment named `name` is known in any mode.
   *
   * @param name - The environment name.
   * @returns `true` if the environment is known.
   */
  knowsEnvName(name: string): boolean;
  /**
   * Whether a special character named `name` is known in any mode.
   *
   * @param name - The character name, without the leading backslash.
   * @returns `true` if the character is known.
   */
  knowsCharacterName(name: string): boolean;
  /**
   * Look up the {@link CharacterInfo} for a character in a given mode.
   *
   * @param name - The character name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   */
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | null;
  /**
   * Look up the {@link CommandInfo} for a command in a given mode.
   *
   * Resolves through any mode-specific overrides, so the returned record is the
   * one the parser would actually use in `mode`.
   *
   * @param name - The command name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   * @example
   * ```ts
   * new Parser().lookupCommand('frac', 'math');
   * // { name: 'frac', kind: 'prefix', allowedMode: 'math', specString: 'm m', ... }
   * ```
   */
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  /**
   * Look up the {@link EnvInfo} for an environment in a given mode.
   *
   * @param name - The environment name.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   */
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | null;
  /**
   * Look up only an explicit, non-character-derived command.
   *
   * Unlike {@link Parser.lookupCommand}, this does not return the zero-arg
   * command view projected from character metadata.
   *
   * @param name - The command name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if not explicitly defined in that
   *   mode.
   */
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  /**
   * Parse a LaTeX string into a {@link ParseResult}.
   *
   * Never throws on malformed input: the result has one of three honest states
   * (no tree, complete tree, or partial tree with `Error` nodes). Empty input
   * (`''`) yields a clean, complete document, not `null`.
   *
   * @param src - The LaTeX source string.
   * @param config - A {@link ParseConfigInput} object, or omit/`null` for the
   *   defaults.
   * @returns The parse result (`document` and `diagnostics`).
   * @example
   * ```ts
   * const result = new Parser().parse(String.raw`\frac{x}{y}`);
   * const document = result.document;
   * const diagnostics = result.diagnostics;
   * ```
   */
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
}

/**
 * A profile-based normalization engine: a parser paired with a transform
 * pipeline.
 *
 * `TransformEngine` normalizes a formula into the canonical form selected by a
 * {@link Profile}. It exposes a string-to-string {@link TransformEngine.normalize}
 * path and an in-place {@link TransformEngine.transform} path over a live
 * {@link Document}. It also exposes its own {@link TransformEngine.parse} and the
 * same knowledge-base lookups as {@link Parser}. For the conceptual model —
 * profiles, the multi-phase pipeline, and the eliminated-form contract — see the
 * Transforms guide.
 *
 * @see {@link Parser}
 * @see {@link Document}
 * @example
 * ```ts
 * import { TransformEngine } from 'texform';
 *
 * const engine = new TransformEngine({ profile: 'corpus' });
 * ```
 */
export class TransformEngine {
  /**
   * Construct an engine for a profile, optionally restricting packages,
   * injecting context items, or disabling rules.
   *
   * @param options - A {@link TransformEngineOptions} object. `profile` is
   *   required; an unknown profile throws {@link TexformConfigError}.
   */
  constructor(options: TransformEngineOptions);
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Parse a LaTeX string into a {@link ParseResult}, using this engine's parser.
   *
   * Non-null documents from this result keep the engine's parser identity, so
   * they can be edited and then passed to {@link TransformEngine.transform}. The
   * The engine parser defaults to lenient parsing; pass `config` to override it
   * per call.
   *
   * @param src - The LaTeX source string.
   * @param config - A {@link ParseConfigInput} object, or omit/`null` for the
   *   defaults.
   * @returns The parse result (`document` and `diagnostics`).
   */
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
  /**
   * Parse, normalize, and serialize a formula in one call.
   *
   * Normalization is gated on a complete tree: if the input cannot produce one,
   * this throws {@link TexformParseError} carrying its `diagnostics` and the
   * partial `document`. Empty input is complete and normalizes normally.
   *
   * @param src - The LaTeX source string.
   * @param options - A {@link NormalizeOptions} object overriding the profile's
   *   defaults, or omit/`null` to use them.
   * @returns The normalized string and its {@link TransformReport}.
   * @example
   * ```ts
   * const engine = new TransformEngine({ profile: 'corpus' });
   * engine.normalize(String.raw`\dv{f}{x}`).normalized;
   * // '\\frac { \\mathrm { d } f } { \\mathrm { d } x }'
   * ```
   */
  normalize(src: string, options?: NormalizeOptions | null): TransformResult;
  /**
   * Transform a live {@link Document} in place and return the report.
   *
   * The document must have come from this engine's {@link TransformEngine.parse}
   * (it carries the matching parser identity). A document created with
   * `new Document()` or {@link Document.fromSyntax} can be edited and
   * serialized, but `transform` rejects it with {@link TexformTransformError}.
   * A document that {@link Document.hasErrors} is read-only and cannot be
   * transformed; this precondition error is surfaced as {@link TexformError}.
   *
   * @param document - The live document to update in place.
   * @param options - A {@link TransformOptions} object overriding the profile's
   *   transform defaults, or omit/`null` to use them. It uses the nested
   *   per-phase shape and does not accept parse options.
   * @returns The phase-oriented transform report.
   * @example
   * ```ts
   * const engine = new TransformEngine({ profile: 'corpus' });
   * const result = engine.parse(String.raw`a \over b`);
   * if (result.document) {
   *   const report = engine.transform(result.document);
   *   result.document.toLatex(); // '\\frac { a } { b }'
   *   report.rules.some((rule) => rule.key === 'base/over-to-frac'); // true
   * }
   * ```
   */
  transform(document: Document, options?: TransformOptions | null): TransformReport;
  /**
   * Whether `name` is a delimiter-control command. See
   * {@link Parser.isDelimiterControl}.
   *
   * @param name - The command name, without the leading backslash.
   * @returns `true` if it is a delimiter control.
   */
  isDelimiterControl(name: string): boolean;
  /**
   * Whether a command named `name` is known in any mode. See
   * {@link Parser.knowsCommandName}.
   *
   * @param name - The command name, without the leading backslash.
   * @returns `true` if the command is known.
   */
  knowsCommandName(name: string): boolean;
  /**
   * Whether an environment named `name` is known in any mode. See
   * {@link Parser.knowsEnvName}.
   *
   * @param name - The environment name.
   * @returns `true` if the environment is known.
   */
  knowsEnvName(name: string): boolean;
  /**
   * Whether a special character named `name` is known in any mode. See
   * {@link Parser.knowsCharacterName}.
   *
   * @param name - The character name, without the leading backslash.
   * @returns `true` if the character is known.
   */
  knowsCharacterName(name: string): boolean;
  /**
   * Look up the {@link CharacterInfo} for a character in a given mode. See
   * {@link Parser.lookupCharacter}.
   *
   * @param name - The character name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   */
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | null;
  /**
   * Look up the {@link CommandInfo} for a command in a given mode. See
   * {@link Parser.lookupCommand}.
   *
   * @param name - The command name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   */
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  /**
   * Look up the {@link EnvInfo} for an environment in a given mode. See
   * {@link Parser.lookupEnv}.
   *
   * @param name - The environment name.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if unknown in that mode.
   */
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | null;
  /**
   * Look up only an explicit, non-character-derived command. See
   * {@link Parser.lookupExplicitCommand}.
   *
   * @param name - The command name, without the leading backslash.
   * @param mode - The content mode, `"math"` or `"text"`.
   * @returns The knowledge entry, or `null` if not explicitly defined in that
   *   mode.
   */
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
}

/**
 * Render a {@link SyntaxNode} object to LaTeX text using the canonical
 * serializer.
 *
 * `Error` nodes round-trip their captured snippet, and pure prime superscripts
 * serialize compactly as `f'` or `f''`. The serializer guarantees text
 * idempotency.
 *
 * @deprecated Prefer `Document.fromSyntax(node).toLatex(options)` or
 * `document.toLatex(options)`. This remains exported as a compatibility helper
 * for older snapshot call sites and takes the same options as `toLatex`.
 *
 * @param node - A {@link SyntaxNode} object, typically from
 *   {@link Document.toSyntax} or a stored snapshot.
 * @param options - A {@link SerializeOptions} object, or omit/`null` for the
 *   default (spaced) style.
 * @returns The canonical LaTeX string.
 * @example
 * ```ts
 * import { Parser, serialize } from 'texform';
 *
 * const result = new Parser().parse(String.raw`x^2`);
 * if (!result.document) throw new Error('parse failed');
 * const syntax = result.document.toSyntax();
 * serialize(syntax);                                                // 'x ^ { 2 }'
 * serialize(syntax, { math: { scripts: { spacing: 'compact' } } }); // 'x^{ 2 }'
 * ```
 */
export function serialize(node: SyntaxNode, options?: SerializeOptions | null): string;

/**
 * Validate an xparse-style argspec string and report its parsed slots.
 *
 * Use it to self-check an argspec before injecting a custom command into a
 * {@link Parser}. Never throws on a malformed spec: the failure is reported
 * through the `valid` / `error` fields of the result. For the notation
 * semantics, see the Argspec guide.
 *
 * @param spec - The argspec string, such as `'o m'`, `'s m{}'`, or
 *   `` `d<(,)><[,]>` ``.
 * @returns A {@link ValidateArgspecResult} describing validity and slots.
 * @example
 * ```ts
 * import { validateArgspec } from 'texform';
 *
 * validateArgspec('o m'); // { valid: true, error: null, argCount: 2, parsed: [ ... ] }
 * validateArgspec('ABC123').valid; // false
 * ```
 */
export function validateArgspec(spec: string): ValidateArgspecResult;

/**
 * Summary of one built-in knowledge package, returned by {@link listPackages}.
 */
export interface PackageInfo {
  /** The package identifier, accepted by the `packages` option. */
  name: string;
  /** Number of command records in the package. */
  commands: number;
  /** Number of environment records in the package. */
  environments: number;
}

/**
 * List all built-in knowledge packages with record counts.
 *
 * The returned names are the identifiers accepted by the `packages` option of
 * {@link Parser} and {@link TransformEngine}: `ams`, `base`, `bboldx`,
 * `boldsymbol`, `braket`, `physics`, `textmacros`.
 *
 * @returns One {@link PackageInfo} per built-in package.
 * @example
 * ```ts
 * import { listPackages } from 'texform';
 *
 * listPackages(); // [ { name: 'ams', commands: 35, environments: 28 }, ... ]
 * ```
 */
export function listPackages(): PackageInfo[];
