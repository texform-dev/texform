import type { AllowedMode, BodyMode, CommandKind } from '../lib/texformWasm'

interface KnowledgeFormProps {
  activeForm: 'command' | 'environment' | 'delimiter'
  name: string
  spec: string
  mode: AllowedMode
  commandKind: CommandKind
  bodyMode: BodyMode
  error: string | null
  onNameChange: (name: string) => void
  onSpecChange: (spec: string) => void
  onModeChange: (mode: AllowedMode) => void
  onCommandKindChange: (kind: CommandKind) => void
  onBodyModeChange: (mode: BodyMode) => void
  onSubmit: () => void
}

const inputStyle = {
  background: 'var(--color-canvas-default)',
  border: '1px solid var(--color-border-default)',
  color: 'var(--color-fg-default)',
}

const selectStyle = {
  ...inputStyle,
}

export default function KnowledgeForm({
  activeForm,
  name,
  spec,
  mode,
  commandKind,
  bodyMode,
  error,
  onNameChange,
  onSpecChange,
  onModeChange,
  onCommandKindChange,
  onBodyModeChange,
  onSubmit,
}: KnowledgeFormProps) {
  const isCommandForm = activeForm === 'command'
  const isEnvironmentForm = activeForm === 'environment'
  const isDelimiterForm = activeForm === 'delimiter'

  return (
    <form
      className="rounded p-2"
      style={{
        background: 'var(--color-canvas-subtle)',
        border: '1px solid var(--color-border-muted)',
      }}
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit()
      }}
    >
      <div className="grid grid-cols-[1fr_1fr] gap-x-2 gap-y-1">
        <label
          className="text-xs font-medium uppercase tracking-wide"
          style={{ color: 'var(--color-fg-muted)' }}
        >
          Name
          <input
            value={name}
            onChange={(event) => onNameChange(event.target.value)}
            className="mt-1 block w-full rounded-sm px-2 py-1 text-xs focus:outline-none"
            style={inputStyle}
            placeholder={
              isEnvironmentForm
                ? 'e.g. proofbox'
                : isDelimiterForm
                  ? 'e.g. langle'
                  : 'e.g. dv'
            }
            autoFocus
          />
        </label>
        {!isDelimiterForm && (
          <label
            className="text-xs font-medium uppercase tracking-wide"
            style={{ color: 'var(--color-fg-muted)' }}
          >
            Spec
            <input
              value={spec}
              onChange={(event) => onSpecChange(event.target.value)}
              className="mt-1 block w-full rounded-sm px-2 py-1 text-xs focus:outline-none"
              style={{ ...inputStyle, fontFamily: 'var(--font-mono)' }}
              placeholder="e.g. s o m"
            />
          </label>
        )}
        {!isDelimiterForm && (
          <label
            className="text-xs font-medium uppercase tracking-wide"
            style={{ color: 'var(--color-fg-muted)' }}
          >
            Allowed Mode
            <select
              value={mode}
              onChange={(event) => onModeChange(event.target.value as AllowedMode)}
              className="mt-1 block w-full rounded-sm px-2 py-1 text-xs"
              style={selectStyle}
            >
              <option value="math">math</option>
              <option value="text">text</option>
              <option value="both">both</option>
            </select>
          </label>
        )}
        {isCommandForm && (
          <label
            className="text-xs font-medium uppercase tracking-wide"
            style={{ color: 'var(--color-fg-muted)' }}
          >
            Kind
            <select
              value={commandKind}
              onChange={(event) => onCommandKindChange(event.target.value as CommandKind)}
              className="mt-1 block w-full rounded-sm px-2 py-1 text-xs"
              style={selectStyle}
            >
              <option value="prefix">prefix</option>
              <option value="infix">infix</option>
              <option value="declarative">declarative</option>
            </select>
          </label>
        )}
        {isEnvironmentForm && (
          <label
            className="text-xs font-medium uppercase tracking-wide"
            style={{ color: 'var(--color-fg-muted)' }}
          >
            Body Mode
            <select
              value={bodyMode}
              onChange={(event) => onBodyModeChange(event.target.value as BodyMode)}
              className="mt-1 block w-full rounded-sm px-2 py-1 text-xs"
              style={selectStyle}
            >
              <option value="math">math</option>
              <option value="text">text</option>
            </select>
          </label>
        )}
      </div>

      <div className="mt-2 flex items-center gap-2">
        <button
          type="submit"
          className="btn-submit rounded-sm px-2.5 py-1 text-xs font-medium leading-tight transition-colors"
          style={{
            background: 'var(--color-accent-subtle)',
            color: 'var(--color-accent-fg)',
            border: '1px solid var(--color-accent-muted)',
            cursor: 'pointer',
          }}
        >
          {isEnvironmentForm
            ? 'Add Environment'
            : isDelimiterForm
              ? 'Add Delimiter'
              : 'Add Command'}
        </button>
        {error && (
          <span className="text-xs" style={{ color: 'var(--color-danger-fg)' }}>
            {error}
          </span>
        )}
      </div>
    </form>
  )
}
