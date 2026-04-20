import type {
  ArgSpecInfo,
  Argument,
  ArgumentSlot,
  ArgumentValue,
  CharacterInfo,
  CommandInfo,
  ContentMode,
  EnvInfo,
  GroupKind,
  ParseDiagnostic,
  ParseResult,
  SyntaxNode,
} from './texformWasm'
import type { TreeNode } from './types'

// -- Tree building --

export interface LookupContext {
  lookupCommand(name: string, mode: ContentMode): CommandInfo | null
  lookupExplicitCommand(name: string, mode: ContentMode): CommandInfo | null
  lookupCharacter(name: string, mode: ContentMode): CharacterInfo | null
  lookupEnv(name: string, mode: ContentMode): EnvInfo | null
}

export function buildSyntaxTree(
  node: SyntaxNode,
  id: string,
  currentMode: ContentMode,
  lookup: LookupContext,
): TreeNode {
  if (node === 'ActiveSpace') {
    return {
      id,
      type: 'ActiveSpace',
      value: quoted('~'),
      spanIds: [id],
      children: [],
    }
  }

  if (typeof node !== 'object' || node === null) {
    return {
      id,
      type: 'UnknownNode',
      value: quoted(String(node)),
      spanIds: [id],
      children: [],
    }
  }

  if ('Text' in node) {
    return {
      id,
      type: 'Text',
      value: quoted(node.Text),
      spanIds: [id],
      children: [],
    }
  }

  if ('Char' in node) {
    return {
      id,
      type: 'Char',
      value: quoted(node.Char),
      spanIds: [id],
      children: [],
    }
  }

  if ('Root' in node) {
    const root = node.Root
    const rawChildren = root.children.map((child: SyntaxNode, index: number) =>
      buildSyntaxTree(
        child,
        `${id}.child.${index}`,
        root.mode.toLowerCase() as ContentMode,
        lookup,
      ),
    )
    return {
      id,
      type: 'Root',
      subtitle: root.mode,
      spanIds: [id],
      children: mergeConsecutiveChars(rawChildren, id),
    }
  }

  if ('Group' in node) {
    const group = node.Group
    const rawChildren = group.children.map((child: SyntaxNode, index: number) =>
      buildSyntaxTree(
        child,
        `${id}.child.${index}`,
        group.mode.toLowerCase() as ContentMode,
        lookup,
      ),
    )
    return {
      id,
      type: 'Group',
      subtitle: `${group.mode} · ${describeGroupKind(group.kind)}`,
      spanIds: [id],
      children: mergeConsecutiveChars(rawChildren, id),
    }
  }

  if ('Command' in node) {
    const command = node.Command
    const activeSpec = lookup.lookupCommand(command.name, currentMode)
    const explicitSpec = lookup.lookupExplicitCommand(command.name, currentMode)
    const character = lookup.lookupCharacter(command.name, currentMode)
    return {
      id,
      type: 'Command',
      known: command.known,
      commandName: `\\${command.name}`,
      subtitle: `${command.args.length} args`,
      spanIds: [id],
      specString: activeSpec?.spec_string,
      specFromPackages: activeSpec?.from_packages,
      specDetail: activeSpec ? formatSpecDetail(activeSpec.args) : undefined,
      explicitSpecString: explicitSpec?.spec_string,
      explicitSpecFromPackages: explicitSpec?.from_packages,
      explicitSpecDetail: explicitSpec ? formatSpecDetail(explicitSpec.args) : undefined,
      characterUnicodeValue: character?.unicode_value,
      characterPackage: character?.package,
      characterMathvariant: character?.attributes.mathvariant,
      children: command.args.map((arg: ArgumentSlot, index: number) =>
        buildArgumentNode(
          arg,
          `${id}.arg.${index}`,
          index,
          currentMode,
          lookup,
        ),
      ),
    }
  }

  if ('Infix' in node) {
    const infix = node.Infix
    const activeSpec = lookup.lookupCommand(infix.name, currentMode)
    const explicitSpec = lookup.lookupExplicitCommand(infix.name, currentMode)
    const character = lookup.lookupCharacter(infix.name, currentMode)
    const args = infix.args.map((arg: ArgumentSlot, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        currentMode,
        lookup,
      ),
    )
    return {
      id,
      type: 'Infix',
      commandName: `\\${infix.name}`,
      subtitle: `${infix.args.length} args`,
      spanIds: [id],
      specString: activeSpec?.spec_string,
      specFromPackages: activeSpec?.from_packages,
      specDetail: activeSpec ? formatSpecDetail(activeSpec.args) : undefined,
      explicitSpecString: explicitSpec?.spec_string,
      explicitSpecFromPackages: explicitSpec?.from_packages,
      explicitSpecDetail: explicitSpec ? formatSpecDetail(explicitSpec.args) : undefined,
      characterUnicodeValue: character?.unicode_value,
      characterPackage: character?.package,
      characterMathvariant: character?.attributes.mathvariant,
      children: [
        withRole(
          buildSyntaxTree(
            infix.left,
            `${id}.left`,
            currentMode,
            lookup,
          ),
          'left',
        ),
        ...args,
        withRole(
          buildSyntaxTree(
            infix.right,
            `${id}.right`,
            currentMode,
            lookup,
          ),
          'right',
        ),
      ],
    }
  }

  if ('Declarative' in node) {
    const declarative = node.Declarative
    const activeSpec = lookup.lookupCommand(declarative.name, currentMode)
    const explicitSpec = lookup.lookupExplicitCommand(declarative.name, currentMode)
    const character = lookup.lookupCharacter(declarative.name, currentMode)
    const args = declarative.args.map((arg: ArgumentSlot, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        currentMode,
        lookup,
      ),
    )
    return {
      id,
      type: 'Declarative',
      commandName: `\\${declarative.name}`,
      subtitle: `${declarative.args.length} args`,
      spanIds: [id],
      specString: activeSpec?.spec_string,
      specFromPackages: activeSpec?.from_packages,
      specDetail: activeSpec ? formatSpecDetail(activeSpec.args) : undefined,
      explicitSpecString: explicitSpec?.spec_string,
      explicitSpecFromPackages: explicitSpec?.from_packages,
      explicitSpecDetail: explicitSpec ? formatSpecDetail(explicitSpec.args) : undefined,
      characterUnicodeValue: character?.unicode_value,
      characterPackage: character?.package,
      characterMathvariant: character?.attributes.mathvariant,
      children: [...args],
    }
  }

  if ('Environment' in node) {
    const env = node.Environment
    const spec = lookup.lookupEnv(env.name, currentMode)
    const args = env.args.map((arg: ArgumentSlot, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        currentMode,
        lookup,
      ),
    )
    return {
      id,
      type: 'Environment',
      known: env.known,
      commandName: env.name,
      subtitle: `${env.args.length} args`,
      spanIds: [id],
      specString: spec?.spec_string,
      specFromPackages: spec?.from_packages,
      specDetail: spec ? formatSpecDetail(spec.args) : undefined,
      children: [
        ...args,
        withRole(
          buildSyntaxTree(
            env.body,
            `${id}.body`,
            currentMode,
            lookup,
          ),
          'body',
        ),
      ],
    }
  }

  if ('Scripted' in node) {
    const scripted = node.Scripted
    const children: TreeNode[] = [
      withRole(
        buildSyntaxTree(
          scripted.base,
          `${id}.base`,
          currentMode,
          lookup,
        ),
        'base',
      ),
    ]
    if (scripted.subscript) {
      children.push(
        withRole(
          buildSyntaxTree(
            scripted.subscript,
            `${id}.sub`,
            currentMode,
            lookup,
          ),
          'sub',
        ),
      )
    }
    if (scripted.superscript) {
      children.push(
        withRole(
          buildSyntaxTree(
            scripted.superscript,
            `${id}.sup`,
            currentMode,
            lookup,
          ),
          'sup',
        ),
      )
    }
    return {
      id,
      type: 'Scripted',
      spanIds: [id],
      children,
    }
  }

  if ('Error' in node) {
    const err = node.Error
    return {
      id,
      type: 'Error',
      errorMessage: err.message,
      errorSnippet: err.snippet,
      spanIds: [id],
      children: [],
    }
  }

  return {
    id,
    type: 'UnknownNode',
    spanIds: [id],
    children: [],
  }
}

