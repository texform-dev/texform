import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { type Layout, Group, Panel, Separator } from 'react-resizable-panels'

const LAYOUT_STORAGE_KEY = 'texform-playground-layout'
const LEFT_PANEL_ID = 'left'
const RIGHT_PANEL_ID = 'right'
const DEFAULT_LAYOUT: Layout = { [LEFT_PANEL_ID]: 38, [RIGHT_PANEL_ID]: 62 }

function loadSavedLayout(): Layout | undefined {
  try {
    const raw = localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (!raw) return undefined
    const parsed = JSON.parse(raw) as unknown
    if (parsed !== null && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Layout
    }
  } catch {
    // ignore
  }
  return undefined
}
import { ChevronsDownUp, ChevronsUpDown, Settings2 } from 'lucide-react'

import Header from './components/Header'
import InputSection from './components/InputSection'
import KnowledgeSection from './components/KnowledgeSection'
import SyntaxTreeTab from './components/SyntaxTreeTab'
import SerializedTab from './components/SerializedTab'

import { useMediaQuery } from './lib/useMediaQuery'
import { type Theme, applyTheme, getInitialTheme, toggleTheme } from './lib/theme'
import {
  type AllowedMode,
  type BodyMode,
  type CommandKind,
  type ParseDiagnostic,
  type ParseResult,
  ParseContext,
  ensureWasmReady,
  serializeLatex,
} from './lib/texformWasm'
import {
  buildSyntaxTree,
  flattenTree,
  computeTreeDepth,
  extractFatalMessage,
  isParseResult,
} from './lib/treeBuilder'
import {
  loadStoredCustomKnowledgeRecords,
  persistCustomKnowledgeRecords,
  buildParseContext,
  recordIdentity,
} from './lib/knowledgeRecords'
import type { CustomKnowledgeRecordEntry, CustomKnowledgeRecordTarget } from './lib/types'
import type { SerializeOptions } from './schema/serializeOptions'
import { DEFAULT_SERIALIZE_OPTIONS } from './schema/serializeOptions'
import { readStateFromUrl, buildShareUrl, copyToClipboard } from './lib/urlState'

const SAMPLE_LATEX = ''

interface ParseViewState {
  result: ParseResult | null
  diagnostics: ParseDiagnostic[]
  fatalMessage: string | null
  parseTime: number | null
}

interface ParseThrowLike {
  diagnostics?: unknown
  partial_result?: unknown
}

// Read URL state once, outside the component, to avoid effect-based setState
const initialUrlState = readStateFromUrl()

