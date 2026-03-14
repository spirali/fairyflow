import { useState } from 'react';

const ICONS = { node: '📦', rect: '▭', circle: '○' };

/** Accumulate keyframes from t=0 up to and including `step`, unwrapping {value,transition}. */
function resolveProps(node, step) {
  const out = {};
  for (let t = 0; t <= step; t++) {
    const kf = node.keyframes?.[String(t)];
    if (kf) {
      for (const [key, entry] of Object.entries(kf)) {
        out[key] = entry?.value ?? entry;
      }
    }
  }
  return out;
}

/** Resolve props with smooth interpolation for animation mode.
 *  frame: 0..framesPerStep-1 within the current step. */
export function resolvePropsAnimated(node, step, frame, framesPerStep) {
  const cur = resolveProps(node, step);
  if (frame === 0 || step >= Infinity) return cur;

  // Look for smooth transitions in the next step
  const nextKf = node.keyframes?.[String(step + 1)];
  if (!nextKf) return cur;

  const t = frame / framesPerStep;
  const out = { ...cur };

  for (const [key, entry] of Object.entries(nextKf)) {
    if (entry?.transition !== 'smooth') continue;
    const from = cur[key];
    const to = entry.value;
    if (from == null || to == null) continue;

    if (typeof to === 'number' && typeof from === 'number') {
      out[key] = from + (to - from) * t;
    } else if (typeof to === 'string' && to.startsWith('#') && from.startsWith?.('#')) {
      out[key] = lerpColor(from, to, t);
    }
  }
  return out;
}

function lerpColor(a, b, t) {
  const p = (s, i) => parseInt(s.slice(i, i + 2), 16);
  const r = Math.round(p(a, 1) + (p(b, 1) - p(a, 1)) * t);
  const g = Math.round(p(a, 3) + (p(b, 3) - p(a, 3)) * t);
  const bl = Math.round(p(a, 5) + (p(b, 5) - p(a, 5)) * t);
  return `#${r.toString(16).padStart(2,'0')}${g.toString(16).padStart(2,'0')}${bl.toString(16).padStart(2,'0')}`;
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

function fmt(v) {
  return typeof v === 'number' ? (Number.isInteger(v) ? v : v.toFixed(1)) : v;
}

function NodeLabel({ node, step, frame, framesPerStep }) {
  const props = resolvePropsAnimated(node, step, frame, framesPerStep);
  const changed = changedAt(node, step);
  const anyChanged = changed.size > 0;

  const hi = (...keys) => keys.some(k => changed.has(k));

  return (
    <span className={anyChanged ? 'node-label node-label-changed' : 'node-label'}>
      <span className="node-type">{node.type}</span>
      {props.width  != null && <Prop label={`${fmt(props.width)}×${fmt(props.height)}`} changed={hi('width','height')} />}
      {props.color  != null && <Prop label={fmt(props.color)}                            changed={hi('color')} />}
      {props.x      != null && <Prop label={`${fmt(props.x)}, ${fmt(props.y ?? 0)}`}    changed={hi('x','y')} />}
      {props.radius != null && <Prop label={`r=${fmt(props.radius)}`}                   changed={hi('radius')} />}
    </span>
  );
}

function addIds(nodes, prefix = '') {
  return (nodes ?? []).map((n, i) => {
    const id = `${prefix}${i}`;
    return { ...n, id, children: addIds(n.children, `${id}.`) };
  });
}

function TreeNode({ node, depth, selected, onSelect, step, frame, framesPerStep }) {
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
        <NodeLabel node={node} step={step} frame={frame} framesPerStep={framesPerStep} />
      </div>
      {open &&
        node.children?.map((child) => (
          <TreeNode key={child.id} node={child} depth={depth + 1} selected={selected} onSelect={onSelect} step={step} frame={frame} framesPerStep={framesPerStep} />
        ))}
    </div>
  );
}

export default function TreeView({ nodes, step = 0, frame = 0, framesPerStep = 1 }) {
  const [selected, setSelected] = useState(null);
  const tree = nodes?.length ? nodes : null;

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      {tree
        ? tree.map((n) => (
            <TreeNode key={n.id} node={n} depth={0} selected={selected} onSelect={setSelected} step={step} frame={frame} framesPerStep={framesPerStep} />
          ))
        : <div className="tree-empty">Run a script to see the scene tree</div>
      }
    </div>
  );
}

export { addIds };
