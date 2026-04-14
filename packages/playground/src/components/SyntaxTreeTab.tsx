import TreeNodeRow from './TreeNodeRow'
import type { TreeNode } from '../lib/types'

interface SyntaxTreeTabProps {
  treeRoot: TreeNode | null
  collapsedNodes: Set<string>
  onToggleNode: (id: string) => void
  nodeCount: number
  treeDepth: number
  parseTime: number | null
}

export default function SyntaxTreeTab({
  treeRoot,
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
