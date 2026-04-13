import { RotateCcw } from 'lucide-react'
import SectionHeader from './SectionHeader'
import type { ParseDiagnostic } from '../lib/texformWasm'

interface InputSectionProps {
  source: string
  strictMode: boolean
  collapsed: boolean
  fatalMessage: string | null
  diagnostics: ParseDiagnostic[]
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
  onSourceChange,
  onStrictModeChange,
  onResetSample,
  onToggleCollapsed,
}: InputSectionProps) {
  const hasDiagnostics = fatalMessage !== null || diagnostics.length > 0

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
        <div className="relative flex min-h-0 flex-1 flex-col">
          <textarea
            value={source}
            onChange={(e) => onSourceChange(e.target.value)}
            className="min-h-0 flex-1 resize-none border-0 p-3 pb-7 outline-none"
            style={{
              fontFamily: 'var(--font-mono)',
              fontSize: 14,
              lineHeight: 1.6,
              color: 'var(--color-fg-default)',
              background: 'var(--color-canvas-default)',
            }}
            placeholder="Enter LaTeX formula..."
            spellCheck={false}
          />
          <div
            className="pointer-events-none absolute bottom-1.5 right-3 flex items-center gap-3 text-[11px]"
            style={{ fontFamily: 'var(--font-mono)', color: 'var(--color-fg-subtle)' }}
          >
            <label
              className="pointer-events-auto flex cursor-pointer items-center gap-1"
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
            <span>{source.length} chars</span>
          </div>

          {hasDiagnostics && (
            <div
              className="shrink-0 border-t p-2.5 text-xs"
              style={{
                borderColor: 'var(--color-border-muted)',
                background: fatalMessage
                  ? 'var(--color-danger-subtle)'
                  : 'var(--color-attention-subtle)',
                color: fatalMessage
                  ? 'var(--color-danger-fg)'
                  : 'var(--color-attention-fg)',
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
