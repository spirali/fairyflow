import { useState } from 'react';
import type { RawNode, TreeNodeData } from '../types';
import { NodeKindIcon } from './Icons';

function fmt(v: number | string): number | string {
  return typeof v === 'number' ? (Number.isInteger(v) ? v : v.toFixed(1)) : v;
}

function Prop({ label, changed }: { label: number | string; changed: boolean }) {
  return changed
    ? <span className="prop-chip prop-chip-changed">{label}</span>
    : <span className="prop-chip">{label}</span>;
}

function NodeLabel({ node, prevNode }: { node: TreeNodeData; prevNode?: TreeNodeData }) {
  const ch = (key: keyof TreeNodeData): boolean =>
    prevNode != null && prevNode[key] !== node[key];

  const anyChanged = (['width', 'height', 'fill_color', 'alpha', 'x', 'radius'] as const).some(ch);

  return (
    <span className={anyChanged ? 'node-label node-label-changed' : 'node-label'}>
      <span className="node-type">{node.kind}</span>
      {node.width     != null && <Prop label={`${fmt(node.width)}×${fmt(node.height ?? 0)}`} changed={ch('width') || ch('height')} />}
      {node.fill_color != null && (
        <span className={ch('fill_color') ? 'prop-chip prop-chip-changed' : 'prop-chip'}>
          <span style={{ display: 'inline-block', width: 10, height: 10, background: node.fill_color, border: '1px solid rgba(255,255,255,0.3)', borderRadius: 2, verticalAlign: 'middle', marginRight: 3 }} />
          {fmt(node.fill_color)}
        </span>
      )}
      {node.alpha != null && node.alpha !== 1 && <Prop label={`α=${fmt(node.alpha)}`} changed={ch('alpha')} />}
      {node.x      != null && <Prop label={`x=${fmt(node.x)}`}      changed={ch('x')} />}
      {node.radius != null && <Prop label={`r=${fmt(node.radius)}`} changed={ch('radius')} />}
    </span>
  );
}

interface TreeNodeProps {
  node: TreeNodeData;
  prevNode?: TreeNodeData;
  depth: number;
  selected: string | null;
  onSelect: (id: string) => void;
}

function TreeNode({ node, prevNode, depth, selected, onSelect }: TreeNodeProps) {
  const [open, setOpen] = useState(true);
  const hasChildren = (node.children?.length ?? 0) > 0;

  const anyChanged = prevNode != null &&
    (['width', 'height', 'fill_color', 'alpha', 'x', 'radius'] as const).some(k => prevNode[k] !== node[k]);

  return (
    <div>
      <div
        className={['tree-node', selected === node.id ? 'selected' : '', anyChanged ? 'node-changed' : ''].filter(Boolean).join(' ')}
        style={{ paddingLeft: 8 + depth * 16 }}
        onClick={() => {
          if (hasChildren) setOpen((o) => !o);
          onSelect(node.id);
        }}
      >
        <span className="tree-node-toggle">{hasChildren ? (open ? '▾' : '▸') : ''}</span>
        <span className="tree-node-icon"><NodeKindIcon kind={node.kind} /></span>
        <NodeLabel node={node} prevNode={prevNode} />
      </div>
      {open &&
        node.children?.map((child, i) => (
          <TreeNode
            key={child.id}
            node={child}
            prevNode={prevNode?.children?.[i]}
            depth={depth + 1}
            selected={selected}
            onSelect={onSelect}
          />
        ))}
    </div>
  );
}

interface TreeViewProps {
  nodes: TreeNodeData[];
  prevNodes: TreeNodeData[];
}

export default function TreeView({ nodes, prevNodes }: TreeViewProps) {
  const [selected, setSelected] = useState<string | null>(null);
  const tree = nodes.length > 0 ? nodes : null;

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      {tree
        ? tree.map((n, i) => (
            <TreeNode key={n.id} node={n} prevNode={prevNodes[i]} depth={0} selected={selected} onSelect={setSelected} />
          ))
        : <div className="tree-empty">Run a script to see the scene tree</div>
      }
    </div>
  );
}

export function addIds(nodes: RawNode[] | undefined, prefix = ''): TreeNodeData[] {
  return (nodes ?? []).map((n, i) => {
    const id = `${prefix}${i}`;
    return { ...n, id, children: addIds(n.children, `${id}.`) };
  });
}
