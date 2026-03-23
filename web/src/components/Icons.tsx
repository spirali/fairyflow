interface IconProps {
  size?: number;
}

const STROKE = { fill: 'none', strokeWidth: 1.5, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const };

export function SceneIcon({ size = 16 }: IconProps) {
  const c = '#8B83F0';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" stroke={c} strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
      <rect x="4" y="9" width="24" height="18" rx="2"/>
      <line x1="4" y1="14" x2="28" y2="14"/>
      <line x1="8" y1="9" x2="6" y2="14"/>
      <line x1="13" y1="9" x2="11" y2="14"/>
      <line x1="18" y1="9" x2="16" y2="14"/>
      <line x1="23" y1="9" x2="21" y2="14"/>
      <path d="M13 18.5L13 24.5L20 21.5Z"/>
    </svg>
  );
}

export function NodeIcon({ size = 16 }: IconProps) {
  const c = '#4DA6E8';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" stroke={c} strokeWidth={1.5} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 12 L4 25 Q4 27 6 27 L26 27 Q28 27 28 25 L28 14 Q28 12 26 12 L16 12 L14 10 Q13 8 11 8 L6 8 Q4 8 4 10 Z"/>
    </svg>
  );
}

export function RectIcon({ size = 16 }: IconProps) {
  const c = '#3ECFA0';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <rect x="5" y="9" width="22" height="14" rx="1.5"/>
      <circle cx="5"  cy="9"  r="1.5" fill={c} stroke="none"/>
      <circle cx="27" cy="9"  r="1.5" fill={c} stroke="none"/>
      <circle cx="5"  cy="23" r="1.5" fill={c} stroke="none"/>
      <circle cx="27" cy="23" r="1.5" fill={c} stroke="none"/>
    </svg>
  );
}

export function EllipseIcon({ size = 16 }: IconProps) {
  const c = '#E8714A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <ellipse cx="16" cy="16" rx="13" ry="9"/>
      <circle cx="3"  cy="16" r="1.5" fill={c} stroke="none"/>
      <circle cx="29" cy="16" r="1.5" fill={c} stroke="none"/>
      <circle cx="16" cy="7"  r="1.5" fill={c} stroke="none"/>
      <circle cx="16" cy="25" r="1.5" fill={c} stroke="none"/>
    </svg>
  );
}


export function PathIcon({ size = 16 }: IconProps) {
  const c = '#E8608A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <path d="M5 24 C5 10, 27 10, 27 24"/>
      <rect x="3"  y="22" width="4" height="4" rx="0.5" fill={c} stroke="none"/>
      <rect x="25" y="22" width="4" height="4" rx="0.5" fill={c} stroke="none"/>
      <circle cx="5"  cy="10" r="2"/>
      <circle cx="27" cy="10" r="2"/>
      <line x1="5"  y1="24" x2="5"  y2="10" strokeDasharray="2.5 2"/>
      <line x1="27" y1="24" x2="27" y2="10" strokeDasharray="2.5 2"/>
    </svg>
  );
}

export function MoveToIcon({ size = 16 }: IconProps) {
  const c = '#E8608A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <line x1="8" y1="24" x2="24" y2="8" strokeDasharray="3 2.5"/>
      <circle cx="8" cy="24" r="2.5"/>
      <circle cx="24" cy="8" r="2.5" fill={c} stroke="none"/>
    </svg>
  );
}

export function LineToIcon({ size = 16 }: IconProps) {
  const c = '#E8608A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <line x1="7" y1="25" x2="25" y2="7"/>
      <rect x="5" y="23" width="4" height="4" rx="0.5" fill={c} stroke="none"/>
      <rect x="23" y="5"  width="4" height="4" rx="0.5" fill={c} stroke="none"/>
    </svg>
  );
}

