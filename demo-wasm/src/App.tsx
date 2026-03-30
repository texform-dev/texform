import { useEffect, useMemo, useState } from 'react'
import {
  ParseContext as WasmParseContext,
  type AllowedMode,
  type ArgSpecInfo,
  type BodyMode,
  type CharacterInfo,
  type CommandInfo,
  type CommandKind,
  type ContextItem,
  type EnvInfo,
  ensureWasmReady,
  type Argument,
  type ArgumentValue,
  type GroupKind,
  type ParseDiagnostic,
  type ParseResult,
  type SyntaxNode,
} from './texformWasm'
import type {
  CustomKnowledgeRecordEntry,
  CustomKnowledgeRecordTarget,
  TreeNode,
} from './appTypes'
import AppHeader from './components/AppHeader'
import LatexInputPane from './components/LatexInputPane'
import SyntaxTreePane from './components/SyntaxTreePane'

const SAMPLE_LATEX = ''
const CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY = 'texform-custom-knowledge-records'
const LEGACY_CUSTOM_COMMANDS_STORAGE_KEY = 'texform-custom-commands'

interface ParseViewState {
  result: ParseResult | null
  diagnostics: ParseDiagnostic[]
  fatalMessage: string | null
}

interface ParseThrowLike {
  diagnostics?: unknown
  partial_result?: unknown
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

function isAllowedMode(value: unknown): value is AllowedMode {
  return value === 'math' || value === 'text' || value === 'both'
}

function isCommandKind(value: unknown): value is CommandKind {
  return value === 'prefix' || value === 'infix' || value === 'declarative'
}

function isBodyMode(value: unknown): value is BodyMode {
  return value === 'math' || value === 'text'
}

function recordIdentity(
  record: Pick<CustomKnowledgeRecordEntry, 'target' | 'name'>,
): string {
  return `${record.target}:${record.name}`
}

function normalizeStoredCustomKnowledgeRecords(value: unknown): CustomKnowledgeRecordEntry[] {
  if (!Array.isArray(value)) {
    return []
  }

  const deduped = new Map<string, CustomKnowledgeRecordEntry>()

  for (const item of value) {
    if (typeof item !== 'object' || item === null) {
      continue
    }

    const candidate = item as Record<string, unknown>
    const name =
      typeof candidate.name === 'string'
        ? candidate.name.trim().replace(/^\\/, '')
        : ''
    if (!name) {
      continue
    }

    if (candidate.target === 'delimiter') {
      const record: CustomKnowledgeRecordEntry = {
        target: 'delimiter',
        name,
      }
      deduped.set(recordIdentity(record), record)
      continue
    }

    const spec = typeof candidate.spec === 'string' ? candidate.spec : ''
    const mode = candidate.mode
    if (!isAllowedMode(mode)) {
      continue
    }

    if (candidate.target === 'environment') {
      const bodyMode = candidate.bodyMode
      if (!isBodyMode(bodyMode)) {
        continue
      }

      const record: CustomKnowledgeRecordEntry = {
        target: 'environment',
        name,
        mode,
        bodyMode,
        spec,
      }
      deduped.set(recordIdentity(record), record)
      continue
    }

    const kind = candidate.kind
    if (!isCommandKind(kind)) {
      continue
    }

    const record: CustomKnowledgeRecordEntry = {
      target: 'command',
      name,
      kind,
      mode,
      spec,
    }
    deduped.set(recordIdentity(record), record)
  }

  return [...deduped.values()]
}

function loadStoredCustomKnowledgeRecords(): CustomKnowledgeRecordEntry[] {
  try {
    const raw =
      localStorage.getItem(CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY) ??
      localStorage.getItem(LEGACY_CUSTOM_COMMANDS_STORAGE_KEY)
    if (!raw) {
      return []
    }

    return normalizeStoredCustomKnowledgeRecords(JSON.parse(raw))
  } catch {
    return []
  }
}

function persistCustomKnowledgeRecords(records: CustomKnowledgeRecordEntry[]): void {
  try {
    localStorage.setItem(CUSTOM_KNOWLEDGE_RECORDS_STORAGE_KEY, JSON.stringify(records))
    localStorage.removeItem(LEGACY_CUSTOM_COMMANDS_STORAGE_KEY)
  } catch {
    // Ignore storage quota errors
  }
}

function insertCustomKnowledgeRecord(
  context: WasmParseContext,
  record: CustomKnowledgeRecordEntry,
): void {
  context.insertItem(customKnowledgeRecordToContextItem(record))
}

function removeCustomKnowledgeRecord(
  context: WasmParseContext,
  record: CustomKnowledgeRecordEntry,
): boolean {
  return context.removeItem(customKnowledgeRecordToContextItem(record))
}

function customKnowledgeRecordToContextItem(
  record: CustomKnowledgeRecordEntry,
): ContextItem {
  if (record.target === 'command') {
    return {
      target: 'command',
      name: record.name,
      kind: record.kind,
      allowed_mode: record.mode,
      spec: record.spec,
    }
  }

  if (record.target === 'delimiter') {
    return {
      target: 'delimiter',
      name: record.name,
    }
  }

  return {
    target: 'environment',
    name: record.name,
    allowed_mode: record.mode,
    body_mode: record.bodyMode,
    spec: record.spec,
  }
}

function App() {
  const [source, setSource] = useState(SAMPLE_LATEX)
  const [strictMode, setStrictMode] = useState(false)
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmInitError, setWasmInitError] = useState<string | null>(null)
  const [parseContext, setParseContext] = useState<WasmParseContext | null>(null)
  const [contextVersion, setContextVersion] = useState(0)
  const [collapsedNodes, setCollapsedNodes] = useState<Set<string>>(new Set())
  const [customRecordName, setCustomRecordName] = useState('')
  const [customCommandKind, setCustomCommandKind] = useState<CommandKind>('prefix')
  const [customRecordMode, setCustomRecordMode] = useState<AllowedMode>('math')
  const [customEnvironmentBodyMode, setCustomEnvironmentBodyMode] =
    useState<BodyMode>('math')
  const [customRecordSpec, setCustomRecordSpec] = useState('m')
  const [customRecordError, setCustomRecordError] = useState<string | null>(null)
  const [customKnowledgeRecords, setCustomKnowledgeRecords] = useState<
    CustomKnowledgeRecordEntry[]
  >(() => loadStoredCustomKnowledgeRecords())
  const [activeCustomRecordForm, setActiveCustomRecordForm] =
    useState<CustomKnowledgeRecordTarget | null>(null)

