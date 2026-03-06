import initWasmModule, {
  ParseContext as WasmParseContext,
  lookup_command_info as wasmLookupCommandInfo,
  lookup_env_info as wasmLookupEnvInfo,
  parse as wasmParse,
  parse_once_with_spec as wasmParseOnceWithSpec,
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
export type ParseOnceSpecTarget =
  | {
      target: 'command'
      kind: CommandKind
      mode: AllowedMode
    }
  | {
      target: 'environment'
      mode: AllowedMode
      bodyMode: BodyMode
    }

export interface ArgSpecInfo {
  required: boolean
  no_leading_space: boolean
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

export function parseOnceWithSpec(
  name: string,
  target: ParseOnceSpecTarget,
  spec: string,
  input: string,
  strict?: boolean | null,
  packages?: string[],
): ParseResult {
  assertReady()
  if (target.target === 'command') {
    return wasmParseOnceWithSpec(
      name,
      'command',
      target.mode,
      spec,
      input,
      strict,
      packages,
      target.kind,
      undefined,
    )
  }

  return wasmParseOnceWithSpec(
    name,
    'environment',
    target.mode,
    spec,
    input,
    strict,
    packages,
    undefined,
    target.bodyMode,
  )
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
