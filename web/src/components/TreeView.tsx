import { useState } from "react";
import type { RawNode, RawImageLayer, SceneData, TreeNodeData } from "../types";
import { NodeKindIcon } from "./Icons";

const KIND_LABELS: Partial<Record<TreeNodeData["kind"], string>> = {
  t_group: "TextGroup",
  t_span: "TextSpan",
};

function fmt(v: number | string): number | string {
  return typeof v === "number" ? (Number.isInteger(v) ? v : v.toFixed(1)) : v;
}

function isTransparent(color: string): boolean {
  return color === "#00000000";
}

function shortHex(color: string): string {
  const m6 = color.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (m6) {
    const [, r, g, b] = m6;
    if (r[0] === r[1] && g[0] === g[1] && b[0] === b[1]) return `#${r[0]}${g[0]}${b[0]}`;
    return color;
  }
  const m8 = color.match(/^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (m8) {
    const [, r, g, b, a] = m8;
    if (r[0] === r[1] && g[0] === g[1] && b[0] === b[1] && a[0] === a[1])
      return `#${r[0]}${g[0]}${b[0]}${a[0]}`;
    return color;
  }
  return color;
}

function Prop({ label, changed }: { label: number | string; changed: boolean }) {
  return changed ? (
    <span className="prop-chip prop-chip-changed">{label}</span>
  ) : (
    <span className="prop-chip">{label}</span>
  );
}

function NodeLabel({
  node,
  prevNode,
  displayLabel,
}: {
  node: TreeNodeData;
  prevNode?: TreeNodeData;
  displayLabel: string;
}) {
  const ch = (key: keyof TreeNodeData): boolean => prevNode != null && prevNode[key] !== node[key];

  const TRACKED = [
    "width",
    "height",
    "fill_color",
    "stroke_color",
    "stroke_width",
    "alpha",
    "x",
    "y",
    "scale_x",
    "scale_y",
    "rotation",
    "c1_x",
    "c1_y",
    "c2_x",
    "c2_y",
    "z_level",
  ] as const;
  const anyChanged = TRACKED.some(ch);

  const spanText =
    node.text != null ? (node.text.length > 30 ? node.text.slice(0, 28) + "…" : node.text) : null;

  return (
    <span className={anyChanged ? "node-label node-label-changed" : "node-label"}>
      <span className="node-type">{displayLabel}</span>
      {node.width != null && (
        <Prop
          label={`${fmt(node.width)}×${fmt(node.height ?? 0)}`}
          changed={ch("width") || ch("height")}
        />
      )}
      {node.fill_color != null && !isTransparent(node.fill_color) && (
        <span className={ch("fill_color") ? "prop-chip prop-chip-changed" : "prop-chip"}>
          <span
            style={{
              display: "inline-block",
              width: 10,
              height: 10,
              background: node.fill_color,
              border: "1px solid rgba(255,255,255,0.3)",
              borderRadius: 2,
              verticalAlign: "middle",
              marginRight: 3,
            }}
          />
          {shortHex(node.fill_color)}
        </span>
      )}
      {node.stroke_color != null && !isTransparent(node.stroke_color) && (
        <span
          className={
            ch("stroke_color") || ch("stroke_width") ? "prop-chip prop-chip-changed" : "prop-chip"
          }
        >
          <span
            style={{
              display: "inline-block",
              width: 10,
              height: 10,
              background: "transparent",
              border: `2px solid ${node.stroke_color}`,
              borderRadius: 2,
              verticalAlign: "middle",
              marginRight: 3,
            }}
          />
          {shortHex(node.stroke_color)}
          {node.stroke_width !== 1 && ` ${fmt(node.stroke_width ?? 1)}px`}
        </span>
      )}
      {node.alpha != null && node.alpha !== 1 && (
        <Prop label={`α=${fmt(node.alpha)}`} changed={ch("alpha")} />
      )}
      {node.x != null && (
        <Prop label={`(${fmt(node.x)}, ${fmt(node.y ?? 0)})`} changed={ch("x") || ch("y")} />
      )}
      {node.c1_x != null && (
        <Prop
          label={`c1=(${fmt(node.c1_x)}, ${fmt(node.c1_y ?? 0)})`}
          changed={ch("c1_x") || ch("c1_y")}
        />
      )}
      {node.c2_x != null && (
        <Prop
          label={`c2=(${fmt(node.c2_x)}, ${fmt(node.c2_y ?? 0)})`}
          changed={ch("c2_x") || ch("c2_y")}
        />
      )}
      {node.scale_x != null && (node.scale_x !== 1 || node.scale_y !== 1) && (
        <Prop
          label={
            node.scale_x === node.scale_y
              ? `s=${fmt(node.scale_x)}`
              : `sx=${fmt(node.scale_x)} sy=${fmt(node.scale_y ?? 1)}`
          }
          changed={ch("scale_x") || ch("scale_y")}
        />
      )}
      {node.rotation != null && node.rotation !== 0 && (
        <Prop label={`r=${fmt(node.rotation)}°`} changed={ch("rotation")} />
      )}
      {spanText != null && <Prop label={`"${spanText}"`} changed={false} />}
      {node.font_size != null && <Prop label={`${fmt(node.font_size)}px`} changed={false} />}
      {node.font_family != null && <Prop label={node.font_family} changed={false} />}
      {node.italic && <Prop label="italic" changed={false} />}
      {node.z_level != null && <Prop label={`z=${fmt(node.z_level)}`} changed={ch("z_level")} />}
    </span>
  );
}

interface TreeNodeProps {
  node: TreeNodeData;
  prevNode?: TreeNodeData;
  depth: number;
  selectedNid: number | null;
  onSelect: (nid: number | null) => void;
  isDirectTextChild?: boolean;
}

function TreeNode({
  node,
  prevNode,
  depth,
  selectedNid,
  onSelect,
  isDirectTextChild,
}: TreeNodeProps) {
  const [open, setOpen] = useState(true);
  const hasChildren = (node.children?.length ?? 0) > 0;

  const anyChanged =
    prevNode != null &&
    (
      [
        "width",
        "height",
        "fill_color",
        "stroke_color",
        "stroke_width",
        "alpha",
        "x",
        "y",
        "scale_x",
        "scale_y",
        "rotation",
        "c1_x",
        "c1_y",
        "c2_x",
        "c2_y",
        "z_level",
      ] as const
    ).some((k) => prevNode[k] !== node[k]);

  const iconKind = isDirectTextChild ? "t_line" : node.kind;
  const displayLabel = isDirectTextChild
    ? "Line"
    : node.kind === "layer" && node.layer_name
      ? `layer ${node.layer_name}`
      : (KIND_LABELS[node.kind] ?? node.kind);

  return (
    <div>
      <div
        className={[
          "tree-node",
          selectedNid === node.nid ? "selected" : "",
          anyChanged ? "node-changed" : "",
          node.implicit ? "node-implicit" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        style={{ paddingLeft: 8 + depth * 16 }}
        onClick={() => onSelect(selectedNid === node.nid ? null : node.nid)}
      >
        <span
          className="tree-node-toggle"
          onClick={
            hasChildren
              ? (e) => {
                  e.stopPropagation();
                  setOpen((o: boolean) => !o);
                }
              : undefined
          }
        >
          {hasChildren ? (open ? "▾" : "▸") : ""}
        </span>
        <span className="tree-node-icon">
          <NodeKindIcon kind={iconKind} />
        </span>
        <NodeLabel node={node} prevNode={prevNode} displayLabel={displayLabel} />
      </div>
      {open &&
        node.children?.map((child, i) => (
          <TreeNode
            key={child.id}
            node={child}
            prevNode={prevNode?.children?.[i]}
            depth={depth + 1}
            selectedNid={selectedNid}
            onSelect={onSelect}
            isDirectTextChild={node.kind === "text"}
          />
        ))}
    </div>
  );
}

interface TreeViewProps {
  scene: SceneData | null;
  prevScene: SceneData | null;
  selectedNid: number | null;
  onSelect: (nid: number | null) => void;
}

export default function TreeView({ scene, prevScene, selectedNid, onSelect }: TreeViewProps) {
  const nodes: TreeNodeData[] = scene ? addIds(scene.children) : [];
  const prevNodes: TreeNodeData[] = prevScene ? addIds(prevScene.children) : [];

  return (
    <div className="tree-panel">
      <div className="tree-panel-label">Scene</div>
      {scene ? (
        <>
          <div className="tree-node" style={{ paddingLeft: 8 }}>
            <span className="tree-node-toggle" />
            <span className="tree-node-icon">
              <NodeKindIcon kind="scene" />
            </span>
            <span className="node-label">
              <span className="node-type">scene</span>
              <span className="prop-chip">
                {scene.width}×{scene.height}
              </span>
              <span className="prop-chip">
                <span
                  style={{
                    display: "inline-block",
                    width: 10,
                    height: 10,
                    background: scene.fill_color,
                    border: "1px solid rgba(255,255,255,0.3)",
                    borderRadius: 2,
                    verticalAlign: "middle",
                    marginRight: 3,
                  }}
                />
                {shortHex(scene.fill_color)}
              </span>
            </span>
          </div>
          {nodes.map((n, i) => (
            <TreeNode
              key={n.id}
              node={n}
              prevNode={prevNodes[i]}
              depth={1}
              selectedNid={selectedNid}
              onSelect={onSelect}
            />
          ))}
        </>
      ) : (
        <div className="tree-empty">Run a script to see the scene tree</div>
      )}
    </div>
  );
}

function addIds(nodes: RawNode[] | undefined, prefix = "", _counter = { n: -1 }): TreeNodeData[] {
  return (nodes ?? []).map((n, i) => {
    const {
      id: nid,
      children: rawChildren,
      layers: rawLayers,
      hidden_layers: rawHidden,
      all_svg_layers: allSvgLayers,
      ...rest
    } = n;
    const id = `${prefix}${i}`;
    const children = addIds(rawChildren, `${id}.`, _counter);

    // Explicitly named active layers (have overrides).
    const namedLayerNames = new Set((rawLayers ?? []).map((l: RawImageLayer) => l.layer_name));
    const hiddenLayerNames = new Set(rawHidden ?? []);

    const explicitChildren: TreeNodeData[] = (rawLayers ?? []).map(
      (l: RawImageLayer, li: number) => ({
        kind: "layer" as const,
        id: `${id}.l${li}`,
        nid: l.id,
        layer_name: l.layer_name,
        x: l.x,
        y: l.y,
        width: l.width,
        height: l.height,
        alpha: l.alpha,
        z_level: l.z_level,
        children: [],
      }),
    );

    // Implicit layers: present in the SVG but not explicitly named and not inactive-hidden.
    // Each gets a unique negative nid so clicking one selects only that layer.
    const implicitChildren: TreeNodeData[] = (allSvgLayers ?? [])
      .filter((name) => !namedLayerNames.has(name) && !hiddenLayerNames.has(name))
      .map((name, li) => ({
        kind: "layer" as const,
        id: `${id}.il${li}`,
        nid: _counter.n--,
        layer_name: name,
        children: [],
        implicit: true,
      }));

    return {
      ...rest,
      id,
      nid,
      children: [...children, ...explicitChildren, ...implicitChildren],
    };
  });
}