  useEffect(() => {
    let alive = true
    ensureWasmReady()
      .then(() => {
        if (!alive) {
          return
        }
        try {
          const ctx = new WasmParseContext()
          if (!alive) {
            return
          }

          // Restore persisted custom knowledge records into the new context
          try {
            const saved = loadStoredCustomKnowledgeRecords()
            const items = saved.map(customKnowledgeRecordToContextItem)
            if (items.length > 0) {
              ctx.insertItems(items)
            }
          } catch {
            // Ignore malformed localStorage data
          }

          setParseContext(ctx)
          setWasmReady(true)
          setWasmInitError(null)
        } catch (error) {
          if (!alive) {
            return
          }
          setWasmReady(false)
          setWasmInitError(extractFatalMessage(error))
        }
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
    if (!wasmReady || !parseContext) {
      return {
        result: null,
        diagnostics: [],
        fatalMessage: wasmInitError,
      }
    }

    try {
      const parsed = parseContext.parse(source, strictMode)
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
  }, [source, strictMode, wasmReady, wasmInitError, parseContext, contextVersion])

  const treeRoot = useMemo(() => {
    if (!parseState.result || !parseContext) {
      return null
    }
    return buildSyntaxTree(
      parseState.result.node,
      'root',
      (name) => parseContext.lookupActiveCommand(name),
      (name) => parseContext.lookupExplicitCommand(name),
      (name) => parseContext.lookupCharacter(name),
      (name) => parseContext.lookupEnv(name),
    )
  }, [parseState.result, parseContext, contextVersion])

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

  const parseErrorMessage = useMemo(() => {
    const hasFatal = parseState.fatalMessage !== null
    const hasDiagnosticsOnlyFailure =
      parseState.result === null && parseState.diagnostics.length > 0
    if (!hasFatal && !hasDiagnosticsOnlyFailure) {
      return null
    }
    return formatParseErrorMessage(parseState.fatalMessage, parseState.diagnostics)
  }, [parseState.result, parseState.fatalMessage, parseState.diagnostics])

  const isWasmLoading = !wasmReady && wasmInitError === null

  const statusText = isWasmLoading
    ? 'WASM Initializing'
    : parseState.fatalMessage !== null
      ? 'Parse Failed'
      : parseState.diagnostics.length > 0
        ? 'Partial Parse'
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
    'rounded-sm border border-slate-300 bg-slate-50 px-2 py-1 text-xs leading-tight transition-colors hover:bg-slate-100'

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

  const toggleCustomRecordForm = (target: CustomKnowledgeRecordTarget) => {
    setCustomRecordError(null)
    setActiveCustomRecordForm((current) => (current === target ? null : target))
  }

  const addCustomCommand = () => {
    if (!parseContext) {
      setCustomRecordError('Parse context is not ready.')
      return
    }

    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Command name is required.')
      return
    }

    const nextRecord: CustomKnowledgeRecordEntry = {
      target: 'command',
      name,
      kind: customCommandKind,
      mode: customRecordMode,
      spec: customRecordSpec,
    }

    try {
      insertCustomKnowledgeRecord(parseContext, nextRecord)
      const updated = [
        ...customKnowledgeRecords.filter(
          (entry) => recordIdentity(entry) !== recordIdentity(nextRecord),
        ),
        nextRecord,
      ]
      setCustomKnowledgeRecords(updated)
      persistCustomKnowledgeRecords(updated)
      setCustomRecordError(null)
      setContextVersion((v) => v + 1)
      setCustomRecordName('')
      setActiveCustomRecordForm(null)
    } catch (error) {
      setCustomRecordError(extractFatalMessage(error))
    }
  }

  const addCustomEnvironment = () => {
    if (!parseContext) {
      setCustomRecordError('Parse context is not ready.')
      return
    }

    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Environment name is required.')
      return
    }

    const nextRecord: CustomKnowledgeRecordEntry = {
      target: 'environment',
      name,
      mode: customRecordMode,
      bodyMode: customEnvironmentBodyMode,
      spec: customRecordSpec,
    }

    try {
      insertCustomKnowledgeRecord(parseContext, nextRecord)
      const updated = [
        ...customKnowledgeRecords.filter(
          (entry) => recordIdentity(entry) !== recordIdentity(nextRecord),
        ),
        nextRecord,
      ]
      setCustomKnowledgeRecords(updated)
      persistCustomKnowledgeRecords(updated)
      setCustomRecordError(null)
      setContextVersion((v) => v + 1)
      setCustomRecordName('')
      setActiveCustomRecordForm(null)
    } catch (error) {
      setCustomRecordError(extractFatalMessage(error))
    }
  }

  const addCustomDelimiter = () => {
    if (!parseContext) {
      setCustomRecordError('Parse context is not ready.')
      return
    }

    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Delimiter name is required.')
      return
    }

    const nextRecord: CustomKnowledgeRecordEntry = {
      target: 'delimiter',
      name,
    }

    try {
      insertCustomKnowledgeRecord(parseContext, nextRecord)
      const updated = [
        ...customKnowledgeRecords.filter(
          (entry) => recordIdentity(entry) !== recordIdentity(nextRecord),
        ),
        nextRecord,
      ]
      setCustomKnowledgeRecords(updated)
      persistCustomKnowledgeRecords(updated)
      setCustomRecordError(null)
      setContextVersion((v) => v + 1)
      setCustomRecordName('')
      setActiveCustomRecordForm(null)
    } catch (error) {
      setCustomRecordError(extractFatalMessage(error))
    }
  }

