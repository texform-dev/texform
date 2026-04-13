import { RotateCcw } from 'lucide-react'
import type { AllowedMode, BodyMode, CommandKind } from '../lib/texformWasm'
import type { CustomKnowledgeRecordEntry, CustomKnowledgeRecordTarget } from '../lib/types'
import KnowledgeForm from './KnowledgeForm'
import KnowledgeRecord from './KnowledgeRecord'
import SectionHeader from './SectionHeader'

interface KnowledgeSectionProps {
  collapsed: boolean
  onToggleCollapsed: () => void
  customKnowledgeRecords: CustomKnowledgeRecordEntry[]
  activeForm: CustomKnowledgeRecordTarget | null
  name: string
  spec: string
  mode: AllowedMode
  commandKind: CommandKind
  bodyMode: BodyMode
  error: string | null
  onToggleForm: (target: CustomKnowledgeRecordTarget) => void
  onNameChange: (name: string) => void
  onSpecChange: (spec: string) => void
  onModeChange: (mode: AllowedMode) => void
  onCommandKindChange: (kind: CommandKind) => void
  onBodyModeChange: (mode: BodyMode) => void
  onAddCommand: () => void
  onAddEnvironment: () => void
  onAddDelimiter: () => void
  onRemoveRecord: (record: CustomKnowledgeRecordEntry) => void
  onResetAll: () => void
}

const actionButtonBase =
  'rounded-sm px-1.5 py-0.5 text-[10px] font-medium leading-tight transition-colors'

export default function KnowledgeSection({
  collapsed,
  onToggleCollapsed,
  customKnowledgeRecords,
  activeForm,
  name,
  spec,
  mode,
  commandKind,
  bodyMode,
  error,
  onToggleForm,
  onNameChange,
  onSpecChange,
  onModeChange,
  onCommandKindChange,
  onBodyModeChange,
  onAddCommand,
  onAddEnvironment,
  onAddDelimiter,
  onRemoveRecord,
  onResetAll,
}: KnowledgeSectionProps) {
  const recordCount = customKnowledgeRecords.length

  const handleSubmit = () => {
    if (activeForm === 'environment') {
      onAddEnvironment()
    } else if (activeForm === 'delimiter') {
      onAddDelimiter()
    } else {
      onAddCommand()
    }
  }

  return (
    <div className="flex min-h-0 flex-col">
      <SectionHeader
        label="KNOWLEDGE RECORDS"
        collapsed={collapsed}
        onToggle={onToggleCollapsed}
        badge={
          recordCount > 0 ? (
            <span
              className="ml-1 text-[10px] font-normal"
              style={{ color: 'var(--color-fg-subtle)' }}
            >
              {recordCount}
            </span>
          ) : undefined
        }
        actions={
          <>
            <button
              type="button"
              className={actionButtonBase}
              style={{ color: 'var(--color-accent-fg)' }}
              onClick={() => onToggleForm('command')}
            >
              {activeForm === 'command' ? '- Cmd' : '+Cmd'}
            </button>
            <button
              type="button"
              className={actionButtonBase}
              style={{ color: 'var(--color-success-fg)' }}
              onClick={() => onToggleForm('environment')}
            >
              {activeForm === 'environment' ? '- Env' : '+Env'}
            </button>
            <button
              type="button"
              className={actionButtonBase}
              style={{ color: 'var(--color-attention-fg)' }}
              onClick={() => onToggleForm('delimiter')}
            >
              {activeForm === 'delimiter' ? '- Delim' : '+Delim'}
            </button>
          </>
        }
      />

      {!collapsed && (
        <div className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-hidden px-3 py-2">
          {activeForm !== null && (
            <KnowledgeForm
              activeForm={activeForm}
              name={name}
              spec={spec}
              mode={mode}
              commandKind={commandKind}
              bodyMode={bodyMode}
              error={error}
              onNameChange={onNameChange}
              onSpecChange={onSpecChange}
              onModeChange={onModeChange}
              onCommandKindChange={onCommandKindChange}
              onBodyModeChange={onBodyModeChange}
              onSubmit={handleSubmit}
            />
          )}

          {recordCount > 0 ? (
            <div className="min-h-0 flex-1 space-y-1 overflow-y-auto pr-1">
              {customKnowledgeRecords.map((record) => (
                <KnowledgeRecord
                  key={`${record.target}:${record.name}`}
                  record={record}
                  onRemove={() => onRemoveRecord(record)}
                />
              ))}
            </div>
          ) : activeForm === null ? (
            <div
              className="flex items-center justify-center rounded-sm px-3 py-4 text-center text-xs italic"
              style={{
                border: '1px dashed var(--color-border-muted)',
                color: 'var(--color-fg-subtle)',
              }}
            >
              No custom knowledge records.
            </div>
          ) : null}

          {recordCount > 0 && (
            <button
              type="button"
              className="btn-subtle inline-flex shrink-0 items-center gap-1 self-start rounded-sm px-2 py-1 text-[10px] transition-colors"
              style={{
                color: 'var(--color-fg-subtle)',
                border: '1px solid var(--color-border-muted)',
                background: 'transparent',
              }}
              onClick={onResetAll}
              title="Remove all custom knowledge records"
            >
              <RotateCcw size={10} />
              Reset all
            </button>
          )}
        </div>
      )}
    </div>
  )
}
