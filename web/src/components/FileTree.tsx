import { useState, useEffect, useRef } from 'react';
import { withToken } from '../auth';

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

function FfpyFileIcon() {
  const c = '#8B83F0';
  return (
    <svg width="16" height="16" viewBox="0 0 32 32" fill="none" stroke={c} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
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

function FfsqFileIcon() {
  const c = '#60c0a0';
  return (
    <svg width="16" height="16" viewBox="0 0 32 32" fill="none" stroke={c} strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="4" y="6" width="24" height="20" rx="2"/>
      <line x1="4" y1="12" x2="28" y2="12"/>
      <line x1="4" y1="18" x2="28" y2="18"/>
      <line x1="4" y1="24" x2="28" y2="24"/>
      <line x1="10" y1="6" x2="10" y2="12"/>
      <line x1="16" y1="6" x2="16" y2="12"/>
      <line x1="22" y1="6" x2="22" y2="12"/>
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

function FairyFlowTomlIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="6.5" stroke="#c09040" strokeWidth="1.2" />
      <circle cx="8" cy="8" r="2.2" fill="#c09040" />
      <circle cx="8" cy="1.5" r="1.2" fill="#c09040" />
      <circle cx="8" cy="14.5" r="1.2" fill="#c09040" />
      <circle cx="1.5" cy="8" r="1.2" fill="#c09040" />
      <circle cx="14.5" cy="8" r="1.2" fill="#c09040" />
      <circle cx="3.4" cy="3.4" r="1.2" fill="#c09040" />
      <circle cx="12.6" cy="12.6" r="1.2" fill="#c09040" />
      <circle cx="12.6" cy="3.4" r="1.2" fill="#c09040" />
      <circle cx="3.4" cy="12.6" r="1.2" fill="#c09040" />
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
  if (entry.name === 'fairyflow.toml') return <FairyFlowTomlIcon />;
  if (entry.name.endsWith('.ffpy')) return <FfpyFileIcon />;
  if (entry.name.endsWith('.ffsq')) return <FfsqFileIcon />;
  if (entry.name.endsWith('.py')) return <PythonFileIcon />;
  return <GenericFileIcon />;
}

interface TreeNodeProps {
  entry: FsEntry;
  path: string;
  depth: number;
  expandedDirs: Set<string>;
  activeFile: string | null;
  contextPath: string | null;
  onToggleDir: (path: string) => void;
  onFileClick: (path: string) => void;
  onSelectContext: (path: string, isDir: boolean) => void;
}

function TreeNode({ entry, path, depth, expandedDirs, activeFile, contextPath, onToggleDir, onFileClick, onSelectContext }: TreeNodeProps) {
  const open = expandedDirs.has(path);
  const isActive = !entry.is_dir && path === activeFile;
  const isContext = path === contextPath;
  return (
    <div>
      <div
        className={`filetree-entry${entry.is_dir ? ' filetree-dir' : ' filetree-file'}${isActive ? ' filetree-active' : ''}${isContext ? ' filetree-context' : ''}`}
        style={{ paddingLeft: 8 + depth * 14 }}
        draggable={!entry.is_dir && entry.name.endsWith('.ffpy')}
        onDragStart={!entry.is_dir && entry.name.endsWith('.ffpy') ? (e) => {
          e.dataTransfer.setData('application/ffpy-path', path);
          e.dataTransfer.effectAllowed = 'copy';
        } : undefined}
        onClick={() => {
          if (entry.is_dir) {
            onToggleDir(path);
            onSelectContext(path, true);
          } else {
            onFileClick(path);
            onSelectContext(path, false);
          }
        }}
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
          contextPath={contextPath}
          onToggleDir={onToggleDir}
          onFileClick={onFileClick}
          onSelectContext={onSelectContext}
        />
      ))}
    </div>
  );
}

type DialogKind = 'ffpy' | 'ffsq' | 'file' | 'dir';

const DIALOG_LABELS: Record<DialogKind, { title: string; placeholder: string; ext?: string }> = {
  ffpy: { title: 'New Scene File', placeholder: 'filename.ffpy', ext: '.ffpy' },
  ffsq: { title: 'New Sequence File', placeholder: 'filename.ffsq', ext: '.ffsq' },
  file: { title: 'New File', placeholder: 'filename' },
  dir: { title: 'New Directory', placeholder: 'directory name' },
};

interface Props {
  activeFile: string | null;
  onFileClick: (path: string) => void;
  refreshTrigger?: number;
}