export function buildArgumentNode(
  argument: ArgumentSlot,
  id: string,
  index: number,
  currentMode: ContentMode,
  lookup: LookupContext,
): TreeNode {
  if (argument == null || typeof argument !== 'object' || !('value' in argument)) {
    return {
      id,
      type: 'Arg',
      argIndex: index,
      subtitle: 'missing',
      spanIds: [],
      children: [],
    }
  }

  const value = describeArgumentValue(argument.value)

  // Flatten: if the arg is Content with a single child, inline it
  if (value.content !== null) {
    const contentChild = buildSyntaxTree(
      value.content,
      `${id}.content`,
      value.contentMode ?? currentMode,
      lookup,
    )
    // If the content child is a Group with children, we can still flatten
    // by promoting the content node and annotating it with arg info
    return {
      id,
      type: 'Arg',
      argKind: describeArgumentKind(argument.kind),
      argIndex: index,
      subtitle: value.kind,
      value: value.value,
      spanIds: [id],
      children: [contentChild],
    }
  }

  return {
    id,
    type: 'Arg',
    argKind: describeArgumentKind(argument.kind),
    argIndex: index,
    subtitle: value.kind,
    value: value.value,
    spanIds: [id],
    children: [],
  }
}

/**
 * Merge runs of consecutive Char leaf nodes into a single "Chars" node.
 * The merged node is expandable to reveal individual Char children.
 * Runs of length 1 are kept as-is.
 */
