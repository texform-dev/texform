import { useState } from 'react'

interface SpecPopoverProps {
  specString: string
  specFromPackages?: string[]
  specDetail?: string
  explicitSpecString?: string
  explicitSpecFromPackages?: string[]
  explicitSpecDetail?: string
  characterUnicodeValue?: string
  characterPackage?: string
  characterMathvariant?: string
}

function formatPackageList(packages?: string[]): string {
  return packages && packages.length > 0 ? packages.join(', ') : 'unknown'
}

function sameStringList(left?: string[], right?: string[]): boolean {
  if (left === right) return true
  if (!left || !right || left.length !== right.length) return false
  return left.every((value, index) => value === right[index])
}

/** Render each arg line with colored tokens. */
function SpecArgsList({ detail }: { detail: string }) {
  const lines = detail.split('\n')
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      {lines.map((line, idx) => {
        // Parse format: "[0] required standard content(math)"
        const m = line.match(/^\[(\d+)\]\s+(required|optional)\s+(\S+)\s+(.*)/)
        if (!m) {
          return (
            <div
              key={idx}
              className="text-xs"
              style={{
                fontFamily: 'var(--font-mono)',
                color: 'var(--color-fg-muted)',
              }}
            >
              {line}
            </div>
          )
        }
        const [, index, req, form, kind] = m
        return (
          <div
            key={idx}
            className="flex items-baseline gap-1 text-xs"
            style={{ fontFamily: 'var(--font-mono)' }}
          >
            <span
              className="shrink-0 text-right"
              style={{ width: 56, color: 'var(--color-fg-muted)' }}
            >
              [{index}]
            </span>
            <span
              className="rounded px-1"
              style={{
                color:
                  req === 'required'
                    ? 'var(--color-attention-fg)'
                    : 'var(--color-fg-muted)',
                background:
                  req === 'required'
                    ? 'var(--color-attention-subtle)'
                    : 'var(--color-neutral-muted)',
              }}
            >
              {req}
            </span>
            <span
              className="rounded px-1"
              style={{
                color: 'var(--color-done-fg)',
                background: 'var(--color-done-subtle)',
              }}
            >
              {form}
            </span>
            <span
              className="rounded px-1"
              style={{
                color: 'var(--color-success-fg)',
                background: 'var(--color-success-subtle)',
              }}
            >
              {kind}
            </span>
          </div>
        )
      })}
    </div>
  )
}

