import initWasmModule, {
  ParseContext as WasmParseContext,
  lookup_command_info as wasmLookupCommandInfo,
  lookup_env_info as wasmLookupEnvInfo,
  parse as wasmParse,
  parse_with_context_items as wasmParseWithContextItems,
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
export type ContextItem =
  | {
      target: 'command'
      name: string
      spec: string
      kind: CommandKind
      allowed_mode: AllowedMode
      tags?: string[]
    }
  | {
      target: 'environment'
      name: string
      spec: string
      allowed_mode: AllowedMode
      body_mode: BodyMode
      tags?: string[]
    }
  | {
      target: 'delimiter'
      name: string
    }

export interface ParseWithContextSingleResult {
  input: string
  success: boolean
  result: ParseResult | null
  display: string | null
  diagnostics: ParseDiagnostic[]
  partial_result: ParseResult | null
  partial_display: string | null
  error: string | null
}

export type ParseWithContextBatchResult = ParseWithContextSingleResult[]

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

  insertItem(item: ContextItem): void {
    this.inner.insert_item(item)
  }

  insertItems(items: ContextItem[]): void {
    this.inner.insert_items(items)
  }

  removeItem(item: ContextItem): boolean {
    return this.inner.remove_item(item) as boolean
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

export function parseWithContextItems(
  items: ContextItem[],
  inputs: string[],
  packages?: string[] | null,
  strict?: boolean | null,
): ParseWithContextBatchResult {
  assertReady()
  return wasmParseWithContextItems(
    items,
    inputs,
    packages ?? undefined,
    strict,
  ) as ParseWithContextBatchResult
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
