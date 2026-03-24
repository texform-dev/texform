import type { AllowedMode, BodyMode, CommandKind, ParseDiagnostic } from '../texformWasm'
import type {
  CustomKnowledgeRecordEntry,
  CustomKnowledgeRecordTarget,
} from '../appTypes'

interface LatexInputPaneProps {
  paneClass: string
  sectionHeadClass: string
  sectionTitleClass: string
  buttonClass: string
  source: string
  strictMode: boolean
  fatalMessage: string | null
  diagnostics: ParseDiagnostic[]
  customKnowledgeRecords: CustomKnowledgeRecordEntry[]
  activeCustomRecordForm: CustomKnowledgeRecordTarget | null
  customRecordName: string
  customCommandKind: CommandKind
  customRecordMode: AllowedMode
  customEnvironmentBodyMode: BodyMode
  customRecordSpec: string
  customRecordError: string | null
  rootSpanText: string
  treeDepth: number
  nodesCount: number
  onResetSample: () => void
  onStrictModeChange: (checked: boolean) => void
  onSourceChange: (source: string) => void
  onToggleCustomRecordForm: (target: CustomKnowledgeRecordTarget) => void
  onCustomRecordNameChange: (name: string) => void
  onCustomRecordSpecChange: (spec: string) => void
  onCustomCommandKindChange: (kind: CommandKind) => void
  onCustomRecordModeChange: (mode: AllowedMode) => void
  onCustomEnvironmentBodyModeChange: (mode: BodyMode) => void
  onAddCustomCommand: () => void
  onAddCustomEnvironment: () => void
  onRemoveCustomRecord: (record: CustomKnowledgeRecordEntry) => void
  onResetAllCustomKnowledgeRecords: () => void
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
  customKnowledgeRecords,
  activeCustomRecordForm,
  customRecordName,
  customCommandKind,
  customRecordMode,
  customEnvironmentBodyMode,
  customRecordSpec,
  customRecordError,
  rootSpanText,
  treeDepth,
  nodesCount,
  onResetSample,
  onStrictModeChange,
  onSourceChange,
  onToggleCustomRecordForm,
  onCustomRecordNameChange,
  onCustomRecordSpecChange,
  onCustomCommandKindChange,
  onCustomRecordModeChange,
  onCustomEnvironmentBodyModeChange,
  onAddCustomCommand,
  onAddCustomEnvironment,
  onRemoveCustomRecord,
  onResetAllCustomKnowledgeRecords,
}: LatexInputPaneProps) {
  const isCommandForm = activeCustomRecordForm === 'command'
  const isEnvironmentForm = activeCustomRecordForm === 'environment'

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
          <div className="text-xs font-semibold text-slate-700">Custom Knowledge Records</div>
          <div className="flex items-center gap-1.5">
            {customKnowledgeRecords.length > 0 ? (
              <button
                type="button"
                className="rounded-sm px-1.5 py-1 text-xs leading-none text-slate-400 transition-colors hover:bg-red-50 hover:text-red-500"
                onClick={onResetAllCustomKnowledgeRecords}
                title="Remove all custom knowledge records"
              >
                Reset
              </button>
            ) : null}
            <button
              type="button"
              className="rounded-sm px-1.5 py-1 text-xs leading-none text-blue-500 transition-colors hover:bg-blue-50 hover:text-blue-600"
              onClick={() => onToggleCustomRecordForm('command')}
            >
              {isCommandForm ? '− Cancel Command' : '+ Add Command'}
            </button>
            <button
              type="button"
              className="rounded-sm px-1.5 py-1 text-xs leading-none text-teal-600 transition-colors hover:bg-teal-50 hover:text-teal-700"
              onClick={() => onToggleCustomRecordForm('environment')}
            >
              {isEnvironmentForm ? '− Cancel Environment' : '+ Add Environment'}
            </button>
          </div>
        </div>

        {activeCustomRecordForm !== null ? (
          <form
            className="mt-1.5 rounded border border-blue-100 bg-blue-50/30 p-2"
            onSubmit={(event) => {
              event.preventDefault()
              if (isEnvironmentForm) {
                onAddCustomEnvironment()
                return
              }
              onAddCustomCommand()
            }}
          >
            <div className="grid grid-cols-[1fr_1fr] gap-x-2 gap-y-1">
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Name
                <input
                  value={customRecordName}
                  onChange={(event) => onCustomRecordNameChange(event.target.value)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                  placeholder={isEnvironmentForm ? 'e.g. proofbox' : 'e.g. dv'}
                  autoFocus
                />
              </label>
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Spec
                <input
                  value={customRecordSpec}
                  onChange={(event) => onCustomRecordSpecChange(event.target.value)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs [font-family:var(--font-code)] focus:border-blue-400 focus:outline-none focus:ring-1 focus:ring-blue-200"
                  placeholder="e.g. s o m"
                />
              </label>
              <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                Allowed Mode
                <select
                  value={customRecordMode}
                  onChange={(event) => onCustomRecordModeChange(event.target.value as AllowedMode)}
                  className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs"
                >
                  <option value="math">math</option>
                  <option value="text">text</option>
                  <option value="both">both</option>
                </select>
              </label>
              {isCommandForm ? (
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
              ) : null}
              {isEnvironmentForm ? (
                <label className="text-xs font-medium uppercase tracking-wide text-slate-400">
                  Body Mode
                  <select
                    value={customEnvironmentBodyMode}
                    onChange={(event) =>
                      onCustomEnvironmentBodyModeChange(event.target.value as BodyMode)
                    }
                    className="mt-1 block w-full rounded-sm border border-slate-300 bg-white px-2 py-1 text-xs"
                  >
                    <option value="math">math</option>
                    <option value="text">text</option>
                  </select>
                </label>
              ) : null}
            </div>

            <div className="mt-2 flex items-center gap-2">
              <button
                type="submit"
                className="rounded-sm border border-blue-200 bg-blue-50 px-2.5 py-1 text-xs font-medium leading-tight text-blue-600 transition-colors hover:bg-blue-100"
              >
                {isEnvironmentForm ? 'Add Environment' : 'Add Command'}
              </button>
              {customRecordError ? <span className="text-xs text-red-600">{customRecordError}</span> : null}
            </div>
          </form>
        ) : null}

        {customKnowledgeRecords.length > 0 ? (
          <div className="mt-1.5 max-h-32 space-y-1 overflow-y-auto">
            {customKnowledgeRecords.map((record) => (
              <div
                key={`${record.target}:${record.name}`}
                className="flex items-center gap-1.5 rounded border border-slate-200 bg-slate-50/70 px-2 py-1"
              >
                <span className="min-w-0 shrink-0 text-xs font-semibold [font-family:var(--font-code)]">
                  {record.target === 'command' ? `\\${record.name}` : `\\begin{${record.name}}`}
                </span>
                <span className="rounded-sm bg-slate-200 px-1 py-px text-xs leading-none text-slate-700">
                  {record.target}
                </span>
                {record.target === 'command' ? (
                  <span className="rounded-sm bg-blue-100 px-1 py-px text-xs leading-none text-blue-700">
                    {record.kind}
                  </span>
                ) : (
                  <span className="rounded-sm bg-teal-100 px-1 py-px text-xs leading-none text-teal-700">
                    body {record.bodyMode}
                  </span>
                )}
                <span className="rounded-sm bg-emerald-100 px-1 py-px text-xs leading-none text-emerald-700">
                  {record.mode}
                </span>
                <span className="min-w-0 flex-1 truncate text-xs text-slate-400 [font-family:var(--font-code)]">
                  {record.spec || '(no spec)'}
                </span>
                <button
                  type="button"
                  className="ml-auto shrink-0 rounded-sm px-1 py-px text-xs leading-tight text-slate-300 transition-colors hover:bg-red-50 hover:text-red-500"
                  onClick={() => onRemoveCustomRecord(record)}
                  title="Remove record"
                >
                  ✕
                </button>
              </div>
            ))}
          </div>
        ) : activeCustomRecordForm === null ? (
          <p className="mt-1 text-xs italic text-slate-400">No custom knowledge records.</p>
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