export default function SpecPopover({
  specString,
  specFromPackages,
  specDetail,
  explicitSpecString,
  explicitSpecFromPackages,
  explicitSpecDetail,
  characterUnicodeValue,
  characterPackage,
  characterMathvariant,
}: SpecPopoverProps) {
  const [show, setShow] = useState(false)
  const activePackageLabel = formatPackageList(specFromPackages)
  const explicitPackageLabel = formatPackageList(explicitSpecFromPackages)
  const showExplicitSection =
    explicitSpecString !== undefined &&
    (explicitSpecString !== specString ||
      explicitSpecDetail !== specDetail ||
      !sameStringList(explicitSpecFromPackages, specFromPackages))

  const labelStyle: React.CSSProperties = {
    width: 56,
    flexShrink: 0,
    textAlign: 'right',
    fontSize: 11,
    fontWeight: 600,
    textTransform: 'uppercase',
    letterSpacing: '0.05em',
    color: 'var(--color-fg-muted)',
  }

  const sectionTitleStyle: React.CSSProperties = {
    fontSize: 11,
    fontWeight: 600,
    textTransform: 'uppercase',
    letterSpacing: '0.05em',
    color: 'var(--color-fg-muted)',
  }

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setShow(true)}
      onMouseLeave={() => setShow(false)}
    >
      <span
        className="cursor-help rounded-sm px-1 text-xs leading-none"
        style={{
          color: 'var(--color-success-fg)',
          background: 'var(--color-success-subtle)',
          border: '1px solid currentColor',
          borderColor: 'color-mix(in srgb, var(--color-success-fg), transparent 70%)',
        }}
      >
        spec
      </span>
      {show ? (
        <div
          className="absolute left-0 top-full z-50 mt-1 w-max max-w-sm rounded-md p-2.5"
          style={{
            background: 'var(--color-canvas-default)',
            border: '1px solid var(--color-border-default)',
            boxShadow: 'var(--shadow-md)',
          }}
        >
          <div className="space-y-2 text-xs">
            {/* Active section */}
            <div className="space-y-1.5">
              <div style={sectionTitleStyle}>active</div>
              <div className="flex items-baseline gap-2">
                <span style={labelStyle}>spec</span>
                <code
                  className="rounded px-1.5 font-semibold"
                  style={{
                    fontFamily: 'var(--font-mono)',
                    color: 'var(--color-done-fg)',
                    background: 'var(--color-done-subtle)',
                  }}
                >
                  {specString || '(empty)'}
                </code>
              </div>
              <div className="flex items-baseline gap-2">
                <span style={labelStyle}>packages</span>
                <span
                  className="rounded px-1.5"
                  style={{
                    color: 'var(--color-accent-fg)',
                    background: 'var(--color-accent-subtle)',
                  }}
                >
                  {activePackageLabel}
                </span>
              </div>
              {specDetail ? <SpecArgsList detail={specDetail} /> : null}
            </div>

            {/* Explicit section — only when different from active */}
            {showExplicitSection ? (
              <div
                className="space-y-1.5 pt-2"
                style={{ borderTop: '1px solid var(--color-border-muted)' }}
              >
                <div style={sectionTitleStyle}>explicit</div>
                <div className="flex items-baseline gap-2">
                  <span style={labelStyle}>spec</span>
                  <code
                    className="rounded px-1.5 font-semibold"
                    style={{
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--color-attention-fg)',
                      background: 'var(--color-attention-subtle)',
                    }}
                  >
                    {explicitSpecString || '(empty)'}
                  </code>
                </div>
                <div className="flex items-baseline gap-2">
                  <span style={labelStyle}>packages</span>
                  <span
                    className="rounded px-1.5"
                    style={{
                      color: 'var(--color-attention-fg)',
                      background: 'var(--color-attention-subtle)',
                    }}
                  >
                    {explicitPackageLabel}
                  </span>
                </div>
                {explicitSpecDetail ? <SpecArgsList detail={explicitSpecDetail} /> : null}
              </div>
            ) : null}

            {/* Character section */}
            {characterUnicodeValue ? (
              <div
                className="space-y-1.5 pt-2"
                style={{ borderTop: '1px solid var(--color-border-muted)' }}
              >
                <div style={sectionTitleStyle}>character</div>
                <div className="flex items-baseline gap-2">
                  <span style={labelStyle}>unicode</span>
                  <code
                    className="rounded px-1.5 font-semibold"
                    style={{
                      fontFamily: 'var(--font-mono)',
                      color: 'var(--color-success-fg)',
                      background: 'var(--color-success-subtle)',
                    }}
                  >
                    {characterUnicodeValue}
                  </code>
                </div>
                <div className="flex items-baseline gap-2">
                  <span style={labelStyle}>package</span>
                  <span
                    className="rounded px-1.5"
                    style={{
                      color: 'var(--color-success-fg)',
                      background: 'var(--color-success-subtle)',
                    }}
                  >
                    {characterPackage ?? 'unknown'}
                  </span>
                </div>
                {characterMathvariant ? (
                  <div className="flex items-baseline gap-2">
                    <span style={labelStyle}>variant</span>
                    <code
                      className="rounded px-1.5 font-semibold"
                      style={{
                        fontFamily: 'var(--font-mono)',
                        color: 'var(--color-success-fg)',
                        background: 'var(--color-success-subtle)',
                      }}
                    >
                      {characterMathvariant}
                    </code>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </span>
  )
}
