// A node in the evaluated scene tree (mirrors renderer::Node serialization).
// Variant-specific fields are flattened into the object by serde.
export interface RawNode {
  id: number;
  kind: 'group' | 'rect' | 'ellipse' | 'path' | 'move' | 'line' | 'cubic';
  // position (group, rect, ellipse, move, line, cubic)
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