export function CubicToIcon({ size = 16 }: IconProps) {
  const c = '#E8608A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <path d="M6 25 C6 10, 26 22, 26 7"/>
      <rect x="4" y="23" width="4" height="4" rx="0.5" fill={c} stroke="none"/>
      <rect x="24" y="5"  width="4" height="4" rx="0.5" fill={c} stroke="none"/>
      <circle cx="6"  cy="10" r="2"/>
      <circle cx="26" cy="22" r="2"/>
      <line x1="6"  y1="25" x2="6"  y2="10" strokeDasharray="2 2"/>
      <line x1="26" y1="7"  x2="26" y2="22" strokeDasharray="2 2"/>
    </svg>
  );
}

export function ClosePathIcon({ size = 16 }: IconProps) {
  const c = '#E8608A';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <line x1="7"  y1="9"  x2="25" y2="9"/>
      <line x1="25" y1="9"  x2="7"  y2="23"/>
      <line x1="7"  y1="23" x2="25" y2="23"/>
    </svg>
  );
}

export function ImageIcon({ size = 16 }: IconProps) {
  const c = '#95CC35';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <rect x="4" y="6" width="24" height="20" rx="2"/>
      <circle cx="10" cy="12" r="2.5"/>
      <path d="M4 22 L10 15 L16 20 L21 14 L28 22"/>
    </svg>
  );
}

export function LayerIcon({ size = 16 }: IconProps) {
  const c = '#E8A830';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <rect x="8" y="18" width="18" height="9" rx="1.5"/>
      <rect x="6" y="13" width="18" height="9" rx="1.5"/>
      <rect x="4" y="8"  width="18" height="9" rx="1.5"/>
    </svg>
  );
}

export function ImageLayerIcon({ size = 16 }: IconProps) {
  const c = '#5BBFD4';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <path d="M4 8 L4 4 L11 4 L11 8" strokeLinejoin="round"/>
      <rect x="4" y="8" width="24" height="18" rx="2"/>
      <path d="M7 22 L12 15 L17 20 L21 14 L28 22"/>
    </svg>
  );
}

export function TextIcon({ size = 16 }: IconProps) {
  const c = '#7EC832';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <line x1="5"  y1="8" x2="27" y2="8"/>
      <line x1="5"  y1="6" x2="5"  y2="10"/>
      <line x1="27" y1="6" x2="27" y2="10"/>
      <line x1="16" y1="8" x2="16" y2="26"/>
      <line x1="10" y1="26" x2="22" y2="26"/>
    </svg>
  );
}

export function TextLineIcon({ size = 16 }: IconProps) {
  const c = '#7EC832';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <line x1="5" y1="22" x2="27" y2="22"/>
      {/* small T */}
      <line x1="12" y1="8" x2="20" y2="8"/>
      <line x1="16" y1="8" x2="16" y2="18"/>
    </svg>
  );
}

export function TextGroupIcon({ size = 16 }: IconProps) {
  const c = '#7EC832';
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" {...STROKE} stroke={c}>
      <rect x="4" y="5" width="24" height="22" rx="2"/>
      {/* small T */}
      <line x1="12" y1="10" x2="20" y2="10"/>
      <line x1="16" y1="10" x2="16" y2="22"/>
    </svg>
  );
}

const KIND_ICONS: Record<string, ({ size }: IconProps) => React.ReactElement> = {
  scene:   SceneIcon,
  group:   NodeIcon,
  rect:    RectIcon,
  ellipse: EllipseIcon,
  path:    PathIcon,
  move:    MoveToIcon,
  line:    LineToIcon,
  cubic:   CubicToIcon,
  close:   ClosePathIcon,
  image:   ImageIcon,
  layer:   ImageLayerIcon,
  text:    TextIcon,
  t_line:  TextLineIcon,
  t_group: TextGroupIcon,
  t_span:  TextIcon,
};

export function NodeKindIcon({ kind, size = 16 }: { kind: string; size?: number }) {
  const Icon = KIND_ICONS[kind];
  if (!Icon) return <span style={{ width: size, height: size, display: 'inline-block' }}>◆</span>;
  return <Icon size={size} />;
}
