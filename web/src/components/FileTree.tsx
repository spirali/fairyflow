import { useState, useEffect } from 'react';

export interface FsEntry {
  name: string;
  is_dir: boolean;
  children?: FsEntry[];
}

function FolderIcon({ open }: { open: boolean }) {
  return open ? (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <path d="M1 4a1 1 0 011-1h4l1.5 1.5H14a1 1 0 011 1v6a1 1 0 01-1 1H2a1 1 0 01-1-1V4z" fill="#e8a040" />
      <path d="M1 6.5h14v5a1 1 0 01-1 1H2a1 1 0 01-1-1V6.5z" fill="#f0b860" />
    </svg>
  ) : (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <path d="M1 4a1 1 0 011-1h4l1.5 1.5H14a1 1 0 011 1v6a1 1 0 01-1 1H2a1 1 0 01-1-1V4z" fill="#c8843a" />
    </svg>
  );
}

function ApyFileIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <rect x="2" y="1" width="9" height="14" rx="1" fill="#1a7a5a" />
      <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#26a87a" />
      <path d="M11 1v3h3" fill="none" stroke="#5dd4aa" strokeWidth="0.5" />
      <text x="3" y="10" fontSize="4.5" fill="#ccf5e8" fontWeight="bold" fontFamily="monospace">apy</text>
    </svg>
  );
}

function PythonFileIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <rect x="2" y="1" width="9" height="14" rx="1" fill="#2b5ba8" />
      <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#3a78d4" />
      <path d="M11 1v3h3" fill="none" stroke="#6aaaf0" strokeWidth="0.5" />
      <text x="4" y="10" fontSize="5" fill="#f0e060" fontWeight="bold" fontFamily="monospace">py</text>
    </svg>
  );
}

function GenericFileIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <rect x="2" y="1" width="9" height="14" rx="1" fill="#5a6070" />
      <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#7a8090" />
      <path d="M11 1v3h3" fill="none" stroke="#9aa0b0" strokeWidth="0.5" />
    </svg>
  );
}

function EntryIcon({ entry, open }: { entry: FsEntry; open: boolean }) {
  if (entry.is_dir) return <FolderIcon open={open} />;
  if (entry.name.endsWith('.apy')) return <ApyFileIcon />;
  if (entry.name.endsWith('.py')) return <PythonFileIcon />;
  return <GenericFileIcon />;
}

interface TreeNodeProps {
  entry: FsEntry;
  path: string;
  depth: number;
  expandedDirs: Set<string>;
  activeFile: string | null;
  onToggleDir: (path: string) => void;
  onFileClick: (path: string) => void;
}

function TreeNode({ entry, path, depth, expandedDirs, activeFile, onToggleDir, onFileClick }: TreeNodeProps) {
  const open = expandedDirs.has(path);
  const isActive = !entry.is_dir && path === activeFile;
  return (
    <div>
      <div
        className={`filetree-entry${entry.is_dir ? ' filetree-dir' : ' filetree-file'}${isActive ? ' filetree-active' : ''}`}
        style={{ paddingLeft: 8 + depth * 14 }}
        onClick={() => entry.is_dir ? onToggleDir(path) : onFileClick(path)}
        role="button"
      >
        {entry.is_dir
          ? <span className="filetree-chevron">{open ? '▾' : '▸'}</span>
          : <span className="filetree-chevron" />
        }
        <span className="filetree-entry-icon">
          <EntryIcon entry={entry} open={open} />
        </span>
        <span className="filetree-entry-name">{entry.name}</span>
      </div>
      {entry.is_dir && open && entry.children?.map(child => (
        <TreeNode
          key={child.name}
          entry={child}
          path={`${path}/${child.name}`}
          depth={depth + 1}
          expandedDirs={expandedDirs}
          activeFile={activeFile}
          onToggleDir={onToggleDir}
          onFileClick={onFileClick}
        />
      ))}
    </div>
  );
}

interface Props {
  activeFile: string | null;
  onFileClick: (path: string) => void;
}

export default function FileTree({ activeFile, onFileClick }: Props) {
  const [tree, setTree] = useState<FsEntry[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());

  useEffect(() => {
    fetch('/ls')
      .then(r => r.json())
      .then(d => setTree(d as FsEntry[]))
      .catch(() => {});
  }, []);

  const toggleDir = (path: string) => {
    setExpandedDirs(prev => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  return (
    <div className="filetree-panel">
      <div className="filetree-header">
        <span className="filetree-header-title">EXPLORER</span>
      </div>
      <div className="filetree-content">
        {tree.map(entry => (
          <TreeNode
            key={entry.name}
            entry={entry}
            path={entry.name}
            depth={0}
            expandedDirs={expandedDirs}
            activeFile={activeFile}
            onToggleDir={toggleDir}
            onFileClick={onFileClick}
          />
        ))}
      </div>
    </div>
  );
}
