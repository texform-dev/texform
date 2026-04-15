import type { AllowedMode, BodyMode, CommandKind } from './texformWasm'

export interface TreeNode {
  id: string
  type: string
  known?: boolean
  role?: string
  subtitle?: string
  value?: string
  commandName?: string
  specString?: string
  specFromPackages?: string[]
  specDetail?: string
  explicitSpecString?: string
  explicitSpecFromPackages?: string[]
  explicitSpecDetail?: string
  characterUnicodeValue?: string
  characterPackage?: string
  characterMathvariant?: string
  argKind?: string
  argIndex?: number
  /** Present on Error nodes — the parser's error message */
  errorMessage?: string
  /** Present on Error nodes — the raw source snippet that failed to parse */
  errorSnippet?: string
  spanIds: string[]
  children: TreeNode[]
}

export type CustomKnowledgeRecordTarget = 'command' | 'environment' | 'delimiter'

export type CustomKnowledgeRecordEntry =
  | {
      target: 'command'
      name: string
      kind: CommandKind
      mode: AllowedMode
      argspec: string
    }
  | {
      target: 'environment'
      name: string
      mode: AllowedMode
      bodyMode: BodyMode
      argspec: string
    }
  | {
      target: 'delimiter'
      name: string
    }
