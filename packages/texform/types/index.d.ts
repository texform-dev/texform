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

export type RewriteClass = "standard" | "expand" | "drop" | "equiv";
export type TransformProfile = "authoring" | "corpus" | "corpus-drop" | "equiv";

export interface Span {
  start: number;
  end: number;
}

export interface ParseDiagnosticContext {
  label: string;
  span: Span;
}

export interface ParseDiagnostic {
  kind?: ParseDiagnosticKind;
  message: string;
  span: Span;
  expected: string[];
  found?: string;
  contexts: ParseDiagnosticContext[];
}

export type ContentMode = "Math" | "Text" | "math" | "text";
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
  toLatex(options?: SerializeOptions | null): string;
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
  readonly commandName?: string;
  readonly envName?: string;
  readonly text?: string;
  readonly char?: string;
  primeCount(): number | null;
  errorParts(): { message: string; snippet: string } | null;
  contentMode(): RuntimeContentMode | undefined;
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

export class TexformParseError extends Error {
  diagnostics: ParseDiagnostic[];
}

export type AllowedMode = "math" | "text" | "both";
export type CommandKind = "prefix" | "infix" | "declarative";

export interface ArgSpecInfo {
  required: boolean;
  no_leading_space: boolean;
  nullable: boolean;
  kind: unknown;
  form: unknown;
}

export interface CommandInfo {
  name: string;
  kind: CommandKind;
  allowed_mode: AllowedMode;
  spec_string: string;
  from_packages: string[];
  tags: string[];
  args: ArgSpecInfo[];
}

export interface EnvInfo {
  name: string;
  allowed_mode: AllowedMode;
  body_mode: RuntimeContentMode;
  spec_string: string;
  from_packages: string[];
  tags: string[];
  args: ArgSpecInfo[];
}

export interface CharacterAttributesInfo {
  mathvariant?: string;
}

export interface CharacterInfo {
  name: string;
  allowed_mode: AllowedMode;
  unicode_value: string;
  attributes: CharacterAttributesInfo;
  package: string;
}

export type ContextItem =
  | {
      target: "command";
      name: string;
      kind: CommandKind;
      allowed_mode: AllowedMode;
      argspec: string;
      tags?: string[];
    }
  | {
      target: "environment";
      name: string;
      allowed_mode: AllowedMode;
      body_mode: RuntimeContentMode;
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
  group_inner_spacing?: MathGroupInnerSpacing;
  adjacent_chars?: AdjacentCharSpacing;
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
  name_spacing?: EnvironmentNameSpacing;
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
  rules: Array<{ key: string; applied_count: number; skipped_count: number }>;
  finalizeAst: {
    steps: {
      merge_adjacent_primes: {
        applied_count: number;
      };
    };
  };
  lower_attributes: {
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
    eliminated_empty_segments: number;
  };
  flatten_groups: {
    actions: {
      removed_empty: number;
      replaced_single_child: number;
      inlined_multi_child: number;
      unwrapped_slot: number;
    };
    /** Preserve-guard hit counters; each guard blocks a flatten action in a specific context. */
    guards: {
      preserve_group_containing_declarative_command: number;
      preserve_group_in_script_base_slot: number;
      preserve_group_inside_env_body: number;
      preserve_group_containing_infix: number;
      preserve_group_adjacent_to_command_like: number;
      preserve_group_as_argument_of_command: number;
      preserve_group_after_scripted_command_like: number;
      preserve_empty_group: number;
      preserve_group_with_lone_atom_spacing_char: number;
      preserve_group_starting_with_atom_spacing_char: number;
      preserve_group_containing_delimited_pair: number;
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
  parsed?: unknown[] | null;
  error?: string | null;
  arg_count?: number;
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

export type Profile = "authoring" | "corpus" | "corpus-drop" | "equiv";

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
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | undefined;
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | undefined;
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | undefined;
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | undefined;
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
  lookupCharacter(name: string, mode: RuntimeContentMode): CharacterInfo | undefined;
  lookupCommand(name: string, mode: RuntimeContentMode): CommandInfo | undefined;
  lookupEnv(name: string, mode: RuntimeContentMode): EnvInfo | undefined;
  lookupExplicitCommand(name: string, mode: RuntimeContentMode): CommandInfo | undefined;
}

/**
 * @deprecated Prefer `Document.fromSyntax(node).toLatex(options)` or
 * `document.toLatex(options)`.
 */
export function serialize(node: SyntaxNode, options?: SerializeOptions | null): string;

export function validateArgspec(spec: string): ValidateArgspecResult;
