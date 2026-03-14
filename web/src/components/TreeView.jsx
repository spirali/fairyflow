import { useState } from 'react';

const ICONS = { node: '📦', rect: '▭', circle: '○' };

/** Accumulate keyframes from t=0 up to and including `step`. */
function resolveProps(node, step) {
  const out = {};
  for (let t = 0; t <= step; t++) {
    const kf = node.keyframes?.[String(t)];
    if (kf) Object.assign(out, kf);
  }
  return out;
}

/** Whether a node exists at this step. */
function isVisible(node, step) {
  const start = node.start ?? 0;
  const end = node.end ?? Infinity;
  return step >= start && step < end;
}

/** Keys highlighted at this step: explicit keyframe changes, or all props when the node first appears. */
function changedAt(node, step) {
  const start = node.start ?? 0;
  if (step === start && start > 0) {
    // Node just appeared — highlight all its initial props
    return new Set(Object.keys(node.keyframes?.[String(start)] ?? {}));
  }
  if (step === 0) return new Set();
  return new Set(Object.keys(node.keyframes?.[String(step)] ?? {}));
}

function Prop({ label, changed }) {
  return changed
    ? <span className="prop-chip prop-chip-changed">{label}</span>
    : <span className="prop-chip">{label}</span>;
}

function NodeLabel({ node, step }) {
  const props = resolveProps(node, step);
  const changed = changedAt(node, step);
  const anyChanged = changed.size > 0;

  const hi = (...keys) => keys.some(k => changed.has(k));

  return (
    <span className={anyChanged ? 'node-label node-label-changed' : 'node-label'}>
      <span className="node-type">{node.type}</span>
      {props.width  != null && <Prop label={`${props.width}×${props.height}`} changed={hi('width','height')} />}
      {props.color  != null && <Prop label={props.color}                       changed={hi('color')} />}
      {props.x      != null && <Prop label={`${props.x}, ${props.y ?? 0}`}    changed={hi('x','y')} />}
      {props.radius != null && <Prop label={`r=${props.radius}`}               changed={hi('radius')} />}
    </span>
  );
}

function addIds(nodes, prefix = '') {
  return (nodes ?? []).map((n, i) => {
    const id = `${prefix}${i}`;
    return { ...n, id, children: addIds(n.children, `${id}.`) };
  });
}

function TreeNode({ node, depth, selected, onSelect, step }) {
  const [open, setOpen] = useState(true);

  if (!isVisible(node, step)) return null;

  const visibleChildren = node.children?.filter(c => isVisible(c, step)) ?? [];
  const hasChildren = visibleChildren.length > 0;
  const start = node.start ?? 0;
  const isNew = step === start && start > 0;
  const hasChanges = !isNew && changedAt(node, step).size > 0;
  const rowClass = ['tree-node',
    selected === node.id ? 'selected'     : '',
    isNew                ? 'node-new'     : '',
    hasChanges           ? 'node-changed' : '',
  ].filter(Boolean).join(' ');

  return (
    <div>
      <div className={rowClass}
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
        <NodeLabel node={node} step={step} />
      </div>
      {open &&
        node.children?.map((child) => (
          <TreeNode key={child.id} node={child} depth={depth + 1} selected={selected} onSelect={onSelect} step={step} />
        ))}
    </div>
  );
}

export default function TreeView({ nodes, step = 0 }) {
  const [selected, setSelected] = useState(null);
  const tree = nodes?.length ? nodes : null;

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      {tree
        ? tree.map((n) => (
            <TreeNode key={n.id} node={n} depth={0} selected={selected} onSelect={setSelected} step={step} />
          ))
        : <div className="tree-empty">Run a script to see the scene tree</div>
      }
    </div>
  );
}

export { addIds };
