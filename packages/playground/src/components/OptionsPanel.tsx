import { X } from 'lucide-react'
import ToggleGroup from './ToggleGroup'
import {
  SERIALIZE_OPTION_DESCRIPTORS,
  getOptionValue,
  setOptionValue,
  type SerializeOptions,
} from '../schema/serializeOptions'

interface OptionsPanelProps {
  options: SerializeOptions
  onChange: (options: SerializeOptions) => void
  onClose: () => void
}

export default function OptionsPanel({ options, onChange, onClose }: OptionsPanelProps) {
  return (
    <div
      className="flex w-[280px] shrink-0 flex-col overflow-hidden border-l"
      style={{
        background: 'var(--color-canvas-subtle)',
        borderColor: 'var(--color-border-default)',
      }}
    >
      <div
        className="flex h-9 shrink-0 items-center gap-1.5 border-b px-3 text-[11px] font-semibold uppercase tracking-wide"
        style={{ color: 'var(--color-fg-muted)', borderColor: 'var(--color-border-muted)' }}
      >
        <span className="flex-1">Serialize Options</span>
        <button type="button" className="btn btn-sm btn-icon" onClick={onClose}>
          <X size={12} />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {SERIALIZE_OPTION_DESCRIPTORS.map((desc) => (
          <div
            key={desc.path}
            className="border-b px-3 py-2.5"
            style={{ borderColor: 'var(--color-border-muted)' }}
          >
            <div className="text-xs font-semibold" style={{ color: 'var(--color-fg-default)' }}>
              {desc.title}
            </div>
            <div
              className="text-[10px]"
              style={{ fontFamily: 'var(--font-mono)', color: 'var(--color-fg-subtle)' }}
            >
              {desc.path}
            </div>
            <div
              className="mt-1 text-[11px] leading-snug"
              style={{ color: 'var(--color-fg-muted)' }}
            >
              {desc.description}
            </div>
            <div className="mt-2">
              <ToggleGroup
                values={desc.values}
                selected={desc.disabled ? desc.defaultValue : getOptionValue(options, desc.path)}
                onChange={(value) => onChange(setOptionValue(options, desc.path, value))}
                disabled={desc.disabled}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
