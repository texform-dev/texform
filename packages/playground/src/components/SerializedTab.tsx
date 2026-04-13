import { useCallback, useRef, useState } from 'react'
import { Copy, Check } from 'lucide-react'
import OptionsPanel from './OptionsPanel'
import type { SerializeOptions } from '../schema/serializeOptions'

interface SerializedTabProps {
  serializedOutput: string | null
  serializeError: string | null
  serializeTime: number | null
  serializeOptions: SerializeOptions
  onSerializeOptionsChange: (options: SerializeOptions) => void
  optionsPanelOpen: boolean
  onToggleOptionsPanel: () => void
}

export default function SerializedTab({
  serializedOutput,
  serializeError,
  serializeTime,
  serializeOptions,
  onSerializeOptionsChange,
  optionsPanelOpen,
  onToggleOptionsPanel,
}: SerializedTabProps) {
  const [copied, setCopied] = useState(false)
  const copyTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const handleCopy = useCallback(async () => {
    if (!serializedOutput) return
    try {
      await navigator.clipboard.writeText(serializedOutput)
      setCopied(true)
      if (copyTimer.current) clearTimeout(copyTimer.current)
      copyTimer.current = setTimeout(() => setCopied(false), 2000)
    } catch {
      // ignore
    }
  }, [serializedOutput])

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div className="flex flex-1 overflow-hidden">
        <div className="flex flex-1 flex-col overflow-hidden">
          {serializeError ? (
            <div
              className="m-3 rounded p-2.5 text-xs"
              style={{
                background: 'var(--color-danger-subtle)',
                color: 'var(--color-danger-fg)',
                border: '1px solid var(--color-border-default)',
              }}
            >
              <div className="font-semibold">Serialize Error</div>
              <pre
                className="m-0 mt-1 whitespace-pre-wrap break-words"
                style={{ fontFamily: 'var(--font-mono)' }}
              >
                {serializeError}
              </pre>
            </div>
          ) : (
            <div
              className="flex-1 overflow-auto whitespace-pre-wrap break-all p-3"
              style={{
                fontFamily: 'var(--font-mono)',
                fontSize: 14,
                lineHeight: 1.6,
                color: 'var(--color-fg-default)',
              }}
            >
              {serializedOutput ?? ''}
            </div>
          )}
        </div>

        {optionsPanelOpen && (
          <OptionsPanel
            options={serializeOptions}
            onChange={onSerializeOptionsChange}
            onClose={onToggleOptionsPanel}
          />
        )}
      </div>

      <div
        className="flex shrink-0 items-center border-t px-3 py-1 text-[11px]"
        style={{
          background: 'var(--color-canvas-subtle)',
          borderColor: 'var(--color-border-muted)',
          color: 'var(--color-fg-subtle)',
        }}
      >
        <button
          type="button"
          className="btn btn-sm btn-icon flex items-center gap-1"
          onClick={handleCopy}
          disabled={!serializedOutput}
          title="Copy to clipboard"
          style={{ fontFamily: 'var(--font-sans)', fontSize: 11 }}
        >
          {copied ? <Check size={11} /> : <Copy size={11} />}
          {copied ? 'Copied' : 'Copy'}
        </button>
        <div
          className="ml-auto flex gap-3"
          style={{ fontFamily: 'var(--font-mono)' }}
        >
          {serializedOutput !== null && <span>{serializedOutput.length} chars</span>}
          {serializeTime !== null && <span>{serializeTime.toFixed(1)} ms</span>}
        </div>
      </div>
    </div>
  )
}