export function mergeConsecutiveChars(nodes: TreeNode[], parentId: string): TreeNode[] {
  const result: TreeNode[] = []
  let runStart = 0

  while (runStart < nodes.length) {
    if (nodes[runStart].type === 'Char') {
      let runEnd = runStart + 1
      while (runEnd < nodes.length && nodes[runEnd].type === 'Char') {
        runEnd++
      }
      const runLength = runEnd - runStart
      if (runLength > 1) {
        const combined = nodes
          .slice(runStart, runEnd)
          .map((c) => {
            const raw = c.value ?? ''
            return raw.length >= 2 ? raw.slice(1, -1) : raw
          })
          .join('')
        result.push({
          id: `${parentId}.chars.${runStart}`,
          type: 'Chars',
          value: quoted(combined),
          spanIds: nodes.slice(runStart, runEnd).flatMap((node) => node.spanIds),
          children: [],
        })
      } else {
        result.push(nodes[runStart])
      }
      runStart = runEnd
    } else {
      result.push(nodes[runStart])
      runStart++
    }
  }

  return result
}

export function withRole(node: TreeNode, role: string): TreeNode {
  return { ...node, role }
}

export function describeArgumentKind(kind: Argument['kind']): string {
  if (
    kind === 'Mandatory' ||
    kind === 'Optional' ||
    kind === 'Star' ||
    kind === 'Group'
  ) {
    return kind
  }
  if ('Delimited' in kind) {
    return 'Delimited'
  }
  if ('Paired' in kind) {
    return 'Paired'
  }
  return 'Unknown'
}

export function formatSpecDetail(args: ArgSpecInfo[]): string {
  if (args.length === 0) {
    return 'no arguments'
  }
  return args
    .map((arg, index) => {
      const req = arg.required ? 'required' : 'optional'
      const nullable = arg.nullable ? ' nullable' : ''
      const kind = describeArgSpecKind(arg.kind)
      const form = describeArgSpecForm(arg.form)
      return `[${index}] ${req}${nullable} ${form} ${kind}`
    })
    .join('\n')
}

export function describeArgSpecKind(kind: unknown): string {
  if (typeof kind === 'string') return kind
  if (kind && typeof kind === 'object' && 'type' in kind) {
    const t = kind as { type: string; mode?: string }
    if (t.type === 'content' && t.mode) return `content(${t.mode})`
    return t.type
  }
  return 'unknown'
}

export function describeArgSpecForm(form: unknown): string {
  if (typeof form === 'string') return form
  if (form && typeof form === 'object' && 'type' in form) {
    const f = form as { type: string }
    return f.type
  }
  return ''
}

