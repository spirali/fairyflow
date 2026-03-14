import { useState } from 'react';

const ICONS = { node: '📦', rect: '▭', circle: '○' };

function nodeLabel(n) {
  const { width, height, radius, color } = n.props ?? {};
  let label = n.type;
  if (width != null && height != null) label += ` ${width}×${height}`;
  else if (radius != null) label += ` r=${radius}`;
  if (color) label += ` [${color}]`;
  return label;
}

function addIds(nodes, prefix = '') {
  return nodes.map((n, i) => {
    const id = `${prefix}${i}`;
    return { ...n, id, children: addIds(n.children ?? [], `${id}.`) };
  });
}

function TreeNode({ node, depth, selected, onSelect }) {
  const [open, setOpen] = useState(true);
  const hasChildren = node.children?.length > 0;

  return (
    <div>
      <div
        className={`tree-node${selected === node.id ? ' selected' : ''}`}
        style={{ paddingLeft: 8 + depth * 16 }}
        onClick={() => {
          if (hasChildren) setOpen((o) => !o);
          onSelect(node.id);
        }}
      >
        <span className="tree-node-toggle">
          {hasChildren ? (open ? '▾' : '▸') : ''}
        </span>
        <span className="tree-node-icon">{ICONS[node.type] ?? '◆'}</span>
        {nodeLabel(node)}
      </div>
      {open &&
        node.children?.map((child) => (
          <TreeNode
            key={child.id}
            node={child}
            depth={depth + 1}
            selected={selected}
            onSelect={onSelect}
          />
        ))}
    </div>
  );
}

export default function TreeView({ nodes }) {
  const [selected, setSelected] = useState(null);

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      {nodes?.length
        ? nodes.map((n) => (
            <TreeNode key={n.id} node={n} depth={0} selected={selected} onSelect={setSelected} />
          ))
        : <div className="tree-empty">Run a script to see the scene tree</div>
      }
    </div>
  );
}