  const removeCustomRecord = (record: CustomKnowledgeRecordEntry) => {
    if (!parseContext) {
      return
    }

    try {
      removeCustomKnowledgeRecord(parseContext, record)
      const updated = customKnowledgeRecords.filter(
        (entry) => recordIdentity(entry) !== recordIdentity(record),
      )
      setCustomKnowledgeRecords(updated)
      persistCustomKnowledgeRecords(updated)
      setCustomRecordError(null)
      setContextVersion((v) => v + 1)
    } catch (error) {
      setCustomRecordError(extractFatalMessage(error))
    }
  }

  const resetAllCustomKnowledgeRecords = () => {
    if (!parseContext) {
      return
    }
    for (const record of customKnowledgeRecords) {
      try {
        removeCustomKnowledgeRecord(parseContext, record)
      } catch {
        // best-effort removal
      }
    }
    setCustomKnowledgeRecords([])
    persistCustomKnowledgeRecords([])
    setCustomRecordError(null)
    setContextVersion((v) => v + 1)
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
              className="h-4 w-4 rounded-sm border-0 bg-transparent p-0 text-center text-sm leading-4 text-slate-500 hover:bg-slate-200"
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
            <span className="rounded-sm bg-purple-50 px-1 py-px text-xs leading-none text-purple-600">
              {node.role}
            </span>
          ) : null}

          {/* Type badge (with inline arg index when applicable) */}
          <span
            className={`inline-flex items-baseline gap-1 rounded-sm px-1 py-px text-xs font-medium leading-none ${tone.bg} ${tone.text}`}
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

          {node.specString !== undefined ? (
            <SpecPopover
              specString={node.specString}
              specFromPackages={node.specFromPackages}
              specDetail={node.specDetail}
              explicitSpecString={node.explicitSpecString}
              explicitSpecFromPackages={node.explicitSpecFromPackages}
              explicitSpecDetail={node.explicitSpecDetail}
              characterUnicodeValue={node.characterUnicodeValue}
              characterPackage={node.characterPackage}
              characterMathvariant={node.characterMathvariant}
            />
          ) : null}

          {/* Arg kind — only show "optional" since mandatory is the default */}
          {node.argKind === 'Optional' ? (
            <span className="rounded-sm border border-amber-200 bg-amber-50 px-1 py-px text-xs leading-none text-amber-700">
              opt
            </span>
          ) : null}

          {/* Subtitle (group kind, arg count, etc.) */}
          {node.subtitle ? (
            <span className="text-xs text-slate-400">{node.subtitle}</span>
          ) : null}

          {/* Value */}
          {node.value ? (
            <span className="text-blue-600">{node.value}</span>
          ) : null}
        </div>
        {hasChildren && !collapsed ? (
          <div className="ml-2 border-l border-slate-200 pl-2.5">
            {node.children.map((child) => renderTreeNode(child))}
          </div>
        ) : null}
      </div>
    )
  }

  return (
    <div className="flex min-h-full flex-col p-3.5">
      <AppHeader />

      <main className="grid min-h-0 flex-1 grid-cols-1 gap-3.5 lg:grid-cols-[minmax(300px,1fr)_minmax(0,2fr)]">
        <LatexInputPane
          paneClass={paneClass}
          sectionHeadClass={sectionHeadClass}
          sectionTitleClass={sectionTitleClass}
          buttonClass={buttonClass}
          source={source}
          strictMode={strictMode}
          fatalMessage={parseState.fatalMessage}
          diagnostics={parseState.diagnostics}
          customKnowledgeRecords={customKnowledgeRecords}
          activeCustomRecordForm={activeCustomRecordForm}
          customRecordName={customRecordName}
          customCommandKind={customCommandKind}
          customRecordMode={customRecordMode}
          customEnvironmentBodyMode={customEnvironmentBodyMode}
          customRecordSpec={customRecordSpec}
          customRecordError={customRecordError}
          rootSpanText={
            parseState.result
              ? `${parseState.result.span.start}..${parseState.result.span.end}`
              : '--'
          }
          treeDepth={treeDepth}
          nodesCount={flatNodes.length}
          onResetSample={() => setSource(SAMPLE_LATEX)}
          onStrictModeChange={setStrictMode}
          onSourceChange={setSource}
          onToggleCustomRecordForm={toggleCustomRecordForm}
          onCustomRecordNameChange={setCustomRecordName}
          onCustomRecordSpecChange={setCustomRecordSpec}
          onCustomCommandKindChange={setCustomCommandKind}
          onCustomRecordModeChange={setCustomRecordMode}
          onCustomEnvironmentBodyModeChange={setCustomEnvironmentBodyMode}
          onAddCustomCommand={addCustomCommand}
          onAddCustomEnvironment={addCustomEnvironment}
          onAddCustomDelimiter={addCustomDelimiter}
          onRemoveCustomRecord={removeCustomRecord}
          onResetAllCustomKnowledgeRecords={resetAllCustomKnowledgeRecords}
        />

        <SyntaxTreePane
          paneClass={paneClass}
          sectionHeadClass={sectionHeadClass}
          sectionTitleClass={sectionTitleClass}
          buttonClass={buttonClass}
          statusText={statusText}
          statusToneClass={statusToneClass}
          treeRoot={treeRoot}
          parseErrorMessage={parseErrorMessage}
          onExpandAll={expandAll}
          onCollapseAll={collapseAll}
          renderTreeNode={renderTreeNode}
        />
      </main>
    </div>
  )
}

