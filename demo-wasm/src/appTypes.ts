import type { AllowedMode, BodyMode, CommandKind } from './texformWasm'

export interface TreeNode {
  id: string
  type: string
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
  children: TreeNode[]
}

export type CustomKnowledgeRecordTarget = 'command' | 'environment' | 'delimiter'

export type CustomKnowledgeRecordEntry =
  | {
      target: 'command'
      name: string
      kind: CommandKind
      mode: AllowedMode
      spec: string
    }
  | {
      target: 'environment'
      name: string
      mode: AllowedMode
      bodyMode: BodyMode
      spec: string
    }
  | {
      target: 'delimiter'
      name: string
    }
