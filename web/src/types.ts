// ── Scene node ───────────────────────────────────────────────────────────────

// A node in the evaluated scene tree (mirrors renderer::Node serialization).
// Variant-specific fields are flattened into the object by serde.
// Text children (t_group / t_span) are serialized as regular child nodes.
export interface RawImageLayer {
  id: number;
  layer_name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  alpha: number;
  z_level?: number;
}

export interface RawNode {
  id: number;
  kind: 'group' | 'rect' | 'ellipse' | 'path' | 'move' | 'line' | 'cubic' | 'text' | 't_group' | 't_span' | 'image' | 'layer';
  // position (group, rect, ellipse, move, line, cubic, text, image, layer)
  x?: number;
  y?: number;
  // size (group, rect, ellipse, image, layer)
  width?: number;
  height?: number;
  // group-specific
  alpha?: number;
  scale_x?: number;
  scale_y?: number;
  rotation?: number;
  // style (rect, ellipse, path, t_span)
  fill_color?: string;
  stroke_color?: string;
  stroke_width?: number;
  // cubic control points
  c1_x?: number;
  c1_y?: number;
  c2_x?: number;
  c2_y?: number;
  // z-level (group, rect, ellipse, path, text, image, layer) — absent when inherited
  z_level?: number;
  // children (group, t_group, text lines)
  children?: RawNode[];
  // t_span fields
  text?: string;
  font_family?: string;
  font_size?: number;
  italic?: boolean;
  // layer node
  layer_name?: string;
  // image node
  layers?: RawImageLayer[];
  hidden_layers?: string[];
  all_svg_layers?: string[];
}

// The scene root returned by GET /tree/{n}
export interface SceneData {
  width: number;
  height: number;
  fill_color: string;
  children: RawNode[];
}

// TreeNodeData adds a stable tree-position string id and keeps the numeric node id.
export interface TreeNodeData extends Omit<RawNode, 'id' | 'children'> {
  id: string;    // tree-position key, e.g. "0.1.2"
  nid: number;   // original numeric node id
  children?: TreeNodeData[];
  /** True for SVG layers that exist in the image but aren't explicitly named in the script. */
  implicit?: boolean;
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

export interface StackFrame {
  file: string;
  line: number;
}

export interface InfoEntry {
  id: number;
  stack: StackFrame[];
}

export interface SceneInfo {
  name: string;
  key_frames: number[];
  cue_frames: number[];
  frame_count: number;
  info: InfoEntry[];
}

export type ServerMsg =
  | { type: 'config'; fps: number }
  | { type: 'file'; content: string }
  | { type: 'output'; text: string }
  | { type: 'error'; text: string }
  | { type: 'tree'; key_frames: number[]; cue_frames: number[]; frame_count: number; scenes: SceneInfo[] }
  | { type: 'done'; exit_code: number | null };

// ── Sequence types ────────────────────────────────────────────────────────────

export interface SequenceSceneResult {
  path: string;
  frameCount: number;
  cueFrames: number[];
  frames: string[];   // blob URLs, index = local frame number within this scene
  width: number;
  height: number;
}

export interface SequenceRenderResult {
  scenes: SequenceSceneResult[];
  totalFrames: number;
  globalCueFrames: number[];  // in global frame coordinates; always includes 0 and last frame
}
