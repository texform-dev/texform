import initWasmModule, {
  ParseContext as WasmParseContext,
  lookup_command_info as wasmLookupCommandInfo,
  lookup_env_info as wasmLookupEnvInfo,
  parse as wasmParse,
  parse_with_argspecs as wasmParseWithArgspecs,
  type Argument,
  type ArgumentValue,
  type GroupKind,
  type ParseDiagnostic,
  type ParseResult,
  type Span,
  type SyntaxNode,
} from 'texform-wasm'

let initPromise: Promise<void> | null = null
let initialized = false

function assertReady(): void {
  if (!initialized) {
    throw new Error('WASM is still initializing')
  }
}

export async function ensureWasmReady(): Promise<void> {
  if (initialized) {
    return
  }
  if (!initPromise) {
    initPromise = initWasmModule().then(() => {
      initialized = true
    })
  }
  await initPromise
}

export type CommandKind = 'prefix' | 'infix' | 'declarative'
export type AllowedMode = 'math' | 'text' | 'both'
export type BodyMode = 'math' | 'text'
export type TemporaryArgSpec =
  | {
      target: 'command'
      name: string
      spec: string
      kind: CommandKind
      allowed_mode: AllowedMode
    }
  | {
      target: 'environment'
      name: string
      spec: string
      allowed_mode: AllowedMode
      body_mode: BodyMode
    }

export interface ParseWithArgspecSingleResult {
  input: string
  success: boolean
  result: ParseResult | null
  display: string | null
  diagnostics: ParseDiagnostic[]
  partial_result: ParseResult | null
  partial_display: string | null
  error: string | null
}

export type ParseWithArgspecBatchResult = ParseWithArgspecSingleResult[]

export interface ArgSpecInfo {
  required: boolean
  no_leading_space: boolean
  nullable: boolean
  kind: unknown
  form: unknown
}

export interface CommandInfo {
  name: string
  kind: CommandKind
  allowed_mode: AllowedMode
  spec_string: string
  package: string
  tags: string[]
  args: ArgSpecInfo[]
}

export interface EnvInfo {
  name: string
  allowed_mode: AllowedMode
  body_mode: BodyMode
  spec_string: string
  package: string
  tags: string[]
  args: ArgSpecInfo[]
}

export class ParseContext {
  private readonly inner: WasmParseContext

  constructor(packages?: string[]) {
    assertReady()
    this.inner = new WasmParseContext(packages ?? undefined)
  }

  parse(src: string, strict?: boolean | null): ParseResult {
    return this.inner.parse(src, strict)
  }

  insertCommand(name: string, kind: CommandKind, mode: AllowedMode, spec: string): void {
    this.inner.insert_command(name, kind, mode, spec)
  }

  removeCommand(name: string): boolean {
    return this.inner.remove_command(name)
  }

  insertEnv(
    name: string,
    mode: AllowedMode,
    spec: string,
    bodyMode: BodyMode,
  ): void {
    this.inner.insert_env(name, mode, spec, bodyMode)
  }

  removeEnv(name: string): boolean {
    return this.inner.remove_env(name)
  }

  lookupCommand(name: string): CommandInfo | null {
    return this.inner.lookup_command(name) as CommandInfo | null
  }

  lookupEnv(name: string): EnvInfo | null {
    return this.inner.lookup_env(name) as EnvInfo | null
  }
}

export function parseLatex(src: string, strict?: boolean | null): ParseResult {
  assertReady()
  return wasmParse(src, strict)
}

/**
 * Test one or more ArgSpecs by temporarily injecting commands/environments and parsing one or more inputs.
 *
 * By default, this loads the embedded `test` package so text-mode probes can use `\text{...}`.
 * Pass `packages` to override that with an explicit package list such as `['dev']` or `[]`.
 *
 * Prefer inputs that only exercise the temporary targets plus plain literal content.
 * The one allowed helper command is `\text{...}` when you intentionally need text mode.
 * Avoid other commands/environments and avoid values that depend on unrelated records.
 */
export function parse_with_argspecs(
  argspecs: TemporaryArgSpec[],
  inputs: string[],
  packages?: string[] | null,
  strict?: boolean | null,
): ParseWithArgspecBatchResult {
  assertReady()
  return wasmParseWithArgspecs(
    argspecs,
    inputs,
    packages ?? undefined,
    strict,
  ) as ParseWithArgspecBatchResult
}

export function lookupCommandInfo(name: string): CommandInfo | null {
  assertReady()
  return wasmLookupCommandInfo(name) as CommandInfo | null
}

export function lookupEnvInfo(name: string): EnvInfo | null {
  assertReady()
  return wasmLookupEnvInfo(name) as EnvInfo | null
}

export type {
  Argument,
  ArgumentValue,
  GroupKind,
  ParseDiagnostic,
  ParseResult,
  Span,
  SyntaxNode,
}