export default function FileTree({ activeFile, onFileClick, refreshTrigger }: Props) {
  const [tree, setTree] = useState<FsEntry[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [contextItem, setContextItem] = useState<{ path: string; isDir: boolean } | null>(null);
  const [dialog, setDialog] = useState<DialogKind | null>(null);
  const [inputName, setInputName] = useState('');
  const [createError, setCreateError] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const fetchTree = () => {
    fetch(withToken('/ls'))
      .then(r => r.json())
      .then(d => setTree(d as FsEntry[]))
      .catch(() => {});
  };

  useEffect(() => { fetchTree(); }, []);
  useEffect(() => { if (refreshTrigger) fetchTree(); }, [refreshTrigger]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (dialog !== null) {
      setInputName('');
      setCreateError('');
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [dialog]);

  const toggleDir = (path: string) => {
    setExpandedDirs(prev => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const selectContext = (path: string, isDir: boolean) => {
    setContextItem({ path, isDir });
  };

  function getTargetDir(): string {
    const item = contextItem ?? (activeFile ? { path: activeFile, isDir: false } : null);
    if (!item) return '.';
    if (item.isDir) return item.path;
    const idx = item.path.lastIndexOf('/');
    return idx >= 0 ? item.path.slice(0, idx) : '.';
  }

  async function handleCreate() {
    let name = inputName.trim();
    if (!name) { setCreateError('Name is required'); return; }
    if (name.includes('/') || name.includes('\\')) { setCreateError('Name cannot contain slashes'); return; }

    if (dialog !== 'dir') {
      const ext = DIALOG_LABELS[dialog!].ext;
      if (ext && !name.endsWith(ext)) name = name + ext;
    }

    const dir = getTargetDir();
    const endpoint = dialog === 'dir' ? '/new-dir' : '/new-file';

    try {
      const res = await fetch(withToken(endpoint), {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ dir, name }),
      });
      if (res.status === 204) {
        setDialog(null);
        fetchTree();
      } else {
        const text = await res.text();
        setCreateError(text || 'Failed to create');
      }
    } catch {
      setCreateError('Network error');
    }
  }

  const contextPath = contextItem?.path ?? null;

  return (
    <div className="filetree-panel">
      <div className="filetree-header">
        <span className="filetree-header-title">EXPLORER</span>
        <div className="filetree-header-actions">
          <button className="filetree-action-btn" title="New Scene File (.ffpy)" onClick={() => setDialog('ffpy')}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="2" y="1" width="9" height="14" rx="1" fill="#8B83F0" opacity="0.7"/>
              <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#8B83F0"/>
              <path d="M11 1v3h3" fill="none" stroke="#c0bcf8" strokeWidth="0.5"/>
              <line x1="8" y1="11" x2="8" y2="15" stroke="white" strokeWidth="1.4"/>
              <line x1="6" y1="13" x2="10" y2="13" stroke="white" strokeWidth="1.4"/>
            </svg>
          </button>
          <button className="filetree-action-btn" title="New Sequence File (.ffsq)" onClick={() => setDialog('ffsq')}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="2" y="1" width="9" height="14" rx="1" fill="#60c0a0" opacity="0.7"/>
              <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#60c0a0"/>
              <path d="M11 1v3h3" fill="none" stroke="#a0e0c8" strokeWidth="0.5"/>
              <line x1="8" y1="11" x2="8" y2="15" stroke="white" strokeWidth="1.4"/>
              <line x1="6" y1="13" x2="10" y2="13" stroke="white" strokeWidth="1.4"/>
            </svg>
          </button>
          <button className="filetree-action-btn" title="New File" onClick={() => setDialog('file')}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="2" y="1" width="9" height="14" rx="1" fill="#5a6070" opacity="0.7"/>
              <path d="M11 1l3 3v11H5v-1h8V4l-2-3z" fill="#7a8090"/>
              <path d="M11 1v3h3" fill="none" stroke="#9aa0b0" strokeWidth="0.5"/>
              <line x1="8" y1="11" x2="8" y2="15" stroke="white" strokeWidth="1.4"/>
              <line x1="6" y1="13" x2="10" y2="13" stroke="white" strokeWidth="1.4"/>
            </svg>
          </button>
          <button className="filetree-action-btn" title="New Directory" onClick={() => setDialog('dir')}>
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M1 4a1 1 0 011-1h4l1.5 1.5H14a1 1 0 011 1v6a1 1 0 01-1 1H2a1 1 0 01-1-1V4z" fill="#c8843a" opacity="0.8"/>
              <line x1="8" y1="10" x2="8" y2="14" stroke="white" strokeWidth="1.4"/>
              <line x1="6" y1="12" x2="10" y2="12" stroke="white" strokeWidth="1.4"/>
            </svg>
          </button>
        </div>
      </div>

      {dialog !== null && (
        <div className="filetree-dialog">
          <div className="filetree-dialog-title">{DIALOG_LABELS[dialog].title}</div>
          <div className="filetree-dialog-dir">in: {getTargetDir()}</div>
          <input
            ref={inputRef}
            className="filetree-dialog-input"
            type="text"
            placeholder={DIALOG_LABELS[dialog].placeholder}
            value={inputName}
            onChange={e => { setInputName(e.target.value); setCreateError(''); }}
            onKeyDown={e => {
              if (e.key === 'Enter') handleCreate();
              if (e.key === 'Escape') setDialog(null);
            }}
          />
          {createError && <div className="filetree-dialog-error">{createError}</div>}
          <div className="filetree-dialog-buttons">
            <button className="filetree-dialog-btn filetree-dialog-btn-ok" onClick={handleCreate}>Create</button>
            <button className="filetree-dialog-btn filetree-dialog-btn-cancel" onClick={() => setDialog(null)}>Cancel</button>
          </div>
        </div>
      )}

      <div className="filetree-content">
        {tree.map(entry => (
          <TreeNode
            key={entry.name}
            entry={entry}
            path={entry.name}
            depth={0}
            expandedDirs={expandedDirs}
            activeFile={activeFile}
            contextPath={contextPath}
            onToggleDir={toggleDir}
            onFileClick={onFileClick}
            onSelectContext={selectContext}
          />
        ))}
      </div>
    </div>
  );
}
