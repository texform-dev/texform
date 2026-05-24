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

export type ContentMode = "Math" | "Text";
export type Delimiter = "None" | { Char: string } | { Control: string };
export type GroupKind =
  | "Explicit"
  | "Implicit"
  | { Delimited: { left: Delimiter; right: Delimiter } }
  | "InlineMath";

export type ArgumentKind =
  | "Mandatory"
  | "Optional"
  | "Star"
  | "Group"
  | { Delimited: { open: Delimiter; close: Delimiter } }
  | { Paired: { open: Delimiter; close: Delimiter } };

export type SyntaxNode =
  | { Root: { mode: ContentMode; children: SyntaxNode[] } }
  | { Group: { mode: ContentMode; kind: GroupKind; children: SyntaxNode[] } }
  | { Command: { name: string; args: ArgumentSlot[]; known: boolean } }
  | { Infix: { name: string; args: ArgumentSlot[]; left: SyntaxNode; right: SyntaxNode } }
  | { Declarative: { name: string; args: ArgumentSlot[] } }
  | { Environment: { name: string; args: ArgumentSlot[]; known: boolean; body: SyntaxNode } }
  | { Scripted: { base: SyntaxNode; subscript?: SyntaxNode; superscript?: SyntaxNode } }
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

export interface NodeSpanEntry {
  id: string;
  span: Span;
}

export interface ParseResult {
  node: SyntaxNode;
  span: Span;
  node_spans: NodeSpanEntry[];
  display: string;
}

export interface ParseOutput {
  result?: ParseResult;
  diagnostics: ParseDiagnostic[];
}

export class TexformParseError extends Error {
  diagnostics: ParseDiagnostic[];
  partialResult: ParseResult | null;
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
  body_mode: "math" | "text";
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
      body_mode: "math" | "text";
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
  iterations: number;
  applied: Array<{ key: string; count: number; skipped_count: number }>;
  lower_attributes: {
    eliminated_empty_segments: number;
  };
  flatten_groups: {
    removed_empty: number;
    replaced_single_child: number;
    inlined_multi_child: number;
    unwrapped_slot: number;
    preserved_group_containing_declarative_command: number;
    preserved_group_in_script_base_slot: number;
    preserved_group_inside_env_body: number;
    preserved_group_containing_infix: number;
    preserved_group_adjacent_to_command_like: number;
    preserved_group_as_argument_of_command: number;
    preserved_group_after_scripted_command_like: number;
    preserved_empty_group: number;
    preserved_group_with_lone_atom_spacing_char: number;
    preserved_group_starting_with_atom_spacing_char: number;
    preserved_group_containing_delimited_pair: number;
  };
}

export interface TransformResult {
  normalized: string;
  report: TransformReport;
}

export interface ValidateArgspecResult {
  valid: boolean;
  parsed?: unknown[];
  error?: string;
}

export interface FlattenGroupsConfigInput {
  enabled?: boolean;
  preserve_empty_group?: boolean;
  preserve_group_adjacent_to_command_like?: boolean;
  preserve_group_after_scripted_command_like?: boolean;
  preserve_group_containing_declarative_command?: boolean;
  preserve_group_containing_delimited_pair?: boolean;
  preserve_group_containing_infix?: boolean;
  preserve_group_in_script_base_slot?: boolean;
  preserve_group_inside_env_body?: boolean;
  preserve_group_starting_with_atom_spacing_char?: boolean;
  preserve_group_with_lone_atom_spacing_char?: boolean;
}

export type Profile = "authoring" | "corpus" | "corpus-drop" | "equiv";

export interface ParserOptions {
  packages?: string[];
  items?: ContextItem[];
  removeCommands?: string[];
  removeEnvironments?: string[];
  removeDelimiterControls?: string[];
}

export interface EngineOptions extends ParserOptions {
  profile: Profile;
  disableRules?: string[];
}

export interface NormalizeOptions extends ParseConfigInput {
  flattenGroups?: FlattenGroupsConfigInput;
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
  lookupCharacter(name: string, mode: "math" | "text"): CharacterInfo | undefined;
  lookupCommand(name: string, mode: "math" | "text"): CommandInfo | undefined;
  lookupEnv(name: string, mode: "math" | "text"): EnvInfo | undefined;
  lookupExplicitCommand(name: string, mode: "math" | "text"): CommandInfo | undefined;
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
}

export class Engine {
  constructor(options: EngineOptions);
  free(): void;
  [Symbol.dispose](): void;
  parse(src: string, config?: ParseConfigInput | null): ParseResult;
  normalize(src: string, options?: NormalizeOptions | null): TransformResult;
  isDelimiterControl(name: string): boolean;
  knowsCommandName(name: string): boolean;
  knowsEnvName(name: string): boolean;
  knowsCharacterName(name: string): boolean;
  lookupCharacter(name: string, mode: "math" | "text"): CharacterInfo | undefined;
  lookupCommand(name: string, mode: "math" | "text"): CommandInfo | undefined;
  lookupEnv(name: string, mode: "math" | "text"): EnvInfo | undefined;
  lookupExplicitCommand(name: string, mode: "math" | "text"): CommandInfo | undefined;
}

export function serialize(node: SyntaxNode, options?: SerializeOptions | null): string;
export function validateArgspec(spec: string): ValidateArgspecResult;
