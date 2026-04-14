import { useCallback, useEffect, useRef } from 'react'
import { RotateCcw } from 'lucide-react'
import Editor, { type BeforeMount, type OnMount } from '@monaco-editor/react'
import type * as Monaco from 'monaco-editor'
import SectionHeader from './SectionHeader'
import type { ParseDiagnostic } from '../lib/texformWasm'
import type { Theme } from '../lib/theme'
import { LATEX_LANGUAGE_ID, registerLatexLanguage } from '../lib/latexLanguage'

interface InputSectionProps {
  source: string
  strictMode: boolean
  collapsed: boolean
  fatalMessage: string | null
  diagnostics: ParseDiagnostic[]
  theme: Theme
  onSourceChange: (source: string) => void
  onStrictModeChange: (checked: boolean) => void
  onResetSample: () => void
  onToggleCollapsed: () => void
}

export default function InputSection({
  source,
  strictMode,
  collapsed,
  fatalMessage,
  diagnostics,
  theme,
  onSourceChange,
  onStrictModeChange,
  onResetSample,
  onToggleCollapsed,
}: InputSectionProps) {
  const editorRef = useRef<Monaco.editor.IStandaloneCodeEditor | null>(null)
  const monacoRef = useRef<typeof Monaco | null>(null)

  const hasDiagnostics = fatalMessage !== null || diagnostics.length > 0
  const monacoTheme = theme === 'dark' ? 'texform-dark' : 'texform-light'

  // Register language and themes before editor mounts
  const handleBeforeMount = useCallback<BeforeMount>((monaco) => {
    registerLatexLanguage(monaco)
  }, [])

  // Store editor and monaco refs on mount
  const handleEditorMount = useCallback<OnMount>((editor, monaco) => {
    editorRef.current = editor
    monacoRef.current = monaco
  }, [])

  // Update diagnostic markers whenever diagnostics change
  useEffect(() => {
    const editor = editorRef.current
    const monaco = monacoRef.current
    if (!editor || !monaco) return

    const model = editor.getModel()
    if (!model) return

    if (diagnostics.length === 0) {
      monaco.editor.setModelMarkers(model, 'texform', [])
      return
    }

    const markers: Monaco.editor.IMarkerData[] = diagnostics.map((d) => {
      const start = model.getPositionAt(d.span.start)
      const end = model.getPositionAt(d.span.end)
      // Ensure the squiggly range is at least 1 column wide so it's visible
      const endColumn = start.lineNumber === end.lineNumber && start.column === end.column
        ? end.column + 1
        : end.column
      return {
        severity: monaco.MarkerSeverity.Error,
        message: d.message,
        startLineNumber: start.lineNumber,
        startColumn: start.column,
        endLineNumber: end.lineNumber,
        endColumn,
      }
    })

    monaco.editor.setModelMarkers(model, 'texform', markers)
  }, [diagnostics])

  // Clear markers when component unmounts
  useEffect(() => {
    return () => {
      const monaco = monacoRef.current
      const editor = editorRef.current
      if (!monaco || !editor) return
      const model = editor.getModel()
      if (model) monaco.editor.setModelMarkers(model, 'texform', [])
    }
  }, [])

  return (
    <>
      <SectionHeader
        label="INPUT"
        collapsed={collapsed}
        onToggle={onToggleCollapsed}
        actions={
          <button type="button" className="btn btn-sm btn-icon" onClick={onResetSample} title="Reset input">
            <RotateCcw size={12} />
          </button>
        }
      />
      {!collapsed && (
        <div className="flex min-h-0 flex-1 flex-col">
          {/* Monaco Editor fills available height */}
          <div style={{ flex: '1 1 0', minHeight: 0, position: 'relative', overflow: 'hidden' }}>
            <Editor
              height="100%"
              defaultLanguage={LATEX_LANGUAGE_ID}
              theme={monacoTheme}
              value={source}
              onChange={(value) => onSourceChange(value ?? '')}
              beforeMount={handleBeforeMount}
              onMount={handleEditorMount}
              options={{
                fontSize: 14,
                fontFamily: "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'Liberation Mono', monospace",
                lineNumbers: 'on',
                minimap: { enabled: false },
                scrollBeyondLastLine: false,
                wordWrap: 'on',
                overviewRulerBorder: false,
                overviewRulerLanes: 0,
                padding: { top: 10, bottom: 10 },
                renderLineHighlight: 'line',
                glyphMargin: false,
                folding: false,
                quickSuggestions: false,
                suggestOnTriggerCharacters: false,
                // Render hover/suggest widgets with position:fixed so they
                // escape overflow:hidden ancestors and aren't clipped by the
                // Header or other panels.
                fixedOverflowWidgets: true,
                scrollbar: {
                  horizontal: 'hidden',
                  verticalScrollbarSize: 6,
                },
              }}
            />
          </div>

          <div
            className="flex shrink-0 items-center justify-between border-t px-3 py-1 text-[11px]"
            style={{
              background: 'var(--color-canvas-subtle)',
              borderColor: 'var(--color-border-muted)',
              color: 'var(--color-fg-subtle)',
            }}
          >
            <label
              className="flex cursor-pointer items-center gap-1"
              style={{ fontFamily: 'var(--font-sans)' }}
            >
              <input
                type="checkbox"
                checked={strictMode}
                onChange={(e) => onStrictModeChange(e.target.checked)}
                style={{ accentColor: 'var(--color-accent-fg)' }}
              />
              strict
            </label>
            <span style={{ fontFamily: 'var(--font-mono)' }}>{source.length} chars</span>
          </div>

          {hasDiagnostics && (
            <div
              className="shrink-0 border-t p-2.5 text-xs"
              style={{
                borderColor: 'var(--color-border-muted)',
                background: 'var(--color-danger-subtle)',
                color: 'var(--color-danger-fg)',
              }}
            >
              <div className="font-semibold">
                {fatalMessage
                  ? diagnostics.length > 0
                    ? `Parse Error (${diagnostics.length} diagnostics)`
                    : 'Parse Error'
                  : `Diagnostics (${diagnostics.length})`}
              </div>
              {fatalMessage && (
                <pre
                  className="m-0 mt-1 whitespace-pre-wrap break-words"
                  style={{ fontFamily: 'var(--font-mono)' }}
                >
                  {fatalMessage}
                </pre>
              )}
              {diagnostics.length > 0 && (
                <ul className="mt-1.5 list-disc pl-4">
                  {diagnostics.map((d, i) => (
                    <li key={`${d.message}-${i}`} className="my-1">
                      {d.message}{' '}
                      <span style={{ fontFamily: 'var(--font-mono)', opacity: 0.8 }}>
                        span {d.span.start}..{d.span.end}
                      </span>
                      {d.contexts && d.contexts.length > 0 && (
                        <ul className="mt-0.5 list-none pl-3" style={{ opacity: 0.75 }}>
                          {d.contexts.map((ctx, j) => (
                            <li key={j} style={{ fontFamily: 'var(--font-mono)' }}>
                              in {ctx.label}{' '}
                              <span style={{ opacity: 0.8 }}>
                                span {ctx.span.start}..{ctx.span.end}
                              </span>
                            </li>
                          ))}
                        </ul>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          )}
        </div>
      )}
    </>
  )
}
