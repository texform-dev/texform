import TreeNodeRow from './TreeNodeRow'
import type { TreeNode } from '../lib/types'

interface SyntaxTreeTabProps {
  treeRoot: TreeNode | null
  parseErrorMessage: string | null
  collapsedNodes: Set<string>
  onToggleNode: (id: string) => void
  nodeCount: number
  treeDepth: number
  parseTime: number | null
}

export default function SyntaxTreeTab({
  treeRoot,
  parseErrorMessage,
  collapsedNodes,
  onToggleNode,
  nodeCount,
  treeDepth,
  parseTime,
}: SyntaxTreeTabProps) {
  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      <div
        className="flex-1 overflow-auto py-2"
        style={{ fontFamily: 'var(--font-mono)', fontSize: 13 }}
      >
        {treeRoot ? (
          <TreeNodeRow
            node={treeRoot}
            collapsedNodes={collapsedNodes}
            onToggle={onToggleNode}
            depth={0}
          />
        ) : parseErrorMessage !== null ? (
          <div
            className="mx-3 rounded p-2.5 text-xs"
            style={{
              background: 'var(--color-danger-subtle)',
              color: 'var(--color-danger-fg)',
              border: '1px solid var(--color-border-default)',
            }}
          >
            <div className="font-semibold">Parse Error</div>
            <pre
              className="m-0 mt-1 whitespace-pre-wrap break-words"
              style={{ fontFamily: 'var(--font-mono)' }}
            >
              {parseErrorMessage}
            </pre>
          </div>
        ) : (
          <p className="px-3 text-xs" style={{ color: 'var(--color-fg-muted)' }}>
            No syntax tree available.
          </p>
        )}
      </div>

      <div
        className="flex shrink-0 items-center justify-end gap-3 border-t px-3 py-1 text-[11px]"
        style={{
          fontFamily: 'var(--font-mono)',
          background: 'var(--color-canvas-subtle)',
          borderColor: 'var(--color-border-muted)',
          color: 'var(--color-fg-subtle)',
        }}
      >
        <span>{nodeCount} nodes</span>
        <span>depth {treeDepth}</span>
        {parseTime !== null && <span>{parseTime.toFixed(1)} ms</span>}
      </div>
    </div>
  )
}
