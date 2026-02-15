import { useEffect, useMemo, useState } from 'react'
import {
  ensureWasmReady,
  parseLatex,
  type Argument,
  type ArgumentValue,
  type GroupKind,
  type ParseDiagnostic,
  type ParseResult,
  type SyntaxNode,
} from './texformWasm'

const SAMPLE_LATEX = String.raw`\left(\frac{a+b}{\sqrt[3]{x^2_i}}\right) + \text{foo$a+b$bar}`

interface ParseViewState {
  result: ParseResult | null
  diagnostics: ParseDiagnostic[]
  fatalMessage: string | null
}

interface ParseThrowLike {
  diagnostics?: unknown
  partial_result?: unknown
}

interface TreeNode {
  id: string
  type: string
  role?: string
  subtitle?: string
  value?: string
  commandName?: string
  argKind?: 'Mandatory' | 'Optional'
  argIndex?: number
  children: TreeNode[]
}

// -- Badge color mapping by node type --

type BadgeTone = { bg: string; text: string }

const BADGE_TONES: Record<string, BadgeTone> = {
  Command: { bg: 'bg-blue-100', text: 'text-blue-800' },
  Infix: { bg: 'bg-blue-100', text: 'text-blue-800' },
  Declarative: { bg: 'bg-blue-100', text: 'text-blue-800' },
  Group: { bg: 'bg-emerald-100', text: 'text-emerald-800' },
  Scripted: { bg: 'bg-violet-100', text: 'text-violet-800' },
  Char: { bg: 'bg-slate-100', text: 'text-slate-600' },
  Chars: { bg: 'bg-slate-100', text: 'text-slate-600' },
  Text: { bg: 'bg-slate-100', text: 'text-slate-600' },
  ActiveSpace: { bg: 'bg-slate-100', text: 'text-slate-600' },
  Arg: { bg: 'bg-amber-100', text: 'text-amber-800' },
  Environment: { bg: 'bg-blue-100', text: 'text-blue-800' },
  Unknown: { bg: 'bg-red-50', text: 'text-red-700' },
  UnknownNode: { bg: 'bg-red-50', text: 'text-red-700' },
}

const DEFAULT_BADGE_TONE: BadgeTone = { bg: 'bg-sky-100', text: 'text-sky-800' }

function badgeTone(type: string): BadgeTone {
  return BADGE_TONES[type] ?? DEFAULT_BADGE_TONE
}

