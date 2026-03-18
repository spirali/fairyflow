export interface RawNode {
  kind: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  fill_color?: string;
  stroke_color?: string;
  stroke_width?: number;
  alpha?: number;
  radius?: number;
  scale_x?: number;
  scale_y?: number;
  c1_x?: number;
  c1_y?: number;
  c2_x?: number;
  c2_y?: number;
  children?: RawNode[];
}

export interface TreeNodeData extends Omit<RawNode, 'children'> {
  id: string;
  children?: TreeNodeData[];
}

export type ConsoleLine = { kind: 'out' | 'err' | 'sys'; text: string };

export type WsStatus = 'connecting' | 'connected' | 'reconnecting' | 'failed';

export type ServerMsg =
  | { type: 'file'; content: string }
  | { type: 'output'; text: string }
  | { type: 'error'; text: string }
  | { type: 'tree'; frames: RawNode[]; key_frames: number[] }
  | { type: 'done'; exit_code: number | null };
