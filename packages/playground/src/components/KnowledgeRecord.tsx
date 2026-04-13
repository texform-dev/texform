import { X } from 'lucide-react'
import type { CustomKnowledgeRecordEntry } from '../lib/types'

interface KnowledgeRecordProps {
  record: CustomKnowledgeRecordEntry
  onRemove: () => void
}

export default function KnowledgeRecord({ record, onRemove }: KnowledgeRecordProps) {
  return (
    <div
      className="flex items-center gap-1.5 rounded px-2 py-1 text-xs"
      style={{
        background: 'var(--color-canvas-subtle)',
        border: '1px solid var(--color-border-muted)',
      }}
    >
      <span className="shrink-0 font-semibold" style={{ fontFamily: 'var(--font-mono)' }}>
        {record.target === 'environment'
          ? `\\begin{${record.name}}`
          : `\\${record.name}`}
      </span>

      <span
        className="rounded-sm px-1 py-px text-[10px] font-medium leading-none"
        style={{ background: 'var(--color-neutral-muted)', color: 'var(--color-fg-muted)' }}
      >
        {record.target}
      </span>

      {record.target === 'command' && (
        <span
          className="rounded-sm px-1 py-px text-[10px] font-medium leading-none"
          style={{ background: 'var(--color-accent-subtle)', color: 'var(--color-accent-fg)' }}
        >
          {record.kind}
        </span>
      )}
      {record.target === 'environment' && (
        <span
          className="rounded-sm px-1 py-px text-[10px] font-medium leading-none"
          style={{ background: 'var(--color-success-subtle)', color: 'var(--color-success-fg)' }}
        >
          body {record.bodyMode}
        </span>
      )}
      {record.target === 'delimiter' && (
        <span
          className="rounded-sm px-1 py-px text-[10px] font-medium leading-none"
          style={{ background: 'var(--color-attention-subtle)', color: 'var(--color-attention-fg)' }}
        >
          control
        </span>
      )}

      {'mode' in record && (
        <span
          className="rounded-sm px-1 py-px text-[10px] font-medium leading-none"
          style={{ background: 'var(--color-success-subtle)', color: 'var(--color-success-fg)' }}
        >
          {record.mode}
        </span>
      )}

      <span
        className="min-w-0 flex-1 truncate text-xs"
        style={{ fontFamily: 'var(--font-mono)', color: 'var(--color-fg-subtle)' }}
      >
        {'argspec' in record ? record.argspec || '(no spec)' : 'delimiter control'}
      </span>

      <button
        type="button"
        className="btn-subtle ml-auto shrink-0 rounded-sm px-1 py-px text-xs leading-tight transition-colors"
        style={{ color: 'var(--color-fg-subtle)', background: 'transparent', border: 'none' }}
        onClick={onRemove}
        title="Remove record"
      >
        <X size={12} />
      </button>
    </div>
  )
}
