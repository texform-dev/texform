interface ToggleGroupProps {
  values: readonly string[]
  selected: string
  onChange: (value: string) => void
  disabled?: boolean
}

export default function ToggleGroup({
  values,
  selected,
  onChange,
  disabled,
}: ToggleGroupProps) {
  return (
    <div
      className="inline-flex overflow-hidden rounded-md"
      style={{ border: '1px solid var(--color-btn-border)', opacity: disabled ? 0.5 : 1 }}
    >
      {values.map((value, i) => (
        <button
          key={value}
          type="button"
          disabled={disabled}
          className="border-0 px-2.5 py-1 text-[11px] transition-all"
          style={{
            fontFamily: 'var(--font-mono)',
            cursor: disabled ? 'not-allowed' : 'pointer',
            borderRight:
              i < values.length - 1
                ? '1px solid var(--color-btn-border)'
                : undefined,
            ...(value === selected
              ? {
                  background: 'var(--color-accent-emphasis)',
                  color: '#fff',
                  fontWeight: 500,
                }
              : {
                  background: 'var(--color-canvas-default)',
                  color: 'var(--color-fg-muted)',
                }),
          }}
          onClick={() => !disabled && onChange(value)}
        >
          {value}
        </button>
      ))}
    </div>
  )
}