export default function App() {
  // -- Responsive --
  const isDesktop = useMediaQuery('(min-width: 1024px)')

  // -- Theme --
  const [theme, setTheme] = useState<Theme>(getInitialTheme)

  const handleToggleTheme = useCallback(() => {
    setTheme((prev) => {
      const next = toggleTheme(prev)
      applyTheme(next)
      return next
    })
  }, [])

  // -- Core state (URL params override defaults) --
  const [source, setSource] = useState(initialUrlState.source ?? SAMPLE_LATEX)
  const [strictMode, setStrictMode] = useState(initialUrlState.strict ?? false)
  const [wasmReady, setWasmReady] = useState(false)
  const [wasmInitError, setWasmInitError] = useState<string | null>(null)
  const [parseContext, setParseContext] = useState<ParseContext | null>(null)

  // -- Panel layout state --
  const [layout] = useState<Layout>(() => loadSavedLayout() ?? DEFAULT_LAYOUT)

  const handleLayoutChange = useCallback((newLayout: Layout) => {
    localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(newLayout))
  }, [])

  // -- Panel state --
  const [inputCollapsed, setInputCollapsed] = useState(false)
  const [knowledgeCollapsed, setKnowledgeCollapsed] = useState(true)
  const [rightTab, setRightTab] = useState<'tree' | 'serialized'>(
    initialUrlState.tab ?? 'tree',
  )
  const [collapsedNodes, setCollapsedNodes] = useState<Set<string>>(new Set())
  const [serializeOptions, setSerializeOptions] = useState<SerializeOptions>(
    initialUrlState.serializeOptions ?? DEFAULT_SERIALIZE_OPTIONS,
  )
  const [optionsPanelOpen, setOptionsPanelOpen] = useState(true)

  // -- Knowledge record form state --
  const [customKnowledgeRecords, setCustomKnowledgeRecords] = useState<
    CustomKnowledgeRecordEntry[]
  >(() => loadStoredCustomKnowledgeRecords())
  const [activeCustomRecordForm, setActiveCustomRecordForm] =
    useState<CustomKnowledgeRecordTarget | null>(null)
  const [customRecordName, setCustomRecordName] = useState('')
  const [customCommandKind, setCustomCommandKind] = useState<CommandKind>('prefix')
  const [customRecordMode, setCustomRecordMode] = useState<AllowedMode>('math')
  const [customEnvironmentBodyMode, setCustomEnvironmentBodyMode] = useState<BodyMode>('math')
  const [customRecordSpec, setCustomRecordSpec] = useState('m')
  const [customRecordError, setCustomRecordError] = useState<string | null>(null)

  // -- Share toast --
  const [shareToast, setShareToast] = useState(false)
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // -- WASM initialization --
  useEffect(() => {
    let alive = true
    ensureWasmReady()
      .then(() => {
        if (!alive) return
        try {
          const saved = loadStoredCustomKnowledgeRecords()
          let restored = saved
          let ctx: ParseContext

          try {
            ctx = buildParseContext(saved)
          } catch {
            restored = []
            persistCustomKnowledgeRecords([])
            ctx = new ParseContext()
          }

          if (!alive) return
          setCustomKnowledgeRecords(restored)
          setParseContext(ctx)
          setWasmReady(true)
          setWasmInitError(null)
        } catch (error) {
          if (!alive) return
          setWasmReady(false)
          setWasmInitError(extractFatalMessage(error))
        }
      })
      .catch((error) => {
        if (!alive) return
        setWasmReady(false)
        setWasmInitError(extractFatalMessage(error))
      })

    return () => {
      alive = false
    }
  }, [])

  // -- Derived: parse state --
  const parseState = useMemo<ParseViewState>(() => {
    if (!wasmReady || !parseContext) {
      return { result: null, diagnostics: [], fatalMessage: wasmInitError, parseTime: null }
    }
    const t0 = performance.now()
    try {
      const parsed = parseContext.parse(source, strictMode)
      return { result: parsed, diagnostics: [], fatalMessage: null, parseTime: performance.now() - t0 }
    } catch (error) {
      const parseTime = performance.now() - t0
      const thrown = (error ?? {}) as ParseThrowLike
      const diagnostics = Array.isArray(thrown.diagnostics)
        ? (thrown.diagnostics as ParseDiagnostic[])
        : []
      const partial = isParseResult(thrown.partial_result) ? thrown.partial_result : null
      const fatalMessage = diagnostics.length > 0 ? null : extractFatalMessage(error)
      return { result: partial, diagnostics, fatalMessage, parseTime }
    }
  }, [source, strictMode, wasmReady, wasmInitError, parseContext])

  // -- Derived: serialized state --
  const serializedState = useMemo<{ output: string | null; error: string | null; serializeTime: number | null }>(() => {
    if (!wasmReady || !source) {
      return { output: null, error: null, serializeTime: null }
    }
    const t0 = performance.now()
    try {
      const output = serializeLatex(source, strictMode, serializeOptions)
      return { output, error: null, serializeTime: performance.now() - t0 }
    } catch (error) {
      return { output: null, error: extractFatalMessage(error), serializeTime: performance.now() - t0 }
    }
  }, [source, strictMode, serializeOptions, wasmReady])

  // -- Derived: syntax tree --
  const treeRoot = useMemo(() => {
    if (!parseState.result || !parseContext) return null
    return buildSyntaxTree(
      parseState.result.node,
      'root',
      (name) => parseContext.lookupActiveCommand(name),
      (name) => parseContext.lookupExplicitCommand(name),
      (name) => parseContext.lookupCharacter(name),
      (name) => parseContext.lookupEnv(name),
    )
  }, [parseState.result, parseContext])

  const flatNodes = useMemo(() => (treeRoot ? flattenTree(treeRoot) : []), [treeRoot])
  const treeDepth = useMemo(() => computeTreeDepth(treeRoot), [treeRoot])

  // -- Derived: status --
  const isWasmLoading = !wasmReady && wasmInitError === null

  const statusText = isWasmLoading
    ? 'WASM Initializing'
    : parseState.fatalMessage !== null
      ? 'Parse Failed'
      : parseState.diagnostics.length > 0
        ? 'Partial Parse'
        : 'Parse OK'

  const statusVariant: 'loading' | 'ok' | 'warn' | 'error' = isWasmLoading
    ? 'loading'
    : parseState.fatalMessage !== null
      ? 'error'
      : parseState.diagnostics.length > 0
        ? 'warn'
        : 'ok'

  // -- Tree node toggling --
  const toggleNode = useCallback((nodeId: string) => {
    setCollapsedNodes((prev) => {
      const next = new Set(prev)
      if (next.has(nodeId)) {
        next.delete(nodeId)
      } else {
        next.add(nodeId)
      }
      return next
    })
  }, [])

  const expandAll = useCallback(() => {
    setCollapsedNodes(new Set())
  }, [])

  const collapseAll = useCallback(() => {
    const nodes = flatNodes
    if (nodes.length === 0) {
      setCollapsedNodes(new Set())
      return
    }
    const next = new Set(nodes.map((n) => n.id))
    // Keep root expanded
    if (treeRoot) next.delete(treeRoot.id)
    setCollapsedNodes(next)
  }, [flatNodes, treeRoot])

  // -- Knowledge record CRUD --
  const toggleCustomRecordForm = useCallback((target: CustomKnowledgeRecordTarget) => {
    setCustomRecordError(null)
    setActiveCustomRecordForm((current) => (current === target ? null : target))
  }, [])

  const applyRecordUpdate = useCallback(
    (record: CustomKnowledgeRecordEntry) => {
      if (!wasmReady) {
        setCustomRecordError('Parse context is not ready.')
        return
      }
      try {
        const updated = [
          ...customKnowledgeRecords.filter(
            (entry) => recordIdentity(entry) !== recordIdentity(record),
          ),
          record,
        ]
        const ctx = buildParseContext(updated)
        setParseContext(ctx)
        setCustomKnowledgeRecords(updated)
        persistCustomKnowledgeRecords(updated)
        setCustomRecordError(null)
        setCustomRecordName('')
        setActiveCustomRecordForm(null)
      } catch (error) {
        setCustomRecordError(extractFatalMessage(error))
      }
    },
    [wasmReady, customKnowledgeRecords],
  )

  const addCustomCommand = useCallback(() => {
    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Command name is required.')
      return
    }
    applyRecordUpdate({
      target: 'command',
      name,
      kind: customCommandKind,
      mode: customRecordMode,
      argspec: customRecordSpec,
    })
  }, [customRecordName, customCommandKind, customRecordMode, customRecordSpec, applyRecordUpdate])

  const addCustomEnvironment = useCallback(() => {
    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Environment name is required.')
      return
    }
    applyRecordUpdate({
      target: 'environment',
      name,
      mode: customRecordMode,
      bodyMode: customEnvironmentBodyMode,
      argspec: customRecordSpec,
    })
  }, [customRecordName, customRecordMode, customEnvironmentBodyMode, customRecordSpec, applyRecordUpdate])

  const addCustomDelimiter = useCallback(() => {
    const name = customRecordName.trim().replace(/^\\/, '')
    if (!name) {
      setCustomRecordError('Delimiter name is required.')
      return
    }
    applyRecordUpdate({ target: 'delimiter', name })
  }, [customRecordName, applyRecordUpdate])

  const removeCustomRecord = useCallback(
    (record: CustomKnowledgeRecordEntry) => {
      if (!wasmReady) return
      try {
        const updated = customKnowledgeRecords.filter(
          (entry) => recordIdentity(entry) !== recordIdentity(record),
        )
        const ctx = buildParseContext(updated)
        setParseContext(ctx)
        setCustomKnowledgeRecords(updated)
        persistCustomKnowledgeRecords(updated)
        setCustomRecordError(null)
      } catch (error) {
        setCustomRecordError(extractFatalMessage(error))
      }
    },
    [wasmReady, customKnowledgeRecords],
  )

  const resetAllCustomKnowledgeRecords = useCallback(() => {
    if (!wasmReady) return
    const ctx = buildParseContext([])
    setParseContext(ctx)
    setCustomKnowledgeRecords([])
    persistCustomKnowledgeRecords([])
    setCustomRecordError(null)
  }, [wasmReady])

  // -- Share handler --
  const handleShare = useCallback(async () => {
    const url = buildShareUrl({ source, strict: strictMode, tab: rightTab, serializeOptions })
    const ok = await copyToClipboard(url)
    if (ok) {
      setShareToast(true)
      if (toastTimer.current) clearTimeout(toastTimer.current)
      toastTimer.current = setTimeout(() => setShareToast(false), 2000)
    }
  }, [source, strictMode, rightTab, serializeOptions])

  // -- Render --
  return (
    <div className="flex h-dvh flex-col overflow-hidden">
      <Header
        theme={theme}
        onToggleTheme={handleToggleTheme}
        statusText={statusText}
        statusVariant={statusVariant}
        onShare={handleShare}
      />

      <Group
        orientation={isDesktop ? 'horizontal' : 'vertical'}
        id="playground-panels"
        className="flex-1"
        defaultLayout={layout}
        onLayoutChanged={handleLayoutChange}
      >
        {/* Left panel: input + knowledge */}
        <Panel id={LEFT_PANEL_ID} defaultSize="38%" minSize="20%" maxSize="75%">
          <div
            className="flex h-full flex-col"
            style={{ background: 'var(--color-canvas-default)' }}
          >
            <InputSection
              source={source}
              strictMode={strictMode}
              collapsed={inputCollapsed}
              fatalMessage={parseState.fatalMessage}
              diagnostics={parseState.diagnostics}
              theme={theme}
              onSourceChange={setSource}
              onStrictModeChange={setStrictMode}
              onResetSample={() => setSource(SAMPLE_LATEX)}
              onToggleCollapsed={() => setInputCollapsed((p) => !p)}
            />
            <KnowledgeSection
              collapsed={knowledgeCollapsed}
              onToggleCollapsed={() => setKnowledgeCollapsed((p) => !p)}
              customKnowledgeRecords={customKnowledgeRecords}
              activeForm={activeCustomRecordForm}
              name={customRecordName}
              spec={customRecordSpec}
              mode={customRecordMode}
              commandKind={customCommandKind}
              bodyMode={customEnvironmentBodyMode}
              error={customRecordError}
              onToggleForm={toggleCustomRecordForm}
              onNameChange={setCustomRecordName}
              onSpecChange={setCustomRecordSpec}
              onModeChange={setCustomRecordMode}
              onCommandKindChange={setCustomCommandKind}
              onBodyModeChange={setCustomEnvironmentBodyMode}
              onAddCommand={addCustomCommand}
              onAddEnvironment={addCustomEnvironment}
              onAddDelimiter={addCustomDelimiter}
              onRemoveRecord={removeCustomRecord}
              onResetAll={resetAllCustomKnowledgeRecords}
            />
          </div>
        </Panel>

        {isDesktop && (
          <Separator
            className="resize-handle w-1 transition-colors duration-150"
            style={{ background: 'var(--color-border-default)' }}
          />
        )}

        {/* Right panel: tree / serialized tabs */}
        <Panel id={RIGHT_PANEL_ID} defaultSize="62%" minSize="25%">
          <div
            className="flex h-full flex-col overflow-hidden"
            style={{
              background: 'var(--color-canvas-default)',
              ...(!isDesktop ? { borderTop: '2px solid var(--color-border-default)' } : {}),
            }}
          >
            {/* Tab bar */}
            <div
              className="flex h-9 shrink-0 items-center border-b"
              style={{ borderColor: 'var(--color-border-default)' }}
            >
              <TabButton
                label="Syntax Tree"
                active={rightTab === 'tree'}
                onClick={() => setRightTab('tree')}
              />
              <TabButton
                label="Serialized"
                active={rightTab === 'serialized'}
                onClick={() => setRightTab('serialized')}
              />

              {/* Context actions */}
              <div className="ml-auto flex items-center gap-1 pr-2">
                {rightTab === 'tree' && (
                  <>
                    <button
                      type="button"
                      className="btn btn-icon"
                      onClick={expandAll}
                      title="Expand all"
                    >
                      <ChevronsUpDown size={14} />
                    </button>
                    <button
                      type="button"
                      className="btn btn-icon"
                      onClick={collapseAll}
                      title="Collapse all"
                    >
                      <ChevronsDownUp size={14} />
                    </button>
                  </>
                )}
                {rightTab === 'serialized' && (
                  <button
                    type="button"
                    className="btn btn-icon"
                    onClick={() => setOptionsPanelOpen((p) => !p)}
                    title="Toggle options panel"
                    style={{
                      color: optionsPanelOpen
                        ? 'var(--color-accent-fg)'
                        : 'var(--color-fg-muted)',
                    }}
                  >
                    <Settings2 size={14} />
                  </button>
                )}
              </div>
            </div>

            {/* Tab content */}
            {rightTab === 'tree' ? (
              <SyntaxTreeTab
                treeRoot={treeRoot}
                collapsedNodes={collapsedNodes}
                onToggleNode={toggleNode}
                nodeCount={flatNodes.length}
                treeDepth={treeDepth}
                parseTime={parseState.parseTime}
              />
            ) : (
              <SerializedTab
                serializedOutput={serializedState.output}
                serializeError={serializedState.error}
                serializeTime={serializedState.serializeTime}
                serializeOptions={serializeOptions}
                onSerializeOptionsChange={setSerializeOptions}
                optionsPanelOpen={optionsPanelOpen}
                onToggleOptionsPanel={() => setOptionsPanelOpen((p) => !p)}
              />
            )}
          </div>
        </Panel>
      </Group>

      {/* Share toast */}
      {shareToast && (
        <div
          className="fixed bottom-4 left-1/2 z-50 -translate-x-1/2 rounded-md px-4 py-2 text-sm font-medium shadow-lg"
          style={{
            background: 'var(--color-success-subtle)',
            color: 'var(--color-success-fg)',
            border: '1px solid var(--color-border-default)',
          }}
        >
          Link copied to clipboard
        </div>
      )}
    </div>
  )
}

// -- Tab button helper --

function TabButton({
  label,
  active,
  onClick,
}: {
  label: string
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      className="relative px-3 py-1.5 text-xs font-medium transition-colors"
      style={{
        color: active ? 'var(--color-fg-default)' : 'var(--color-fg-muted)',
      }}
      onClick={onClick}
    >
      {label}
      {active && (
        <span
          className="absolute inset-x-0 bottom-0 h-[2px] rounded-full"
          style={{ background: 'var(--color-accent-fg)' }}
        />
      )}
    </button>
  )
}
