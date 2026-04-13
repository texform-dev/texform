import OptionsPanel from './OptionsPanel'
import type { SerializeOptions } from '../schema/serializeOptions'

interface SerializedTabProps {
  serializedOutput: string | null
  serializeError: string | null
  serializeOptions: SerializeOptions
  onSerializeOptionsChange: (options: SerializeOptions) => void
  optionsPanelOpen: boolean
  onToggleOptionsPanel: () => void
}

export default function SerializedTab({
  serializedOutput,
  serializeError,
  serializeOptions,
  onSerializeOptionsChange,
  optionsPanelOpen,
  onToggleOptionsPanel,
}: SerializedTabProps) {
  return (
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
  )
}