function App() {
  const [source, setSource] = useState(SAMPLE_LATEX)
  const [strictMode, setStrictMode] = useState(false)
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmInitError, setWasmInitError] = useState<string | null>(null)
  const [collapsedNodes, setCollapsedNodes] = useState<Set<string>>(new Set())

  useEffect(() => {
    let alive = true
    ensureWasmReady()
      .then(() => {
        if (!alive) {
          return
        }
        setWasmReady(true)
        setWasmInitError(null)
      })
      .catch((error) => {
        if (!alive) {
          return
        }
        setWasmReady(false)
        setWasmInitError(extractFatalMessage(error))
      })

    return () => {
      alive = false
    }
  }, [])

  const parseState = useMemo<ParseViewState>(() => {
    if (!wasmReady) {
      return {
        result: null,
        diagnostics: [],
        fatalMessage: wasmInitError,
      }
    }

    try {
      const parsed = parseLatex(source, strictMode)
      return {
        result: parsed,
        diagnostics: [],
        fatalMessage: null,
      }
    } catch (error) {
      const thrown = (error ?? {}) as ParseThrowLike
      const diagnostics = Array.isArray(thrown.diagnostics)
        ? (thrown.diagnostics as ParseDiagnostic[])
        : []
      const partial = isParseResult(thrown.partial_result) ? thrown.partial_result : null
      const fatalMessage = diagnostics.length > 0 ? null : extractFatalMessage(error)
      return {
        result: partial,
        diagnostics,
        fatalMessage,
      }
    }
  }, [source, strictMode, wasmReady, wasmInitError])

  const treeRoot = useMemo(() => {
    if (!parseState.result) {
      return null
    }
    return buildSyntaxTree(parseState.result.node, 'root')
  }, [parseState.result])

  const flatNodes = useMemo(() => {
    if (!treeRoot) {
      return []
    }
    return flattenTree(treeRoot)
  }, [treeRoot])

  const knownNodeIds = useMemo(() => {
    return new Set(flatNodes.map((node) => node.id))
  }, [flatNodes])

  const effectiveCollapsedNodes = useMemo(() => {
    const next = new Set<string>()
    for (const nodeId of collapsedNodes) {
      if (knownNodeIds.has(nodeId)) {
        next.add(nodeId)
      }
    }
    return next
  }, [collapsedNodes, knownNodeIds])

  const treeDepth = useMemo(() => {
    return computeTreeDepth(treeRoot)
  }, [treeRoot])

  const isWasmLoading = !wasmReady && wasmInitError === null

  const statusText = isWasmLoading
    ? 'WASM Initializing'
    : parseState.fatalMessage !== null
      ? 'Parse Failed'
      : parseState.diagnostics.length > 0
        ? `Partial Parse (${parseState.diagnostics.length})`
        : 'Parse OK'

  const statusToneClass = isWasmLoading
    ? 'border-sky-200 bg-sky-100 text-sky-800'
    : parseState.fatalMessage !== null
      ? 'border-red-200 bg-red-100 text-red-800'
      : parseState.diagnostics.length > 0
        ? 'border-yellow-200 bg-yellow-100 text-yellow-800'
        : 'border-green-200 bg-green-100 text-green-800'

  const paneClass = 'flex min-h-0 flex-col gap-2.5 rounded-sm border border-slate-300 bg-white p-3'
  const sectionHeadClass = 'flex items-center justify-between gap-2'
  const sectionTitleClass = 'm-0 text-sm font-semibold'
  const buttonClass =
    'rounded-sm border border-slate-300 bg-slate-50 px-2.5 py-1 text-xs leading-[1.2] transition-colors hover:bg-slate-100'

  const toggleNode = (nodeId: string) => {
    setCollapsedNodes((prev) => {
      const next = new Set(prev)
      if (next.has(nodeId)) {
        next.delete(nodeId)
      } else {
        next.add(nodeId)
      }
      return next
    })
  }

  const expandAll = () => {
    setCollapsedNodes(new Set())
  }

  const collapseAll = () => {
    if (!treeRoot) {
      setCollapsedNodes(new Set())
      return
    }
    const next = new Set(flatNodes.map((node) => node.id))
    next.delete(treeRoot.id)
    setCollapsedNodes(next)
  }

  const renderTreeNode = (node: TreeNode) => {
    const hasChildren = node.children.length > 0
    const isLeaf = !hasChildren
    const collapsed = effectiveCollapsedNodes.has(node.id)
    const tone = badgeTone(node.type)

    return (
      <div key={node.id} className="min-w-max">
        <div
          className={`flex min-h-5 items-center gap-1.5 whitespace-nowrap rounded-sm transition-colors hover:bg-slate-50 ${hasChildren ? 'cursor-pointer' : 'cursor-default'}`}
          onClick={hasChildren ? () => toggleNode(node.id) : undefined}
        >
          {/* Expand/collapse toggle — leaves get empty spacer */}
          {isLeaf ? (
            <span className="inline-block h-4 w-4" />
          ) : (
            <button
              type="button"
              className="h-4 w-4 rounded-[2px] border-0 bg-transparent p-0 text-center text-[14px] leading-4 text-slate-500 hover:bg-slate-200"
              onClick={(event) => {
                event.stopPropagation()
                toggleNode(node.id)
              }}
              aria-label={collapsed ? 'Expand node' : 'Collapse node'}
              title={collapsed ? 'Expand node' : 'Collapse node'}
            >
              {collapsed ? '▸' : '▾'}
            </button>
          )}

          {/* Role label (base, sub, sup, left, right, scope, body) */}
          {node.role ? (
            <span className="rounded-sm bg-purple-50 px-1 py-px text-[11px] leading-none text-purple-600">
              {node.role}
            </span>
          ) : null}

          {/* Type badge (with inline arg index when applicable) */}
          <span
            className={`inline-flex items-baseline gap-1 rounded-sm px-1 py-px text-[11px] font-medium leading-none ${tone.bg} ${tone.text}`}
          >
            {node.type}
            {node.argIndex !== undefined ? (
              <span className="font-normal opacity-60">{node.argIndex}</span>
            ) : null}
          </span>

          {/* Command name */}
          {node.commandName ? (
            <span className="font-bold text-slate-950">{node.commandName}</span>
          ) : null}

          {/* Arg kind — only show "optional" since mandatory is the default */}
          {node.argKind === 'Optional' ? (
            <span className="rounded-sm border border-amber-200 bg-amber-50 px-1 py-px text-[10px] leading-none text-amber-700">
              opt
            </span>
          ) : null}

          {/* Subtitle (group kind, arg count, etc.) */}
          {node.subtitle ? (
            <span className="text-[12px] text-slate-400">{node.subtitle}</span>
          ) : null}

          {/* Value */}
          {node.value ? (
            <span className="text-blue-600">{node.value}</span>
          ) : null}
        </div>
        {hasChildren && !collapsed ? (
          <div className="ml-[7px] border-l border-slate-200 pl-2.5">
            {node.children.map((child) => renderTreeNode(child))}
          </div>
        ) : null}
      </div>
    )
  }

  return (
    <div className="flex min-h-full flex-col p-3.5">
      <header className="mb-3 flex items-center justify-between gap-3">
        <h1 className="m-0 text-[22px] font-semibold tracking-[-0.01em]">TeXForm WASM Playground</h1>
        <span
          className={`inline-flex items-center rounded-sm border px-2.5 py-[3px] text-xs font-medium ${statusToneClass}`}
        >
          {statusText}
        </span>
      </header>

      <main className="grid min-h-0 flex-1 grid-cols-1 gap-3.5 lg:grid-cols-[minmax(300px,1fr)_minmax(0,2fr)]">
        <section className={`${paneClass} min-h-[320px] lg:min-h-0`}>
          <div className={sectionHeadClass}>
            <h2 className={sectionTitleClass}>LaTeX Input</h2>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button type="button" className={buttonClass} onClick={() => setSource(SAMPLE_LATEX)}>
              Reset Sample
            </button>
            <label className="inline-flex select-text items-center gap-1.5 text-xs">
              <input
                type="checkbox"
                className="m-0"
                checked={strictMode}
                onChange={(event) => setStrictMode(event.target.checked)}
              />
              Strict Mode
            </label>
          </div>

          <textarea
            value={source}
            onChange={(event) => setSource(event.target.value)}
            className="min-h-[220px] w-full resize-y rounded-sm border border-slate-300 bg-white p-2.5 text-[13px] leading-[1.5] text-slate-900 [font-family:var(--font-code)]"
            placeholder="Input LaTeX formula..."
            spellCheck={false}
          />

          {parseState.fatalMessage ? (
            <p className="m-0 text-xs text-red-700">Fatal: {parseState.fatalMessage}</p>
          ) : null}

          {parseState.diagnostics.length > 0 ? (
            <div className="border-t border-slate-200 pt-2">
              <div className="text-xs font-semibold text-slate-700">Diagnostics</div>
              <ul className="mt-1.5 list-disc pl-[18px] text-xs text-slate-700">
                {parseState.diagnostics.map((diagnostic, index) => (
                  <li key={`${diagnostic.message}-${index}`} className="my-0.5 flex flex-wrap items-baseline gap-2">
                    <span>{diagnostic.message}</span>
                    <span className="[font-family:var(--font-code)] text-slate-600">
                      span {diagnostic.span.start}..{diagnostic.span.end}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          <div className="mt-auto border-t border-slate-200 pt-2">
            <div className="text-xs font-semibold text-slate-700">Statistics (Placeholder)</div>
            <ul className="mt-1.5 list-disc pl-[18px] text-xs leading-[1.5] text-slate-700">
              <li>Chars: {source.length}</li>
              <li>Nodes: {flatNodes.length}</li>
              <li>Tree Depth: {treeDepth}</li>
              <li>Diagnostics: {parseState.diagnostics.length}</li>
              <li>
                Root Span:{' '}
                {parseState.result
                  ? `${parseState.result.span.start}..${parseState.result.span.end}`
                  : '--'}
              </li>
              <li className="text-slate-500">TODO: token stats / complexity score</li>
            </ul>
          </div>
        </section>

        <section className={`${paneClass} min-h-[320px] lg:min-h-0`}>
          <div className={sectionHeadClass}>
            <h2 className={sectionTitleClass}>Syntax Tree</h2>
            <div className="flex flex-wrap items-center gap-2">
              <button type="button" className={buttonClass} onClick={expandAll}>
                Expand All
              </button>
              <button type="button" className={buttonClass} onClick={collapseAll}>
                Collapse All
              </button>
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-auto border-t border-slate-200 pt-2 text-[13px] leading-[1.4] [font-family:var(--font-code)]">
            {treeRoot ? (
              renderTreeNode(treeRoot)
            ) : (
              <p className="m-0 text-xs text-slate-600">No syntax tree available.</p>
            )}
          </div>
        </section>
      </main>
    </div>
  )
}

// -- Tree building --

function buildSyntaxTree(node: SyntaxNode, id: string): TreeNode {
  if (node === 'ActiveSpace') {
    return {
      id,
      type: 'ActiveSpace',
      value: quoted('~'),
      children: [],
    }
  }

  if (typeof node !== 'object' || node === null) {
    return {
      id,
      type: 'UnknownNode',
      value: quoted(String(node)),
      children: [],
    }
  }

  if ('Text' in node) {
    return {
      id,
      type: 'Text',
      value: quoted(node.Text),
      children: [],
    }
  }

  if ('Char' in node) {
    return {
      id,
      type: 'Char',
      value: quoted(node.Char),
      children: [],
    }
  }

  if ('UnknownCommand' in node) {
    const command = node.UnknownCommand
    return {
      id,
      type: 'Unknown',
      commandName: `\\${command.name}${command.starred ? '*' : ''}`,
      children: [],
    }
  }

  if ('Group' in node) {
    const group = node.Group
    const rawChildren = group.children.map((child, index) =>
      buildSyntaxTree(child, `${id}.child.${index}`),
    )
    return {
      id,
      type: 'Group',
      subtitle: `${group.mode} · ${describeGroupKind(group.kind)}`,
      children: mergeConsecutiveChars(rawChildren, id),
    }
  }

  if ('Command' in node) {
    const command = node.Command
    return {
      id,
      type: 'Command',
      commandName: `\\${command.name}${command.starred ? '*' : ''}`,
      subtitle: `${command.args.length} args`,
      children: command.args.map((arg, index) => buildArgumentNode(arg, `${id}.arg.${index}`, index)),
    }
  }

  if ('Infix' in node) {
    const infix = node.Infix
    const args = infix.args.map((arg, index) => buildArgumentNode(arg, `${id}.arg.${index}`, index))
    return {
      id,
      type: 'Infix',
      commandName: `\\${infix.name}${infix.starred ? '*' : ''}`,
      subtitle: `${infix.args.length} args`,
      children: [
        withRole(buildSyntaxTree(infix.left, `${id}.left`), 'left'),
        ...args,
        withRole(buildSyntaxTree(infix.right, `${id}.right`), 'right'),
      ],
    }
  }

  if ('Declarative' in node) {
    const declarative = node.Declarative
    const args = declarative.args.map((arg, index) =>
      buildArgumentNode(arg, `${id}.arg.${index}`, index),
    )
    return {
      id,
      type: 'Declarative',
      commandName: `\\${declarative.name}${declarative.starred ? '*' : ''}`,
      subtitle: `${declarative.args.length} args`,
      children: [...args, withRole(buildSyntaxTree(declarative.scope, `${id}.scope`), 'scope')],
    }
  }

  if ('Environment' in node) {
    const env = node.Environment
    const args = env.args.map((arg, index) => buildArgumentNode(arg, `${id}.arg.${index}`, index))
    return {
      id,
      type: 'Environment',
      commandName: `${env.name}${env.starred ? '*' : ''}`,
      subtitle: `${env.args.length} args`,
      children: [...args, withRole(buildSyntaxTree(env.body, `${id}.body`), 'body')],
    }
  }

  if ('Scripted' in node) {
    const scripted = node.Scripted
    const children: TreeNode[] = [withRole(buildSyntaxTree(scripted.base, `${id}.base`), 'base')]
    if (scripted.subscript) {
      children.push(withRole(buildSyntaxTree(scripted.subscript, `${id}.sub`), 'sub'))
    }
    if (scripted.superscript) {
      children.push(withRole(buildSyntaxTree(scripted.superscript, `${id}.sup`), 'sup'))
    }
    return {
      id,
      type: 'Scripted',
      children,
    }
  }

  return {
    id,
    type: 'UnknownNode',
    children: [],
  }
}

function buildArgumentNode(argument: Argument, id: string, index: number): TreeNode {
  const value = describeArgumentValue(argument.value)

  // Flatten: if the arg is Content with a single child, inline it
  if (value.content !== null) {
    const contentChild = buildSyntaxTree(value.content, `${id}.content`)
    // If the content child is a Group with children, we can still flatten
    // by promoting the content node and annotating it with arg info
    return {
      id,
      type: 'Arg',
      argKind: argument.kind,
      argIndex: index,
      subtitle: value.kind,
      value: value.value,
      children: [contentChild],
    }
  }

  return {
    id,
    type: 'Arg',
    argKind: argument.kind,
    argIndex: index,
    subtitle: value.kind,
    value: value.value,
    children: [],
  }
}

/**
 * Merge runs of consecutive Char leaf nodes into a single "Chars" node.
 * The merged node is expandable to reveal individual Char children.
 * Runs of length 1 are kept as-is.
 */
function mergeConsecutiveChars(nodes: TreeNode[], parentId: string): TreeNode[] {
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

function withRole(node: TreeNode, role: string): TreeNode {
  return { ...node, role }
}

function describeArgumentValue(value: ArgumentValue): {
  kind: string
  value?: string
  content: SyntaxNode | null
} {
  if ('Content' in value) {
    return {
      kind: 'Content',
      content: value.Content,
    }
  }
  if ('Delimiter' in value) {
    return {
      kind: 'Delimiter',
      value: describeDelimiter(value.Delimiter),
      content: null,
    }
  }
  if ('Dimension' in value) {
    return {
      kind: 'Dimension',
      value: value.Dimension,
      content: null,
    }
  }
  if ('Integer' in value) {
    return {
      kind: 'Integer',
      value: value.Integer,
      content: null,
    }
  }
  if ('KeyVal' in value) {
    return {
      kind: 'KeyVal',
      value: value.KeyVal,
      content: null,
    }
  }
  if ('Column' in value) {
    return {
      kind: 'Column',
      value: value.Column,
      content: null,
    }
  }
  return {
    kind: 'Unknown',
    content: null,
  }
}

function describeGroupKind(kind: GroupKind): string {
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

function describeDelimiter(delimiter: unknown): string {
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

function flattenTree(root: TreeNode): TreeNode[] {
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

function computeTreeDepth(root: TreeNode | null): number {
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

function quoted(value: string): string {
  const escaped = value.replace(/\n/g, '\\n').replace(/\t/g, '\\t')
  const clipped = escaped.length > 64 ? `${escaped.slice(0, 61)}...` : escaped
  return `"${clipped}"`
}

function extractFatalMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  return 'Unknown parsing failure'
}

function isParseResult(value: unknown): value is ParseResult {
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

export default App
