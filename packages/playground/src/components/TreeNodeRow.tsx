import SpecPopover from './SpecPopover'
import type { TreeNode } from '../lib/types'

interface TreeNodeRowProps {
  node: TreeNode
  collapsedNodes: Set<string>
  onToggle: (id: string) => void
  depth: number
}

/** Solid filled triangle for expand/collapse — the only custom SVG in the project. */
function TriangleIcon({ collapsed }: { collapsed: boolean }) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="currentColor"
      style={{
        transition: 'transform 0.1s',
        transform: collapsed ? undefined : 'rotate(90deg)',
      }}
    >
      <path d="M6 4l6 4-6 4z" />
    </svg>
  )
}

/** Map node type to its CSS color variable. */
function nodeColor(type: string): string {
  switch (type) {
    case 'Command':
    case 'Infix':
    case 'Declarative':
      return 'var(--node-command)'
    case 'Group':
      return 'var(--node-group)'
    case 'Scripted':
      return 'var(--node-scripted)'
    case 'Char':
    case 'Chars':
    case 'Text':
    case 'ActiveSpace':
      return 'var(--node-char)'
    case 'Arg':
      return 'var(--node-arg)'
    case 'Environment':
      return 'var(--node-env)'
    case 'Unknown':
    case 'UnknownNode':
      return 'var(--node-unknown)'
    case 'Error':
      return 'var(--color-danger-fg)'
    default:
      return 'var(--color-accent-fg)'
  }
}

export default function TreeNodeRow({
  node,
  collapsedNodes,
  onToggle,
  depth,
}: TreeNodeRowProps) {
  const hasChildren = node.children.length > 0
  const isLeaf = !hasChildren
  const collapsed = collapsedNodes.has(node.id)
  const color = nodeColor(node.type)

  return (
    <div className="min-w-max">
      <div
        className={`flex min-h-5 items-center gap-1.5 whitespace-nowrap rounded-sm${hasChildren ? ' tree-node-row' : ''}`}
        style={{ paddingLeft: depth === 0 ? 4 : 0, cursor: hasChildren ? 'pointer' : 'default' }}
        onClick={hasChildren ? () => onToggle(node.id) : undefined}
      >
        {/* Expand/collapse triangle — leaves get invisible spacer */}
        {isLeaf ? (
          <span className="inline-block h-4 w-4" />
        ) : (
          <button
            type="button"
            className="inline-flex h-4 w-4 items-center justify-center rounded-sm border-0 bg-transparent p-0"
            style={{ color: 'var(--color-fg-muted)' }}
            onClick={(event) => {
              event.stopPropagation()
              onToggle(node.id)
            }}
            aria-label={collapsed ? 'Expand node' : 'Collapse node'}
            title={collapsed ? 'Expand node' : 'Collapse node'}
          >
            <TriangleIcon collapsed={collapsed} />
          </button>
        )}

        {/* Role badge (purple) */}
        {node.role ? (
          <span
            className="rounded-sm px-1 py-px text-xs leading-none"
            style={{
              color: 'var(--color-done-fg)',
              background: 'var(--color-done-subtle)',
            }}
          >
            {node.role}
          </span>
        ) : null}

        {/* Type badge colored by node type, with inline arg index */}
        <span
          className="inline-flex items-baseline gap-1 rounded-sm px-1 py-px text-xs font-medium leading-none"
          style={{ color, background: `color-mix(in srgb, ${color}, transparent 88%)` }}
        >
          {node.type}
          {node.argIndex !== undefined ? (
            <span style={{ fontWeight: 400, opacity: 0.6 }}>{node.argIndex}</span>
          ) : null}
        </span>

        {/* Command name */}
        {node.commandName ? (
          <span className="font-bold" style={{ color: 'var(--color-fg-default)' }}>
            {node.commandName}
          </span>
        ) : null}

        {/* Spec popover */}
        {node.specString !== undefined ? (
          <SpecPopover
            specString={node.specString}
            specFromPackages={node.specFromPackages}
            specDetail={node.specDetail}
            explicitSpecString={node.explicitSpecString}
            explicitSpecFromPackages={node.explicitSpecFromPackages}
            explicitSpecDetail={node.explicitSpecDetail}
            characterUnicodeValue={node.characterUnicodeValue}
            characterPackage={node.characterPackage}
            characterMathvariant={node.characterMathvariant}
          />
        ) : null}

        {/* Arg kind — only show "optional" since mandatory is the default */}
        {node.argKind === 'Optional' ? (
          <span
            className="rounded-sm px-1 py-px text-xs leading-none"
            style={{
              color: 'var(--color-attention-fg)',
              background: 'var(--color-attention-subtle)',
              border: '1px solid color-mix(in srgb, var(--color-attention-fg), transparent 70%)',
            }}
          >
            opt
          </span>
        ) : null}

        {/* Subtitle */}
        {node.subtitle ? (
          <span className="text-xs" style={{ color: 'var(--color-fg-muted)' }}>
            {node.subtitle}
          </span>
        ) : null}

        {/* Value */}
        {node.value ? (
          <span style={{ color: 'var(--color-accent-fg)' }}>{node.value}</span>
        ) : null}

        {/* Error message + snippet inline */}
        {node.errorMessage ? (
          <span className="text-xs" style={{ color: 'var(--color-danger-fg)' }}>
            {node.errorMessage}
          </span>
        ) : null}
        {node.errorSnippet ? (
          <span
            className="rounded-sm px-1 py-px text-xs leading-none"
            style={{
              color: 'var(--color-fg-muted)',
              background: 'var(--color-canvas-subtle)',
              border: '1px solid var(--color-border-muted)',
              fontFamily: 'var(--font-mono)',
            }}
          >
            {node.errorSnippet}
          </span>
        ) : null}
      </div>

      {/* Children with indent guide line */}
      {hasChildren && !collapsed ? (
        <div
          className="ml-2 pl-2.5"
          style={{ borderLeft: '1px solid var(--color-border-muted)' }}
        >
          {node.children.map((child) => (
            <TreeNodeRow
              key={child.id}
              node={child}
              collapsedNodes={collapsedNodes}
              onToggle={onToggle}
              depth={depth + 1}
            />
          ))}
        </div>
      ) : null}
    </div>
  )
}
