import type { AllowedMode, CommandKind } from './texformWasm'

export interface TreeNode {
  id: string
  type: string
  role?: string
  subtitle?: string
  value?: string
  commandName?: string
  specString?: string
  specPackage?: string
  specDetail?: string
  argKind?: string
  argIndex?: number
  children: TreeNode[]
}

export interface CustomCommandEntry {
  name: string
  kind: CommandKind
  mode: AllowedMode
  spec: string
}
