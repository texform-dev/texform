export type ArgumentSlot = Argument | null | undefined;

export interface ParseConfigInput {
  rejectUnknown?: boolean;
  abortOnError?: boolean;
  maxGroupDepth?: number;
}

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

export type NormalizationLevel = "standard" | "expand" | "drop" | "equiv";
export type TransformProfile = "authoring" | "faithful" | "corpus" | "equiv";

export interface Span {
  start: number;
  end: number;
}

export interface ParseDiagnosticContext {
  label: string;
  span: Span;
}

export interface ParseDiagnostic {
  kind: ParseDiagnosticKind | null;
  message: string;
  span: Span;
  expected: string[];
  found: string | null;
  contexts: ParseDiagnosticContext[];
}

export type SyntaxContentMode = "Math" | "Text";
export type RuntimeContentMode = "math" | "text";

export type Delimiter = "None" | { Char: string } | { Control: string };
export type DelimiterValue =
  | { kind: "None" }
  | { kind: "Char"; value: string }
  | { kind: "Control"; value: string };

export type GroupKind =
  | "Explicit"
  | "Implicit"
  | { Delimited: { left: Delimiter; right: Delimiter } }
  | "InlineMath";

export type GroupKindRef =
  | { kind: "Explicit" }
  | { kind: "Implicit" }
  | { kind: "Delimited"; left: DelimiterValue; right: DelimiterValue }
  | { kind: "InlineMath" };

export type ArgumentKind =
  | "Mandatory"
  | "Optional"
  | "Star"
  | "Group"
  | { Delimited: { open: Delimiter; close: Delimiter } }
  | { Paired: { open: Delimiter; close: Delimiter } };

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

export interface Argument {
  kind: ArgumentKind;
  value: ArgumentValue;
}

export type ArgumentValue =
  | { MathContent: SyntaxNode }
  | { TextContent: SyntaxNode }
  | { Delimiter: Delimiter }
  | { CSName: string }
  | { Dimension: string }
  | { Integer: string }
  | { KeyVal: string }
  | { Column: string }
  | { Boolean: boolean };

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

export type ArgValueInput = ArgRef;

export interface ParseResult {
  document: Document | null;
  diagnostics: ParseDiagnostic[];
}

export class Document {
  constructor();
  static fromSyntax(node: SyntaxNode): Document;
  free(): void;
  [Symbol.dispose](): void;
  root(): Node;
  hasErrors(): boolean;
  isReadOnly(): boolean;
  errors(): Node[];
  findCommands(name: string): Node[];
  findEnvironments(name: string): Node[];
  createChar(value: string): Node;
  createText(value: string): Node;
  createActiveSpace(): Node;
  createGroup(mode: RuntimeContentMode): Node;
  createCommand(name: string, args?: ArgValueInput[] | null): Node;
  createDeclarative(name: string, args?: ArgValueInput[] | null): Node;
  createEnvironment(name: string, args: ArgValueInput[] | null | undefined, body: Node): Node;
  appendChild(parent: Node, child: Node): void;
  insertChild(parent: Node, index: number, child: Node): void;
  insertBefore(anchor: Node, node: Node): void;
  insertAfter(anchor: Node, node: Node): void;
  replaceWith(target: Node, replacement: Node): void;
  wrap(target: Node, wrapper: Node): Node;
  unwrap(group: Node): Node[];
  extract(node: Node): Node;
  remove(node: Node): void;
  clear(node: Node): void;
  setText(node: Node, value: string): void;
  setChar(node: Node, value: string): void;
  setCommandName(node: Node, name: string): void;
  setArg(node: Node, index: number, value: ArgValueInput): void;
  toSyntax(): SyntaxNode;
  /**
   * Export the parse-time span side table as a list of `{id, span}` entries.
   *
   * Ids follow the parser's tree-path scheme rooted at `root`: `.child.N` for
   * container children, `.arg.N.content` for content-carrying argument slots,
   * `.left` / `.right` for infix operands, `.body` for environment bodies, and
   * `.base` / `.sub` / `.sup` for script slots. Nodes without a recorded span
   * are omitted. Spans reflect the original parse and are not updated by edits.
   */
  nodeSpans(): NodeSpanEntry[];
  toLatex(options?: SerializeOptions | null): string;
}

