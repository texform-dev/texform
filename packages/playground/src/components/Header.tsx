import { Moon, Sun, Share2 } from 'lucide-react'
import type { Theme } from '../lib/theme'

interface HeaderProps {
  theme: Theme
  onToggleTheme: () => void
  statusText: string
  statusVariant: 'loading' | 'ok' | 'warn' | 'error'
  onShare: () => void
}

const STATUS_STYLES: Record<string, { color: string; bg: string }> = {
  loading: {
    color: 'var(--color-accent-fg)',
    bg: 'var(--color-accent-subtle)',
  },
  ok: {
    color: 'var(--color-success-fg)',
    bg: 'var(--color-success-subtle)',
  },
  warn: {
    color: 'var(--color-attention-fg)',
    bg: 'var(--color-attention-subtle)',
  },
  error: {
    color: 'var(--color-danger-fg)',
    bg: 'var(--color-danger-subtle)',
  },
}

export default function Header({
  theme,
  onToggleTheme,
  statusText,
  statusVariant,
  onShare,
}: HeaderProps) {
  const status = STATUS_STYLES[statusVariant]

  return (
    <header
      className="flex h-12 shrink-0 items-center justify-between border-b px-4"
      style={{
        background: 'var(--color-header-bg)',
        borderColor: 'var(--color-border-default)',
      }}
    >
      <div className="flex items-center gap-3">
        <div className="text-[15px] font-semibold" style={{ letterSpacing: '-0.01em' }}>
          TeXForm{' '}
          <span style={{ color: 'var(--color-fg-muted)', fontWeight: 400 }}>
            Playground
          </span>
        </div>
        <span
          className="inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium"
          style={{ color: status.color, background: status.bg }}
        >
          <svg width="8" height="8" viewBox="0 0 8 8" fill="currentColor">
            <circle cx="4" cy="4" r="4" />
          </svg>
          {statusText}
        </span>
      </div>

      <div className="flex items-center gap-2">
        <button type="button" className="btn btn-sm btn-primary" onClick={onShare}>
          <Share2 size={12} />
          Share
        </button>
        <button
          type="button"
          className="btn btn-icon"
          onClick={onToggleTheme}
          title="Toggle theme"
        >
          {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        </button>
      </div>
    </header>
  )
}
