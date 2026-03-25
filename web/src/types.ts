// ── Text subtree types (mirrors renderer::TextChild serialization) ──────────

export interface RawTextSpan {
  id: number;
  text: string;
  fill_color?: string | null;
  stroke_color?: string | null;
  stroke_width?: number;
  alpha?: number;
  font_family?: string;
  font_size?: number;
  italic?: boolean;
}

export interface RawTextGroup {
  id: number;
  children: RawTextChild[];
}

// Externally-tagged by serde: { "Group": {...} } | { "Span": {...} }
export type RawTextChild =
  | { Group: RawTextGroup }
  | { Span: RawTextSpan };

// ── Scene node ───────────────────────────────────────────────────────────────

// A node in the evaluated scene tree (mirrors renderer::Node serialization).
// Variant-specific fields are flattened into the object by serde.
export interface RawNode {
  id: number;
  kind: 'group' | 'rect' | 'ellipse' | 'path' | 'move' | 'line' | 'cubic' | 'text';
  // position (group, rect, ellipse, move, line, cubic, text)
  x?: number;
  y?: number;
  // size (group, rect, ellipse)
  width?: number;
  height?: number;
  // group-specific
  alpha?: number;
  scale_x?: number;
  scale_y?: number;
  rotation?: number;
  // style (rect, ellipse, path)
  fill_color?: string;
  stroke_color?: string;
  stroke_width?: number;
  // cubic control points
  c1_x?: number;
  c1_y?: number;
  c2_x?: number;
  c2_y?: number;
  // children (group nodes and path commands)
  children?: RawNode[];
  // text node lines
  lines?: RawTextChild[];
}

// The scene root returned by GET /tree/{n}
export interface SceneData {
  width: number;
  height: number;
  fill_color: string;
  children: RawNode[];
}

// TreeNodeData adds a stable tree-position string id and keeps the numeric node id.
// 't_line' is a synthetic kind for direct children of a Text node (each represents one line).
// 't_group' / 't_span' mirror the TextGroup / TextSpan types inside those lines.
export interface TreeNodeData extends Omit<RawNode, 'id' | 'children' | 'lines' | 'kind'> {
  id: string;    // tree-position key, e.g. "0.1.2"
  nid: number;   // original numeric node id
  kind: RawNode['kind'] | 't_line' | 't_group' | 't_span';
  children?: TreeNodeData[];
  // extra fields present on t_span (and t_line wrapping a span)
  text?: string;
  font_family?: string;
  font_size?: number;
  italic?: boolean;
}

// Also used for the synthetic scene root row in the tree view
export interface SceneTreeNodeData {
  id: string;
  kind: 'scene';
  width: number;
  height: number;
  fill_color: string;
  children: TreeNodeData[];
}

export interface NodeBounds {
  x: number; y: number; width: number; height: number;
}

export type ConsoleLine = { kind: 'out' | 'err' | 'sys'; text: string };

export type WsStatus = 'connecting' | 'connected' | 'reconnecting' | 'failed';

export type ServerMsg =
  | { type: 'file'; content: string }
  | { type: 'output'; text: string }
  | { type: 'error'; text: string }
  | { type: 'tree'; key_frames: number[]; frame_count: number }
  | { type: 'done'; exit_code: number | null };