export function describeArgumentValue(value: ArgumentValue): {
  kind: string
  value?: string
  content: SyntaxNode | null
  contentMode: ContentMode | null
} {
  if ('MathContent' in value) {
    return {
      kind: 'MathContent',
      content: value.MathContent,
      contentMode: 'math',
    }
  }
  if ('TextContent' in value) {
    return {
      kind: 'TextContent',
      content: value.TextContent,
      contentMode: 'text',
    }
  }
  if ('Delimiter' in value) {
    return {
      kind: 'Delimiter',
      value: describeDelimiter(value.Delimiter),
      content: null,
      contentMode: null,
    }
  }
  if ('CSName' in value) {
    return {
      kind: 'CSName',
      value: value.CSName,
      content: null,
      contentMode: null,
    }
  }
  if ('Dimension' in value) {
    return {
      kind: 'Dimension',
      value: value.Dimension,
      content: null,
      contentMode: null,
    }
  }
  if ('Integer' in value) {
    return {
      kind: 'Integer',
      value: value.Integer,
      content: null,
      contentMode: null,
    }
  }
  if ('KeyVal' in value) {
    return {
      kind: 'KeyVal',
      value: value.KeyVal,
      content: null,
      contentMode: null,
    }
  }
  if ('Column' in value) {
    return {
      kind: 'Column',
      value: value.Column,
      content: null,
      contentMode: null,
    }
  }
  if ('Boolean' in value) {
    return {
      kind: 'Boolean',
      value: String(value.Boolean),
      content: null,
      contentMode: null,
    }
  }
  return {
    kind: 'Unknown',
    content: null,
    contentMode: null,
  }
}

export function describeGroupKind(kind: GroupKind): string {
  if (kind === 'Explicit' || kind === 'Implicit' || kind === 'InlineMath') {
    return kind
  }
  if ('Delimited' in kind) {
    return `Delimited (${describeDelimiter(kind.Delimited.left)} .. ${describeDelimiter(
      kind.Delimited.right,
    )})`
  }
  return 'Unknown group kind'
}

export function describeDelimiter(delimiter: unknown): string {
  if (delimiter === 'None') {
    return 'None'
  }
  if (typeof delimiter === 'object' && delimiter !== null) {
    if ('Char' in delimiter && typeof delimiter.Char === 'string') {
      return quoted(delimiter.Char)
    }
    if ('Control' in delimiter && typeof delimiter.Control === 'string') {
      return `\\${delimiter.Control}`
    }
  }
  return 'Unknown delimiter'
}

export function flattenTree(root: TreeNode): TreeNode[] {
  const list: TreeNode[] = []
  const walk = (node: TreeNode) => {
    list.push(node)
    for (const child of node.children) {
      walk(child)
    }
  }
  walk(root)
  return list
}

export function computeTreeDepth(root: TreeNode | null): number {
  if (!root) {
    return 0
  }

  let maxDepth = 1
  const walk = (node: TreeNode, depth: number) => {
    if (depth > maxDepth) {
      maxDepth = depth
    }
    for (const child of node.children) {
      walk(child, depth + 1)
    }
  }

  walk(root, 1)
  return maxDepth
}

export function quoted(value: string): string {
  const escaped = value.replace(/\n/g, '\\n').replace(/\t/g, '\\t')
  const clipped = escaped.length > 64 ? `${escaped.slice(0, 61)}...` : escaped
  return `"${clipped}"`
}

// -- Utility functions --

export function extractFatalMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  return 'Unknown parsing failure'
}

export function formatParseErrorMessage(
  fatalMessage: string | null,
  diagnostics: ParseDiagnostic[],
): string {
  const sections: string[] = []
  if (fatalMessage !== null) {
    sections.push(fatalMessage)
  }
  if (diagnostics.length > 0) {
    const detailLines = diagnostics.map((diagnostic, index) => {
      const header = `${index + 1}. ${diagnostic.message} (span ${diagnostic.span.start}..${diagnostic.span.end})`
      if (diagnostic.contexts && diagnostic.contexts.length > 0) {
        const contextLines = diagnostic.contexts.map(
          (ctx) => `   in ${ctx.label} (span ${ctx.span.start}..${ctx.span.end})`,
        )
        return `${header}\n${contextLines.join('\n')}`
      }
      return header
    })
    sections.push(`Diagnostics:\n${detailLines.join('\n')}`)
  }
  if (sections.length === 0) {
    return 'Unknown parsing failure'
  }
  return sections.join('\n\n')
}

export function isParseResult(value: unknown): value is ParseResult {
  if (!value || typeof value !== 'object') {
    return false
  }
  const candidate = value as Partial<ParseResult>
  if (!candidate.span || typeof candidate.span !== 'object') {
    return false
  }
  return (
    typeof candidate.span.start === 'number' &&
    typeof candidate.span.end === 'number' &&
    'node' in candidate
  )
}
