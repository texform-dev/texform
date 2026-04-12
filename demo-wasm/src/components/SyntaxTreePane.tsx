import type { ReactNode } from 'react'
import type { TreeNode } from '../appTypes'

interface SyntaxTreePaneProps {
  paneClass: string
  sectionHeadClass: string
  sectionTitleClass: string
  buttonClass: string
  statusText: string
  statusToneClass: string
  treeRoot: TreeNode | null
  parseErrorMessage: string | null
  onExpandAll: () => void
  onCollapseAll: () => void
  renderTreeNode: (node: TreeNode) => ReactNode
  className?: string
}

function SyntaxTreePane({
  paneClass,
  sectionHeadClass,
  sectionTitleClass,
  buttonClass,
  statusText,
  statusToneClass,
  treeRoot,
  parseErrorMessage,
  onExpandAll,
  onCollapseAll,
  renderTreeNode,
  className,
}: SyntaxTreePaneProps) {
  return (
    <section className={`${paneClass} min-h-0${className ? ` ${className}` : ''}`}>
      <div className={sectionHeadClass}>
        <div className="flex items-center gap-2">
          <h2 className={sectionTitleClass}>Syntax Tree</h2>
          <span
            className={`inline-flex items-center rounded-sm border px-2 py-0.5 text-xs font-medium ${statusToneClass}`}
          >
            {statusText}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <button type="button" className={buttonClass} onClick={onExpandAll}>
            Expand All
          </button>
          <button type="button" className={buttonClass} onClick={onCollapseAll}>
            Collapse All
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-auto border-t border-slate-200 pt-2 pr-1 text-sm leading-snug [font-family:var(--font-code)]">
        {treeRoot ? (
          renderTreeNode(treeRoot)
        ) : parseErrorMessage !== null ? (
          <div className="rounded-sm border border-red-200 bg-red-50 p-2.5 text-xs text-red-800">
            <div className="font-semibold">Parse Error</div>
            <pre className="m-0 mt-1 whitespace-pre-wrap break-words [font-family:var(--font-code)]">
              {parseErrorMessage}
            </pre>
          </div>
        ) : (
          <p className="m-0 text-xs text-slate-600">No syntax tree available.</p>
        )}
      </div>
    </section>
  )
}

export default SyntaxTreePane
