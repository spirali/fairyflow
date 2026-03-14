import { useState } from 'react';

const TREE = {
  id: 'root',
  label: 'root',
  icon: '📁',
  children: [
    {
      id: 'node1',
      label: 'node',
      icon: '📦',
      children: [
        { id: 'rect1', label: 'rect', icon: '▭', children: [] },
        { id: 'rect2', label: 'rect', icon: '▭', children: [] },
      ],
    },
    {
      id: 'node2',
      label: 'node',
      icon: '📦',
      children: [
        { id: 'rect3', label: 'rect', icon: '▭', children: [] },
      ],
    },
  ],
};

function TreeNode({ node, depth, selected, onSelect }) {
  const [open, setOpen] = useState(true);
  const hasChildren = node.children.length > 0;

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
        <span className="tree-node-icon">{node.icon}</span>
        {node.label}
      </div>
      {open &&
        node.children.map((child) => (
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

export default function TreeView() {
  const [selected, setSelected] = useState('root');

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      <TreeNode node={TREE} depth={0} selected={selected} onSelect={setSelected} />
    </div>
  );
}