// -- Spec popover component --

function formatPackageList(packages?: string[]): string {
  return packages && packages.length > 0 ? packages.join(', ') : 'unknown'
}

function sameStringList(left?: string[], right?: string[]): boolean {
  if (left === right) {
    return true
  }
  if (!left || !right || left.length !== right.length) {
    return false
  }
  return left.every((value, index) => value === right[index])
}

function SpecPopover({
  specString,
  specFromPackages,
  specDetail,
  explicitSpecString,
  explicitSpecFromPackages,
  explicitSpecDetail,
  characterUnicodeValue,
  characterPackage,
  characterMathvariant,
}: {
  specString: string
  specFromPackages?: string[]
  specDetail?: string
  explicitSpecString?: string
  explicitSpecFromPackages?: string[]
  explicitSpecDetail?: string
  characterUnicodeValue?: string
  characterPackage?: string
  characterMathvariant?: string
}) {
  const [show, setShow] = useState(false)
  const activePackageLabel = formatPackageList(specFromPackages)
  const explicitPackageLabel = formatPackageList(explicitSpecFromPackages)
  const showExplicitSection =
    explicitSpecString !== undefined &&
    (explicitSpecString !== specString ||
      explicitSpecDetail !== specDetail ||
      !sameStringList(explicitSpecFromPackages, specFromPackages))

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
    >
      <span className="cursor-help rounded-sm border border-emerald-200 bg-emerald-50 px-1 py-px text-xs leading-none text-emerald-700">
        spec
      </span>
      {show ? (
        <div className="absolute left-0 top-full z-50 mt-1 w-max max-w-sm rounded-md border border-slate-200 bg-white p-2.5 shadow-lg">
          <div className="space-y-2 text-xs">
            <div className="space-y-1.5">
              <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400">
                active
              </div>
              <div className="flex items-baseline gap-2">
                <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                  spec
                </span>
                <code className="rounded bg-violet-50 px-1.5 py-px font-semibold text-violet-700 [font-family:var(--font-code)]">
                  {specString || '(empty)'}
                </code>
              </div>
              <div className="flex items-baseline gap-2">
                <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                  packages
                </span>
                <span className="rounded bg-sky-50 px-1.5 py-px text-sky-700">
                  {activePackageLabel}
                </span>
              </div>
              {specDetail ? <SpecArgsList detail={specDetail} /> : null}
            </div>

            {showExplicitSection ? (
              <div className="space-y-1.5 border-t border-slate-100 pt-2">
                <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400">
                  explicit
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                    spec
                  </span>
                  <code className="rounded bg-amber-50 px-1.5 py-px font-semibold text-amber-700 [font-family:var(--font-code)]">
                    {explicitSpecString || '(empty)'}
                  </code>
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                    packages
                  </span>
                  <span className="rounded bg-amber-50 px-1.5 py-px text-amber-700">
                    {explicitPackageLabel}
                  </span>
                </div>
                {explicitSpecDetail ? <SpecArgsList detail={explicitSpecDetail} /> : null}
              </div>
            ) : null}

            {characterUnicodeValue ? (
              <div className="space-y-1.5 border-t border-slate-100 pt-2">
                <div className="text-[11px] font-semibold uppercase tracking-wider text-slate-400">
                  character
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                    unicode
                  </span>
                  <code className="rounded bg-emerald-50 px-1.5 py-px font-semibold text-emerald-700 [font-family:var(--font-code)]">
                    {characterUnicodeValue}
                  </code>
                </div>
                <div className="flex items-baseline gap-2">
                  <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                    package
                  </span>
                  <span className="rounded bg-emerald-50 px-1.5 py-px text-emerald-700">
                    {characterPackage ?? 'unknown'}
                  </span>
                </div>
                {characterMathvariant ? (
                  <div className="flex items-baseline gap-2">
                    <span className="w-14 shrink-0 text-right text-xs font-semibold uppercase tracking-wider text-slate-400">
                      variant
                    </span>
                    <code className="rounded bg-emerald-50 px-1.5 py-px font-semibold text-emerald-700 [font-family:var(--font-code)]">
                      {characterMathvariant}
                    </code>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </span>
  )
}

/** Render each arg line with colored tokens. */
function SpecArgsList({ detail }: { detail: string }) {
  const lines = detail.split('\n')
  return (
    <div className="space-y-0.5">
      {lines.map((line, idx) => {
        // Parse format: "[0] required standard content(math)"
        const m = line.match(/^\[(\d+)\]\s+(required|optional)\s+(\S+)\s+(.*)/)
        if (!m) {
          return (
            <div key={idx} className="text-xs text-slate-500 [font-family:var(--font-code)]">
              {line}
            </div>
          )
        }
        const [, index, req, form, kind] = m
        return (
          <div
            key={idx}
            className="flex items-baseline gap-1 text-xs [font-family:var(--font-code)]"
          >
            <span className="w-14 shrink-0 text-right text-slate-400">[{index}]</span>
            <span
              className={`rounded px-1 py-px ${
                req === 'required'
                  ? 'bg-orange-50 text-orange-600'
                  : 'bg-slate-100 text-slate-500'
              }`}
            >
              {req}
            </span>
            <span className="rounded bg-indigo-50 px-1 py-px text-indigo-600">{form}</span>
            <span className="rounded bg-emerald-50 px-1 py-px text-emerald-600">{kind}</span>
          </div>
        )
      })}
    </div>
  )
}

// -- Tree building --

function buildSyntaxTree(
  node: SyntaxNode,
  id: string,
  lookupActiveCommand: (name: string) => CommandInfo | null,
  lookupExplicitCommand: (name: string) => CommandInfo | null,
  lookupCharacter: (name: string) => CharacterInfo | null,
  lookupEnv: (name: string) => EnvInfo | null,
): TreeNode {
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
      commandName: `\\${command.name}`,
      children: [],
    }
  }

  if ('Group' in node) {
    const group = node.Group
    const rawChildren = group.children.map((child: SyntaxNode, index: number) =>
      buildSyntaxTree(
        child,
        `${id}.child.${index}`,
        lookupActiveCommand,
        lookupExplicitCommand,
        lookupCharacter,
        lookupEnv,
      ),
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
    const activeSpec = lookupActiveCommand(command.name)
    const explicitSpec = lookupExplicitCommand(command.name)
    const character = lookupCharacter(command.name)
    return {
      id,
      type: 'Command',
      commandName: `\\${command.name}`,
      subtitle: `${command.args.length} args`,
      specString: activeSpec?.spec_string,
      specFromPackages: activeSpec?.from_packages,
      specDetail: activeSpec ? formatSpecDetail(activeSpec.args) : undefined,
      explicitSpecString: explicitSpec?.spec_string,
      explicitSpecFromPackages: explicitSpec?.from_packages,
      explicitSpecDetail: explicitSpec ? formatSpecDetail(explicitSpec.args) : undefined,
      characterUnicodeValue: character?.unicode_value,
      characterPackage: character?.package,
      characterMathvariant: character?.attributes.mathvariant,
      children: command.args.map((arg: Argument | null, index: number) =>
        buildArgumentNode(
          arg,
          `${id}.arg.${index}`,
          index,
          lookupActiveCommand,
          lookupExplicitCommand,
          lookupCharacter,
          lookupEnv,
        ),
      ),
    }
  }

  if ('Infix' in node) {
    const infix = node.Infix
    const activeSpec = lookupActiveCommand(infix.name)
    const explicitSpec = lookupExplicitCommand(infix.name)
    const character = lookupCharacter(infix.name)
    const args = infix.args.map((arg: Argument | null, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        lookupActiveCommand,
        lookupExplicitCommand,
        lookupCharacter,
        lookupEnv,
      ),
    )
    return {
      id,
      type: 'Infix',
      commandName: `\\${infix.name}`,
      subtitle: `${infix.args.length} args`,
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
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
          ),
          'left',
        ),
        ...args,
        withRole(
          buildSyntaxTree(
            infix.right,
            `${id}.right`,
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
          ),
          'right',
        ),
      ],
    }
  }

  if ('Declarative' in node) {
    const declarative = node.Declarative
    const activeSpec = lookupActiveCommand(declarative.name)
    const explicitSpec = lookupExplicitCommand(declarative.name)
    const character = lookupCharacter(declarative.name)
    const args = declarative.args.map((arg: Argument | null, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        lookupActiveCommand,
        lookupExplicitCommand,
        lookupCharacter,
        lookupEnv,
      ),
    )
    return {
      id,
      type: 'Declarative',
      commandName: `\\${declarative.name}`,
      subtitle: `${declarative.args.length} args`,
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
        ...args,
        withRole(
          buildSyntaxTree(
            declarative.scope,
            `${id}.scope`,
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
          ),
          'scope',
        ),
      ],
    }
  }

  if ('Environment' in node) {
    const env = node.Environment
    const spec = lookupEnv(env.name)
    const args = env.args.map((arg: Argument | null, index: number) =>
      buildArgumentNode(
        arg,
        `${id}.arg.${index}`,
        index,
        lookupActiveCommand,
        lookupExplicitCommand,
        lookupCharacter,
        lookupEnv,
      ),
    )
    return {
      id,
      type: 'Environment',
      commandName: env.name,
      subtitle: `${env.args.length} args`,
      specString: spec?.spec_string,
      specFromPackages: spec?.from_packages,
      specDetail: spec ? formatSpecDetail(spec.args) : undefined,
      children: [
        ...args,
        withRole(
          buildSyntaxTree(
            env.body,
            `${id}.body`,
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
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
          lookupActiveCommand,
          lookupExplicitCommand,
          lookupCharacter,
          lookupEnv,
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
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
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
            lookupActiveCommand,
            lookupExplicitCommand,
            lookupCharacter,
            lookupEnv,
          ),
          'sup',
        ),
      )
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

function buildArgumentNode(
  argument: Argument | null,
  id: string,
  index: number,
  lookupActiveCommand: (name: string) => CommandInfo | null,
  lookupExplicitCommand: (name: string) => CommandInfo | null,
  lookupCharacter: (name: string) => CharacterInfo | null,
  lookupEnv: (name: string) => EnvInfo | null,
): TreeNode {
  if (argument === null) {
    return {
      id,
      type: 'Arg',
      argIndex: index,
      subtitle: 'missing',
      children: [],
    }
  }

  const value = describeArgumentValue(argument.value)

  // Flatten: if the arg is Content with a single child, inline it
  if (value.content !== null) {
    const contentChild = buildSyntaxTree(
      value.content,
      `${id}.content`,
      lookupActiveCommand,
      lookupExplicitCommand,
      lookupCharacter,
      lookupEnv,
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

function describeArgumentKind(kind: Argument['kind']): string {
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

function formatSpecDetail(args: ArgSpecInfo[]): string {
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

function describeArgSpecKind(kind: unknown): string {
  if (typeof kind === 'string') return kind
  if (kind && typeof kind === 'object' && 'type' in kind) {
    const t = kind as { type: string; mode?: string }
    if (t.type === 'content' && t.mode) return `content(${t.mode})`
    return t.type
  }
  return 'unknown'
}

function describeArgSpecForm(form: unknown): string {
  if (typeof form === 'string') return form
  if (form && typeof form === 'object' && 'type' in form) {
    const f = form as { type: string }
    return f.type
  }
  return ''
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
  if ('CSName' in value) {
    return {
      kind: 'CSName',
      value: value.CSName,
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

function formatParseErrorMessage(
  fatalMessage: string | null,
  diagnostics: ParseDiagnostic[],
): string {
  const sections: string[] = []
  if (fatalMessage !== null) {
    sections.push(fatalMessage)
  }
  if (diagnostics.length > 0) {
    const detailLines = diagnostics.map(
      (diagnostic, index) =>
        `${index + 1}. ${diagnostic.message} (span ${diagnostic.span.start}..${diagnostic.span.end})`,
    )
    sections.push(`Diagnostics:\n${detailLines.join('\n')}`)
  }
  if (sections.length === 0) {
    return 'Unknown parsing failure'
  }
  return sections.join('\n\n')
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