export interface NodeSpanEntry {
  id: string;
  span: Span;
}

export class Node {
  free(): void;
  [Symbol.dispose](): void;
  readonly kind: NodeKind;
  isCommand(name?: string | null): boolean;
  isChar(value?: string | null): boolean;
  isError(): boolean;
  parent(): Node | null;
  readonly children: Node[];
  nextSibling(): Node | null;
  prevSibling(): Node | null;
  ancestors(): Node[];
  descendants(): Node[];
  readonly commandName: string | null;
  readonly envName: string | null;
  readonly text: string | null;
  readonly char: string | null;
  primeCount(): number | null;
  errorParts(): { message: string; snippet: string } | null;
  contentMode(): RuntimeContentMode | null;
  groupKind(): GroupKindRef | null;
  argCount(): number;
  arg(index: number): ArgRef | null;
  argSlots(): Array<ArgRef | null>;
  scriptBase(): Node | null;
  subscript(): Node | null;
  superscript(): Node | null;
  infixLeft(): Node | null;
  infixRight(): Node | null;
  envBody(): Node | null;
  span(): Span | null;
}

export class TexformError extends Error {
  readonly kind: "parse" | "edit" | "config" | "transform" | "internal";
}

export class TexformParseError extends TexformError {
  diagnostics: ParseDiagnostic[];
  document: Document | null;
}

export class TexformEditError extends TexformError {}

export class TexformConfigError extends TexformError {}

export class TexformTransformError extends TexformError {}

export type AllowedMode = "math" | "text" | "both";
export type CommandKind = "prefix" | "infix" | "declarative";

export type ArgSpecKindInfo =
  | { type: "content"; mode: RuntimeContentMode }
  | { type: "delimiter" }
  | { type: "csname" }
  | { type: "dimension" }
  | { type: "integer" }
  | { type: "keyval" }
  | { type: "column" }
  | { type: "star" };

export type DelimiterTokenInfo =
  | { type: "char"; value: string }
  | { type: "control-seq"; value: string };

export type ArgSpecFormInfo =
  | { type: "standard" }
  | { type: "star" }
  | { type: "group" }
  | { type: "delimited"; open: DelimiterTokenInfo; close: DelimiterTokenInfo }
  | { type: "paired"; pairs: Array<{ open: DelimiterTokenInfo; close: DelimiterTokenInfo }> };

export interface ParsedArgSpecSlot {
  required: boolean;
  noLeadingSpace: boolean;
  nullable: boolean;
  kind: ArgSpecKindInfo;
  form: ArgSpecFormInfo;
}

export interface CommandInfo {
  name: string;
  kind: CommandKind;
  allowedMode: AllowedMode;
  specString: string;
  fromPackages: string[];
  tags: string[];
  args: ParsedArgSpecSlot[];
}

export interface EnvInfo {
  name: string;
  allowedMode: AllowedMode;
  bodyMode: RuntimeContentMode;
  specString: string;
  fromPackages: string[];
  tags: string[];
  args: ParsedArgSpecSlot[];
}

export interface CharacterAttributesInfo {
  mathvariant: string | null;
}

export interface CharacterInfo {
  name: string;
  allowedMode: AllowedMode;
  unicodeValue: string;
  attributes: CharacterAttributesInfo;
  package: string;
}

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

export type CommandSpacing = "spaced" | "minimal";
export type MathGroupInnerSpacing = "padded" | "compact";
export type AdjacentCharSpacing = "spaced" | "compact";
export type ScriptSpacing = "spaced" | "compact";
export type ScriptOrder = "sub_first" | "sup_first";
export type EnvironmentNameSpacing = "spaced" | "compact";

export interface MathSpacingOptions {
  commands?: CommandSpacing;
  groupInnerSpacing?: MathGroupInnerSpacing;
  adjacentChars?: AdjacentCharSpacing;
}

export interface MathScriptOptions {
  spacing?: ScriptSpacing;
  order?: ScriptOrder;
}

export interface MathSerializeOptions {
  spacing?: MathSpacingOptions;
  scripts?: MathScriptOptions;
}

