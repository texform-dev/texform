import { ChevronDown } from 'lucide-react'
import type { ReactNode } from 'react'

interface SectionHeaderProps {
  label: string
  collapsed: boolean
  onToggle: () => void
  badge?: ReactNode
  actions?: ReactNode
}

export default function SectionHeader({
  label,
  collapsed,
  onToggle,
  badge,
  actions,
}: SectionHeaderProps) {
  return (
    <div
      className="flex h-9 shrink-0 cursor-pointer select-none items-center gap-1.5 border-b px-3 text-[11px] font-semibold uppercase tracking-wide hover:text-[var(--color-fg-default)]"
      style={{
        color: 'var(--color-fg-muted)',
        background: 'var(--color-canvas-subtle)',
        borderColor: 'var(--color-border-muted)',
      }}
      onClick={onToggle}
    >
      <ChevronDown
        size={16}
        style={{
          color: 'var(--color-fg-subtle)',
          transition: 'transform 0.15s',
          transform: collapsed ? 'rotate(-90deg)' : undefined,
          flexShrink: 0,
        }}
      />
      {label}
      {badge}
      {actions && (
        <div
          className="ml-auto flex items-center gap-1"
          onClick={(e) => e.stopPropagation()}
        >
          {actions}
        </div>
      )}
    </div>
  )
}
