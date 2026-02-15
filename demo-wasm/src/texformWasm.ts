import initWasmModule, {
  parse as wasmParse,
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

export function parseLatex(src: string, strict?: boolean | null): ParseResult {
  if (!initialized) {
    throw new Error('WASM is still initializing')
  }
  return wasmParse(src, strict)
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