export interface EnvironmentSerializeOptions {
  nameSpacing?: EnvironmentNameSpacing;
}

export interface SyntaxSerializeOptions {
  environments?: EnvironmentSerializeOptions;
}

export interface SerializeOptions {
  math?: MathSerializeOptions;
  syntax?: SyntaxSerializeOptions;
}

export interface TransformReport {
  /** Fixed-point rewrite iterations for this normalization run. */
  iterations: number;
  /** Rewrite rules that were attempted, sorted by stable rule key. */
  rules: Array<{ key: string; appliedCount: number; skippedCount: number }>;
  finalizeAst: {
    steps: {
      mergeAdjacentPrimes: {
        appliedCount: number;
      };
    };
  };
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
  flattenGroups: {
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

export interface AttributeFormCounts {
  declaratives: number;
  prefixes: number;
}

export interface TransformResult {
  normalized: string;
  report: TransformReport;
}

export interface ValidateArgspecResult {
  valid: boolean;
  error: string | null;
  argCount: number | null;
  parsed: ParsedArgSpecSlot[] | null;
}

export interface LowerAttributesConfigInput {
  enabled?: boolean;
}

export interface RewriteConfigInput {
  enabled?: boolean;
  maxIterations?: number;
}

export interface FinalizeAstConfigInput {
  enabled?: boolean;
}

export interface FlattenGroupsConfigInput {
  enabled?: boolean;
  preserveEmptyGroup?: boolean;
  preserveGroupAdjacentToCommandLike?: boolean;
  preserveGroupAfterScriptedCommandLike?: boolean;
  preserveGroupContainingDeclarativeCommand?: boolean;
  preserveGroupContainingDelimitedPair?: boolean;
  preserveGroupContainingInfix?: boolean;
  preserveGroupInScriptBaseSlot?: boolean;
  preserveGroupInsideEnvBody?: boolean;
  preserveGroupStartingWithAtomSpacingChar?: boolean;
  preserveGroupWithLoneAtomSpacingChar?: boolean;
}

export interface TransformConfigInput {
  lowerAttributes?: LowerAttributesConfigInput;
  rewrite?: RewriteConfigInput;
  finalizeAst?: FinalizeAstConfigInput;
  flattenGroups?: FlattenGroupsConfigInput;
}

export type Profile = "authoring" | "faithful" | "corpus" | "equiv";

export interface ParserOptions {
  packages?: string[];
  items?: ContextItem[];
  removeCommands?: string[];
  removeEnvironments?: string[];
  removeDelimiterControls?: string[];
}

export interface TransformEngineOptions extends ParserOptions {
  profile: Profile;
  disableRules?: string[];
}

export interface NormalizeOptions extends ParseConfigInput {
  flattenGroups?: FlattenGroupsConfigInput;
  finalizeAst?: FinalizeAstConfigInput;
  rewriteEnabled?: boolean;
  lowerAttributesEnabled?: boolean;
  maxIterations?: number;
}

export class Parser {
  constructor(options?: ParserOptions | null);
  free(): void;
  [Symbol.dispose](): void;
  isDelimiterControl(name: string): boolean;
  knowsCommandName(name: string): boolean;
  knowsEnvName(name: string): boolean;
  knowsCharacterName(name: string): boolean;
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | null;
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | null;
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
}

export class TransformEngine {
  constructor(options: TransformEngineOptions);
  free(): void;
  [Symbol.dispose](): void;
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
  normalize(src: string, options?: NormalizeOptions | null): TransformResult;
  isDelimiterControl(name: string): boolean;
  knowsCommandName(name: string): boolean;
  knowsEnvName(name: string): boolean;
  knowsCharacterName(name: string): boolean;
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | null;
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | null;
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | null;
}

/**
 * @deprecated Prefer `Document.fromSyntax(node).toLatex(options)` or
 * `document.toLatex(options)`.
 */
export function serialize(node: SyntaxNode, options?: SerializeOptions | null): string;

export function validateArgspec(spec: string): ValidateArgspecResult;

export interface PackageInfo {
  name: string;
  commands: number;
  environments: number;
}

/** List all built-in knowledge packages with record counts. */
export function listPackages(): PackageInfo[];
