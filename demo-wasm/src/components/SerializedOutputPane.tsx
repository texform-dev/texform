import type {
  AdjacentCharSpacing,
  CommandSpacing,
  EnvironmentNameSpacing,
  MathGroupInnerSpacing,
  ScriptOrder,
  ScriptSpacing,
  SerializeOptions,
} from '../texformWasm'

interface SerializedOutputPaneProps {
  serializedOutput: string | null
  serializeError: string | null
  serializeOptions: SerializeOptions
  onSerializeOptionsChange: (options: SerializeOptions) => void
}

const selectClass =
  'rounded-sm border border-slate-300 bg-white px-1.5 py-0.5 text-xs'

function SerializedOutputPane({
  serializedOutput,
  serializeError,
  serializeOptions,
  onSerializeOptionsChange,
}: SerializedOutputPaneProps) {
  const commandSpacing = serializeOptions.math?.spacing?.commands ?? 'spaced'
  const groupInner = serializeOptions.math?.spacing?.group_inner_spacing ?? 'padded'
  const adjacentChars = serializeOptions.math?.spacing?.adjacent_chars ?? 'spaced'
  const scriptSpacing = serializeOptions.math?.scripts?.spacing ?? 'spaced'
  const scriptOrder = serializeOptions.math?.scripts?.order ?? 'sub_first'
  const envNameSpacing = serializeOptions.syntax?.environments?.name_spacing ?? 'spaced'

  function update(patch: SerializeOptions) {
    const merged: SerializeOptions = {
      math: {
        spacing: {
          ...serializeOptions.math?.spacing,
          ...patch.math?.spacing,
        },
        scripts: {
          ...serializeOptions.math?.scripts,
          ...patch.math?.scripts,
        },
      },
      syntax: {
        environments: {
          ...serializeOptions.syntax?.environments,
          ...patch.syntax?.environments,
        },
      },
    }
    onSerializeOptionsChange(merged)
  }

  return (
    <>
      {/* Options */}
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-slate-200 px-0.5 py-1.5 text-xs text-slate-600 [font-family:var(--font-code)]">
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.spacing.commands</span>
          <select
            className={selectClass}
            value={commandSpacing}
            onChange={(e) =>
              update({ math: { spacing: { commands: e.target.value as CommandSpacing } } })
            }
          >
            <option value="spaced">spaced</option>
            <option value="minimal">minimal</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.spacing.group_inner_spacing</span>
          <select
            className={selectClass}
            value={groupInner}
            onChange={(e) =>
              update({
                math: {
                  spacing: { group_inner_spacing: e.target.value as MathGroupInnerSpacing },
                },
              })
            }
          >
            <option value="padded">padded</option>
            <option value="compact">compact</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.spacing.adjacent_chars</span>
          <select
            className={selectClass}
            value={adjacentChars}
            onChange={(e) =>
              update({
                math: { spacing: { adjacent_chars: e.target.value as AdjacentCharSpacing } },
              })
            }
          >
            <option value="spaced">spaced</option>
            <option value="compact">compact</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.scripts.grouping</span>
          <select className={`${selectClass} opacity-50`} value="always_explicit" disabled>
            <option value="always_explicit">always_explicit</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.scripts.spacing</span>
          <select
            className={selectClass}
            value={scriptSpacing}
            onChange={(e) =>
              update({ math: { scripts: { spacing: e.target.value as ScriptSpacing } } })
            }
          >
            <option value="spaced">spaced</option>
            <option value="compact">compact</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">math.scripts.order</span>
          <select
            className={selectClass}
            value={scriptOrder}
            onChange={(e) =>
              update({ math: { scripts: { order: e.target.value as ScriptOrder } } })
            }
          >
            <option value="sub_first">sub_first</option>
            <option value="sup_first">sup_first</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">syntax.arguments.grouping</span>
          <select className={`${selectClass} opacity-50`} value="always_explicit" disabled>
            <option value="always_explicit">always_explicit</option>
          </select>
        </label>
        <label className="flex items-center gap-1">
          <span className="text-slate-400">syntax.environments.name_spacing</span>
          <select
            className={selectClass}
            value={envNameSpacing}
            onChange={(e) =>
              update({
                syntax: {
                  environments: { name_spacing: e.target.value as EnvironmentNameSpacing },
                },
              })
            }
          >
            <option value="spaced">spaced</option>
            <option value="compact">compact</option>
          </select>
        </label>
      </div>

      {/* Output */}
      <div className="min-h-0 flex-1 overflow-auto border-t border-slate-200 pt-1">
        {serializeError ? (
          <div className="rounded-sm border border-red-200 bg-red-50 p-2.5 text-xs text-red-800">
            <div className="font-semibold">Serialize Error</div>
            <pre className="m-0 mt-1 whitespace-pre-wrap break-words [font-family:var(--font-code)]">
              {serializeError}
            </pre>
          </div>
        ) : (
          <textarea
            readOnly
            value={serializedOutput ?? ''}
            className="m-0 block h-full w-full resize-none border-0 bg-transparent p-1 text-xs leading-relaxed text-slate-800 outline-none [font-family:var(--font-code)]"
            placeholder="No serialized output."
          />
        )}
      </div>
    </>
  )
}

export default SerializedOutputPane
