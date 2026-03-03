import type { AllowedMode, CommandKind, ParseDiagnostic } from '../texformWasm'
import type { CustomCommandEntry } from '../appTypes'

interface LatexInputPaneProps {
  paneClass: string
  sectionHeadClass: string
  sectionTitleClass: string
  buttonClass: string
  source: string
  strictMode: boolean
  fatalMessage: string | null
  diagnostics: ParseDiagnostic[]
  customCommands: CustomCommandEntry[]
  showCommandForm: boolean
  customCommandName: string
  customCommandKind: CommandKind
  customCommandMode: AllowedMode
  customCommandSpec: string
  customCommandError: string | null
  rootSpanText: string
  treeDepth: number
  nodesCount: number
  onResetSample: () => void
  onStrictModeChange: (checked: boolean) => void
  onSourceChange: (source: string) => void
  onToggleCommandForm: () => void
  onCustomCommandNameChange: (name: string) => void
  onCustomCommandSpecChange: (spec: string) => void
  onCustomCommandKindChange: (kind: CommandKind) => void
  onCustomCommandModeChange: (mode: AllowedMode) => void
  onAddCustomCommand: () => void
  onRemoveCustomCommand: (name: string) => void
  onResetAllCustomCommands: () => void
}

function LatexInputPane({
  paneClass,
  sectionHeadClass,
  sectionTitleClass,
  buttonClass,
  source,
  strictMode,
  fatalMessage,
  diagnostics,
  customCommands,
  showCommandForm,
  customCommandName,
  customCommandKind,
  customCommandMode,
  customCommandSpec,
  customCommandError,
  rootSpanText,
  treeDepth,
  nodesCount,
  onResetSample,
  onStrictModeChange,
  onSourceChange,
  onToggleCommandForm,
  onCustomCommandNameChange,
  onCustomCommandSpecChange,
  onCustomCommandKindChange,
  onCustomCommandModeChange,
  onAddCustomCommand,
  onRemoveCustomCommand,
  onResetAllCustomCommands,
}: LatexInputPaneProps) {
  return (
    <section className={`${paneClass} min-h-80 lg:min-h-0`}>
      <div className={sectionHeadClass}>
        <h2 className={sectionTitleClass}>LaTeX Input</h2>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className={buttonClass} onClick={onResetSample}>
          Reset Sample
        </button>
        <label className="inline-flex select-text items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            className="m-0"
            checked={strictMode}
            onChange={(event) => onStrictModeChange(event.target.checked)}
          />
          Strict Mode
        </label>
      </div>

      <textarea
        value={source}
        onChange={(event) => onSourceChange(event.target.value)}
        className="min-h-56 w-full resize-y rounded-sm border border-slate-300 bg-white p-2.5 text-sm leading-normal text-slate-900 [font-family:var(--font-code)]"
        placeholder="Input LaTeX formula..."
        spellCheck={false}
      />

      {fatalMessage !== null || diagnostics.length > 0 ? (
        <div
          className={`mt-1 rounded-sm border p-2 ${
            fatalMessage !== null
              ? 'border-red-200 bg-red-50 text-red-800'
              : 'border-yellow-200 bg-yellow-50 text-yellow-800'
          }`}
        >
          <div className="text-xs font-semibold">
            {fatalMessage !== null
              ? diagnostics.length > 0
                ? `Parse Error (${diagnostics.length} diagnostics)`
                : 'Parse Error'
              : `Diagnostics (${diagnostics.length})`}
          </div>
          {fatalMessage !== null ? (
            <p className="mt-1 mb-0 whitespace-pre-wrap text-xs [font-family:var(--font-code)]">
              {fatalMessage}
            </p>
          ) : null}
          {diagnostics.length > 0 ? (
            <ul className="mt-1.5 list-disc pl-4 text-xs">
              {diagnostics.map((diagnostic, index) => (
                <li key={`${diagnostic.message}-${index}`} className="my-1 flex flex-wrap items-baseline gap-2">
                  <span>{diagnostic.message}</span>
                  <span className="[font-family:var(--font-code)] opacity-80">
                    span {diagnostic.span.start}..{diagnostic.span.end}
                  </span>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : null}

      <div className="border-t border-slate-200 pt-2">
        <div className="flex items-center justify-between">
          <div className="text-xs font-semibold text-slate-700">Custom Commands</div>
          <div className="flex items-center gap-1.5">
            {customCommands.length > 0 ? (
              <button
                type="button"
                className="rounded-sm px-1.5 py-1 text-xs leading-none text-slate-400 transition-colors hover:bg-red-50 hover:text-red-500"
                onClick={onResetAllCustomCommands}
                title="Remove all custom commands"
              >
                Reset
              </button>
            ) : null}
            <button
              type="button"
              className="rounded-sm px-1.5 py-1 text-xs leading-none text-blue-500 transition-colors hover:bg-blue-50 hover:text-blue-600"
              onClick={onToggleCommandForm}
            >
              {showCommandForm ? '− Cancel' : '+ Add'}
            </button>
          </div>
        </div>

        {showCommandForm ? (
          <form
            className="mt-1.5 rounded border border-blue-100 bg-blue-50/30 p-2"
            onSubmit={(event) => {
              event.preventDefault()
              onAddCustomCommand()
            }}
          >
            <div className="grid grid-cols-[1fr_1fr] gap-x-2 gap-y-1">
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Name
                <input
                  value={customCommandName}
                  onChange={(event) => onCustomCommandNameChange(event.target.value)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                  placeholder="e.g. dv"
                  autoFocus
                />
              </label>
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Spec
                <input
                  value={customCommandSpec}
                  onChange={(event) => onCustomCommandSpecChange(event.target.value)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs [font-family:var(--font-code)] focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                  placeholder="e.g. s o m"
                />
              </label>
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Kind
                <select
                  value={customCommandKind}
                  onChange={(event) => onCustomCommandKindChange(event.target.value as CommandKind)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs"
                >
                  <option value="prefix">prefix</option>
                  <option value="infix">infix</option>
                  <option value="declarative">declarative</option>
                </select>
              </label>
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Mode
                <select
                  value={customCommandMode}
                  onChange={(event) => onCustomCommandModeChange(event.target.value as AllowedMode)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs"
                >
                  <option value="math">math</option>
                  <option value="text">text</option>
                  <option value="both">both</option>
                </select>
              </label>
            </div>

            <div className="mt-2 flex items-center gap-2">
              <button
                type="submit"
                className="rounded-sm border border-blue-200 bg-blue-50 px-2.5 py-1 text-xs font-medium leading-tight text-blue-600 transition-colors hover:bg-blue-100"
              >
                Add Command
              </button>
              {customCommandError ? <span className="text-xs text-red-600">{customCommandError}</span> : null}
            </div>
          </form>
        ) : null}

        {customCommands.length > 0 ? (
          <div className="mt-1.5 max-h-32 space-y-1 overflow-y-auto">
            {customCommands.map((command) => (
              <div
                key={command.name}
                className="flex items-center gap-1.5 rounded border border-slate-200 bg-slate-50/70 px-2 py-1"
              >
                <span className="min-w-0 shrink-0 text-xs font-semibold [font-family:var(--font-code)]">
                  \{command.name}
                </span>
                <span className="rounded-sm bg-blue-100 px-1 py-px text-xs leading-none text-blue-700">
                  {command.kind}
                </span>
                <span className="rounded-sm bg-emerald-100 px-1 py-px text-xs leading-none text-emerald-700">
                  {command.mode}
                </span>
                <span className="min-w-0 flex-1 truncate text-xs text-slate-400 [font-family:var(--font-code)]">
                  {command.spec || '(no spec)'}
                </span>
                <button
                  type="button"
                  className="ml-auto shrink-0 rounded-sm px-1 py-px text-xs leading-tight text-slate-300 transition-colors hover:bg-red-50 hover:text-red-500"
                  onClick={() => onRemoveCustomCommand(command.name)}
                  title="Remove command"
                >
                  ✕
                </button>
              </div>
            ))}
          </div>
        ) : !showCommandForm ? (
          <p className="mt-1 text-xs italic text-slate-400">No custom commands.</p>
        ) : null}
      </div>

      <div className="mt-auto border-t border-slate-200 pt-2">
        <div className="text-xs font-semibold text-slate-700">Statistics (Placeholder)</div>
        <ul className="mt-1.5 list-disc pl-4 text-xs leading-normal text-slate-700">
          <li>Chars: {source.length}</li>
          <li>Nodes: {nodesCount}</li>
          <li>Tree Depth: {treeDepth}</li>
          <li>Diagnostics: {diagnostics.length}</li>
          <li>Root Span: {rootSpanText}</li>
          <li className="text-slate-500">TODO: token stats / complexity score</li>
        </ul>
      </div>
    </section>
  )
}

export default LatexInputPane
