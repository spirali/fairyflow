import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { setToken, loadToken, withToken } from './auth';
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle, usePanelRef } from 'react-resizable-panels';
import type { PanelSize } from 'react-resizable-panels';
import Editor from '@monaco-editor/react';
import type { OnMount, Monaco } from '@monaco-editor/react';
import MenuBar from './components/MenuBar';
import TreeView from './components/TreeView';
import FileTree from './components/FileTree';
import SequenceEditor from './components/SequenceEditor';
import SequencePlayer from './components/SequencePlayer';
import type { ConsoleLine, InfoEntry, NodeBounds, SceneData, SceneInfo, ServerMsg, SequenceRenderResult, WsStatus } from './types';
import './App.css';

type MonacoEditor = Parameters<OnMount>[0];
type ViewState = ReturnType<MonacoEditor['saveViewState']>;

interface Tab { path: string; isDirty: boolean; isFfsq?: boolean }

interface RenderedFrameResponse { n: number; png: string }

export default function App() {
  // ── auth ──────────────────────────────────────────────────────────────────
  const [authStatus, setAuthStatus] = useState<'pending' | 'ok' | 'error'>('pending');

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const urlToken = params.get('token');
    if (urlToken) {
      setToken(urlToken);
      params.delete('token');
      const newSearch = params.toString();
      history.replaceState({}, '', window.location.pathname + (newSearch ? `?${newSearch}` : ''));
    } else {
      loadToken();
    }
    const tok = loadToken();
    if (!tok) { setAuthStatus('error'); return; }
    fetch(withToken('/ls'))
      .then(r => { setAuthStatus(r.status === 401 ? 'error' : 'ok'); })
      .catch(() => setAuthStatus('error'));
  }, []);

  // ── scene state ──────────────────────────────────────────────────────────
  const [sceneData, setSceneData] = useState<SceneData | null>(null);
  const [prevSceneData, setPrevSceneData] = useState<SceneData | null>(null);
  const [frames, setFrames] = useState(1);
  const [keyFrames, setKeyFrames] = useState<number[]>([]);
  const [cueFrames, setCueFrames] = useState<number[]>([]);
  const [frame, setFrame_] = useState(0);
  const frameRef = useRef(0);
  const pendingFrameRef = useRef<number | null>(null);
  const setFrame = (f: number | ((prev: number) => number)) => {
    const next = typeof f === 'function' ? f(frameRef.current) : f;
    frameRef.current = next;
    setFrame_(next);
  };
  const [runId, setRunId] = useState(0);
  const [scenes, setScenes_] = useState<SceneInfo[]>([]);
  const scenesRef = useRef<SceneInfo[]>([]);
  const setScenes = (sc: SceneInfo[]) => { scenesRef.current = sc; setScenes_(sc); };
  const [selectedScene, setSelectedScene_] = useState<number | 'all'>('all');
  const selectedSceneRef = useRef<number | 'all'>('all');
  const setSelectedScene = (v: number | 'all') => { selectedSceneRef.current = v; setSelectedScene_(v); };

  // ── canvas layout ─────────────────────────────────────────────────────────
  interface CanvasLayout { serverScale: number; cssWidth: number; cssHeight: number; pngWidth: number; pngHeight: number }
  const [canvasLayout, setCanvasLayout] = useState<CanvasLayout | null>(null);
  const canvasContentRef = useRef<HTMLDivElement | null>(null);

  // ── playback ──────────────────────────────────────────────────────────────
  const [fps, setFps] = useState(24);
  const [stopOnCue, setStopOnCue] = useState(true);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isPrefetching, setIsPrefetching] = useState(false);
  const imageCacheRef = useRef<Map<string, string>>(new Map());
  const treeCacheRef = useRef<Map<string, SceneData>>(new Map());
  const [, setCacheVersion] = useState(0); // incremented to trigger re-render after caching
  const playIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const playReturnFrameRef = useRef(0);
  const cancelledRef = useRef(false);

  // ── video export ─────────────────────────────────────────────────────────
  const [exportMenuOpen, setExportMenuOpen] = useState(false);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const exportMenuRef = useRef<HTMLDivElement | null>(null);
  const [exportWidth, setExportWidth] = useState(0);
  const [exportHeight, setExportHeight] = useState(0);
  const [exportFps, setExportFps] = useState(24);
  const [exportCodec, setExportCodec] = useState<'h264' | 'h265' | 'vp9'>('h264');
  const [exportCrf, setExportCrf] = useState(23);
  const [exportFilename, setExportFilename] = useState('animation.mp4');
  const [exportFromFrame, setExportFromFrame] = useState(0);
  const [exportToFrame, setExportToFrame] = useState(0);
  const [exportStatus, setExportStatus] = useState<'idle' | 'exporting' | 'done' | 'error'>('idle');
  const [exportProgress, setExportProgress] = useState(0);
  const [exportMessage, setExportMessage] = useState('');
  const [exportDonePath, setExportDonePath] = useState('');
  const [exportErrorMsg, setExportErrorMsg] = useState('');

  // ── sequence ──────────────────────────────────────────────────────────────
  const [seqRenderResults, setSeqRenderResults] = useState<Map<string, SequenceRenderResult>>(new Map());
  const wsMessageOverrideRef = useRef<((msg: ServerMsg) => void) | null>(null);
  // Stable function — must not be recreated on re-renders so SequenceEditor's
  // cleanup effect does not clear the override on every state update.
  const setWsOverride = useCallback((fn: ((msg: ServerMsg) => void) | null) => {
    wsMessageOverrideRef.current = fn;
  }, []);
  const seqCanvasAreaRef = useRef<HTMLDivElement>(null);
  const getSeqPlayerAreaSize = useCallback(() => {
    const el = seqCanvasAreaRef.current;
    if (!el) return null;
    const { width, height } = el.getBoundingClientRect();
    return width > 0 && height > 0 ? { width, height } : null;
  }, []);
  const [seqRenderTrigger, setSeqRenderTrigger] = useState(0);
  const handleSeqRerender = useCallback(() => setSeqRenderTrigger(t => t + 1), []);

  // ── node selection ────────────────────────────────────────────────────────
  const [selectedNid, setSelectedNid] = useState<number | null>(null);
  const [nodeBounds, setNodeBounds] = useState<NodeBounds | null>(null);

  // ── tabs ─────────────────────────────────────────────────────────────────
  const [tabs, setTabsState] = useState<Tab[]>([]);
  const [activeTab, setActiveTabState] = useState(-1);
  const tabsRef = useRef<Tab[]>([]);
  const activeTabRef = useRef(-1);
  const currentFileRef = useRef<string | null>(null);
  const modelsRef = useRef<Map<string, ReturnType<Monaco['editor']['createModel']>>>(new Map());
  const viewStatesRef = useRef<Map<string, ViewState>>(new Map());

  const setTabs = (t: Tab[]) => { tabsRef.current = t; setTabsState(t); };
  const setActiveTab = (i: number) => {
    activeTabRef.current = i;
    currentFileRef.current = tabsRef.current[i]?.path ?? null;
    setActiveTabState(i);
  };
  const currentFile = tabs[activeTab]?.path ?? null;

  // ── file tree panel ───────────────────────────────────────────────────────
  const fileTreePanelRef = usePanelRef();
  const [fileTreeCollapsed, setFileTreeCollapsed] = useState(false);
  const [fileTreeRefresh, setFileTreeRefresh] = useState(0);
  const refreshFileTree = () => setFileTreeRefresh(n => n + 1);
  const [sidebarMenuOpen, setSidebarMenuOpen] = useState(false);
  const sidebarMenuRef = useRef<HTMLDivElement>(null);

  const handleFileTreeResize = (_size: PanelSize) => {
    setFileTreeCollapsed(fileTreePanelRef.current?.isCollapsed() ?? false);
  };

  const toggleFileTree = () => {
    const panel = fileTreePanelRef.current;
    if (!panel) return;
    if (panel.isCollapsed()) panel.expand();
    else panel.collapse();
  };

  const switchToTab = (idx: number) => {
    const editor = editorRef.current;
    if (!editor) return;
    const curIdx = activeTabRef.current;
    if (curIdx >= 0 && tabsRef.current[curIdx]) {
      viewStatesRef.current.set(tabsRef.current[curIdx].path, editor.saveViewState());
    }
    const newPath = tabsRef.current[idx]?.path;
    if (!newPath) return;
    if (tabsRef.current[idx]?.isFfsq) {
      editor.setModel(null);
    } else {
      const model = modelsRef.current.get(newPath);
      if (model) {
        editor.setModel(model);
        const vs = viewStatesRef.current.get(newPath);
        if (vs) editor.restoreViewState(vs);
        editor.focus();
      }
    }
    setActiveTab(idx);
  };

  const handleFileClick = (path: string) => {
    const existing = tabsRef.current.findIndex(t => t.path === path);
    if (existing !== -1) { switchToTab(existing); return; }

    if (path.endsWith('.ffsq')) {
      const editor = editorRef.current;
      const curIdx = activeTabRef.current;
      if (editor && curIdx >= 0 && tabsRef.current[curIdx]) {
        viewStatesRef.current.set(tabsRef.current[curIdx].path, editor.saveViewState());
      }
      const newTabs = [...tabsRef.current, { path, isDirty: false, isFfsq: true }];
      setTabs(newTabs);
      const newIdx = newTabs.length - 1;
      activeTabRef.current = newIdx;
      currentFileRef.current = path;
      setActiveTabState(newIdx);
      editor?.setModel(null);
      return;
    }

    fetch(withToken(`/file?path=${encodeURIComponent(path)}`))
      .then(r => r.ok ? r.text() : null)
      .then(content => {
        if (content === null) return;
        const monaco = monacoRef.current;
        const editor = editorRef.current;
        if (!monaco || !editor) return;
        const curIdx = activeTabRef.current;
        if (curIdx >= 0 && tabsRef.current[curIdx]) {
          viewStatesRef.current.set(tabsRef.current[curIdx].path, editor.saveViewState());
        }
        const lang = path.endsWith('.ffpy') ? 'python' : undefined;
        const model = monaco.editor.createModel(content, lang, monaco.Uri.file(path));
        modelsRef.current.set(path, model);
        const newTabs = [...tabsRef.current, { path, isDirty: false }];
        setTabs(newTabs);
        const newIdx = newTabs.length - 1;
        activeTabRef.current = newIdx;
        currentFileRef.current = path;
        setActiveTabState(newIdx);
        editor.setModel(model);
        editor.focus();
      })
      .catch(() => {});
  };

  const handleTabClick = (idx: number) => {
    if (idx !== activeTabRef.current) switchToTab(idx);
  };

  const handleCloseTab = (idx: number) => {
    const tab = tabsRef.current[idx];
    if (!tab.isFfsq) {
      modelsRef.current.get(tab.path)?.dispose();
      modelsRef.current.delete(tab.path);
    }
    viewStatesRef.current.delete(tab.path);
    const newTabs = tabsRef.current.filter((_, i) => i !== idx);
    tabsRef.current = newTabs;
    setTabsState(newTabs);
    if (newTabs.length === 0) {
      activeTabRef.current = -1;
      currentFileRef.current = null;
      setActiveTabState(-1);
    } else {
      const newIdx = Math.min(idx, newTabs.length - 1);
      const newPath = newTabs[newIdx].path;
      activeTabRef.current = newIdx;
      currentFileRef.current = newPath;
      setActiveTabState(newIdx);
      const editor = editorRef.current;
      if (editor) {
        const model = modelsRef.current.get(newPath);
        if (model) {
          editor.setModel(model);
          const vs = viewStatesRef.current.get(newPath);
          if (vs) editor.restoreViewState(vs);
        }
      }
    }
  };

  const markActiveTabClean = () => {
    const idx = activeTabRef.current;
    if (idx < 0) return;
    setTabs(tabsRef.current.map((t, i) => i === idx ? { ...t, isDirty: false } : t));
  };

  useEffect(() => {
    if (!sidebarMenuOpen) return;
    const handler = (e: MouseEvent) => {
      if (sidebarMenuRef.current && !sidebarMenuRef.current.contains(e.target as Node)) {
        setSidebarMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [sidebarMenuOpen]);

  // ── editor / console ─────────────────────────────────────────────────────
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const [running, setRunning] = useState(false);
  const [wsStatus, setWsStatus] = useState<WsStatus>('connecting');
  const wsRef = useRef<WebSocket | null>(null);
  const editorRef = useRef<MonacoEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const decorationsRef = useRef<string[]>([]);
  const consoleEndRef = useRef<HTMLDivElement | null>(null);
  const reconnectCount = useRef(0);

  // ── scene selection ───────────────────────────────────────────────────────
  const sceneParam = selectedScene === 'all' ? '' : `&scene=${selectedScene}`;
  const sceneCacheKey = selectedScene === 'all' ? 'all' : String(selectedScene);

  // ── derived ───────────────────────────────────────────────────────────────
  const maxFrame = frames - 1;
  const keyFrameSet = new Set(keyFrames);
  const cueFrameSet = new Set(cueFrames);
  const prevKeyFrame: number | null = keyFrames.filter(kf => kf < frame).at(-1) ?? null;
  const nextKeyFrame: number | null = keyFrames.find(kf => kf > frame) ?? null;
  const navCueFrames = cueFrames[0] === 0 ? cueFrames : [0, ...cueFrames];
  const prevCueFrame: number | null = navCueFrames.filter(cf => cf < frame).at(-1) ?? null;
  const nextCueFrame: number | null = navCueFrames.find(cf => cf > frame) ?? null;
  const hasScene = sceneData != null;
  const sceneWidth  = sceneData?.width  ?? null;
  const sceneHeight = sceneData?.height ?? null;

  // ── node info map (id → stack) built from all scenes' info arrays ─────────
  const nodeInfoMap = useMemo(() => {
    const map = new Map<number, InfoEntry>();
    for (const s of scenes) {
      for (const entry of (s.info ?? [])) {
        map.set(entry.id, entry);
      }
    }
    return map;
  }, [scenes]);

  // ── websocket ─────────────────────────────────────────────────────────────
  useEffect(() => {
    if (authStatus !== 'ok') return;
    let dead = false;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    const MAX_RETRIES = 3;

    function connect() {
      const ws = new WebSocket(withToken(`ws://${window.location.host}/ws`));

      ws.onopen = () => {
        if (dead) { ws.close(); return; }
        reconnectCount.current = 0;
        setWsStatus('connected');
        wsRef.current = ws;
      };

      ws.onmessage = (e: MessageEvent<string>) => {
        const msg = JSON.parse(e.data) as ServerMsg;
        if (wsMessageOverrideRef.current) {
          wsMessageOverrideRef.current(msg);
          return;
        }
        if (msg.type === 'config') {
          setFps(msg.fps);
        } else if (msg.type === 'output') {
          setLines((prev) => [...prev, { kind: 'out', text: msg.text }]);
        } else if (msg.type === 'error') {
          setLines((prev) => [...prev, { kind: 'err', text: msg.text }]);
        } else if (msg.type === 'tree') {
          const sc = msg.scenes ?? [];
          setScenes(sc);
          const prevSel = selectedSceneRef.current;
          const prevName = typeof prevSel === 'number' ? scenesRef.current[prevSel]?.name : null;
          const restoredIdx = prevName != null ? sc.findIndex(s => s.name === prevName) : -1;
          pendingFrameRef.current = frameRef.current < msg.frame_count ? frameRef.current : 0;
          setSelectedScene(restoredIdx >= 0 ? restoredIdx : 'all');
          setFrames(msg.frame_count);
          setKeyFrames(msg.key_frames ?? []);
          setCueFrames(msg.cue_frames ?? []);
          setRunId(id => id + 1);
        } else if (msg.type === 'done') {
          setRunning(false);
          const label =
            msg.exit_code === 0 ? 'Process finished successfully'
            : msg.exit_code != null ? `Process exited with code ${msg.exit_code}`
            : 'Process terminated';
          setLines((prev) => [...prev, { kind: 'sys', text: label }]);
        }
      };

      ws.onclose = () => {
        if (dead) return;
        wsRef.current = null;
        setRunning(false);
        reconnectCount.current += 1;
        if (reconnectCount.current > MAX_RETRIES) {
          setWsStatus('failed');
        } else {
          setWsStatus('reconnecting');
          retryTimer = setTimeout(connect, 2000);
        }
      };
    }

    connect();
    return () => {
      dead = true;
      if (retryTimer !== null) clearTimeout(retryTimer);
      wsRef.current?.close();
    };
  }, [authStatus]);

  // ── auto-scroll console ────────────────────────────────────────────────────
  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  // ── cache + selection clear when a new run arrives ─────────────────────────
  // sceneData/prevSceneData are intentionally NOT cleared here so the right
  // panel keeps showing the previous content (grayed out) during evaluation.
  useEffect(() => {
    const imgCache = imageCacheRef.current;
    imgCache.forEach(url => URL.revokeObjectURL(url));
    imgCache.clear();
    treeCacheRef.current.clear();
    setCacheVersion(0);
    setSelectedNid(null);
    setNodeBounds(null);
  }, [runId]);

  // ── update frames/keyframes and clear caches when scene selection changes ──
  useEffect(() => {
    const imgCache = imageCacheRef.current;
    imgCache.forEach(url => URL.revokeObjectURL(url));
    imgCache.clear();
    treeCacheRef.current.clear();
    setCacheVersion(0);
    const targetFrame = pendingFrameRef.current ?? 0;
    pendingFrameRef.current = null;
    setFrame(targetFrame);
    if (selectedScene === 'all') {
      // Recompute from all scenes combined
      if (scenes.length > 0) {
        let offset = 0;
        const allKf: number[] = [];
        const allCf: number[] = [];
        for (const s of scenes) {
          for (const kf of s.key_frames) allKf.push(kf + offset);
          for (const cf of (s.cue_frames ?? [])) allCf.push(cf + offset);
          offset += s.frame_count;
        }
        allKf.sort((a, b) => a - b);
        allCf.sort((a, b) => a - b);
        setKeyFrames([...new Set(allKf)]);
        setCueFrames([...new Set(allCf)]);
        setFrames(offset);
      }
    } else {
      const s = scenes[selectedScene as number];
      if (s) {
        setKeyFrames(s.key_frames);
        setCueFrames(s.cue_frames ?? []);
        setFrames(s.frame_count);
      }
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedScene, runId]);

  // ── fetch scene tree on demand (with cache) ────────────────────────────────
  useEffect(() => {
    if (frames <= 0) return;
    const ctrl = new AbortController();

    const fetchTree = async (n: number): Promise<SceneData | null> => {
      const key = `${runId}-${sceneCacheKey}-${n}`;
      const cached = treeCacheRef.current.get(key);
      if (cached) return cached;
      const r = await fetch(withToken(`/tree/${n}?v=${runId}${sceneParam}`), { signal: ctrl.signal });
      if (!r.ok) return null;
      const data = await r.json() as SceneData;
      treeCacheRef.current.set(key, data);
      return data;
    };

    Promise.all([
      fetchTree(frame),
      frame > 0 ? fetchTree(frame - 1) : Promise.resolve(null),
    ]).then(([curr, prev]) => {
      setSceneData(curr);
      setPrevSceneData(prev);
    }).catch(() => {});

    return () => ctrl.abort();
  }, [frame, runId, frames]);

  // ── per-frame image caching ────────────────────────────────────────────────
  useEffect(() => {
    if (!canvasLayout || !hasScene) return;
    const sk = canvasLayout.serverScale.toFixed(3);
    const key = cacheKey(frame, sk);
    if (imageCacheRef.current.has(key)) return; // already cached

    const ctrl = new AbortController();
    fetch(withToken(`/frame/${frame}?scale=${sk}&v=${runId}${sceneParam}`), { signal: ctrl.signal })
      .then(r => r.ok ? r.blob() : null)
      .then(blob => {
        if (!blob || ctrl.signal.aborted) return;
        if (imageCacheRef.current.has(key)) return;
        imageCacheRef.current.set(key, URL.createObjectURL(blob));
        setCacheVersion(v => v + 1);
      })
      .catch(() => {});
    return () => ctrl.abort();
  }, [frame, runId, canvasLayout, hasScene]);

  // ── cleanup interval on unmount ────────────────────────────────────────────
  useEffect(() => {
    return () => {
      if (playIntervalRef.current) clearInterval(playIntervalRef.current);
    };
  }, []);

  // ── node bounds fetch ─────────────────────────────────────────────────────
  useEffect(() => {
    if (selectedNid == null) { setNodeBounds(null); return; }
    const ctrl = new AbortController();
    fetch(withToken(`/node/${selectedNid}?frame=${frame}${sceneParam}`), { signal: ctrl.signal })
      .then(r => r.ok ? r.json() as Promise<NodeBounds> : null)
      .then(data => setNodeBounds(data))
      .catch(() => {});
    return () => ctrl.abort();
  }, [selectedNid, frame]);

  // ── source line highlights ────────────────────────────────────────────────
  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    // Always clear previous decorations first
    if (editor && decorationsRef.current.length > 0) {
      decorationsRef.current = editor.deltaDecorations(decorationsRef.current, []);
    }
    if (!editor || !monaco || selectedNid == null || !currentFile) return;
    const entry = nodeInfoMap.get(selectedNid);
    if (!entry?.stack?.length) return;
    const basename = currentFile.split('/').pop() ?? '';
    const lines = entry.stack.filter(s => s.file === basename).map(s => s.line);
    if (!lines.length) return;
    decorationsRef.current = editor.deltaDecorations([], lines.map(line => ({
      range: new monaco.Range(line, 1, line, 1),
      options: { isWholeLine: true, className: 'node-source-highlight' },
    })));
    editor.revealLineInCenter(lines[0]);
  }, [selectedNid, currentFile, nodeInfoMap]);

  // ── editor ────────────────────────────────────────────────────────────────
  const handleEditorMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;

    monaco.languages.register({ id: 'toml', extensions: ['.toml'] });
    monaco.languages.setMonarchTokensProvider('toml', {
      tokenizer: {
        root: [
          [/#.*$/, 'comment'],
          [/^\s*\[{1,2}[^\]]*\]{1,2}/, 'keyword.section'],
          [/"(?:[^"\\]|\\.)*"/, 'string'],
          [/'[^']*'/, 'string'],
          [/"""[\s\S]*?"""/, 'string'],
          [/'''[\s\S]*?'''/, 'string'],
          [/\b(true|false)\b/, 'keyword'],
          [/[+-]?0x[0-9a-fA-F_]+/, 'number.hex'],
          [/[+-]?(?:inf|nan)\b/, 'number'],
          [/[+-]?\d[\d_]*(?:\.[\d_]+)?(?:[eE][+-]?[\d_]+)?/, 'number'],
          [/\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?)?/, 'string.date'],
          [/[a-zA-Z_][a-zA-Z0-9_.-]*(?=\s*=)/, 'variable'],
          [/=/, 'operator'],
          [/[,\[\]{}]/, 'delimiter'],
        ],
      },
    });

    editor.onDidChangeModelContent(() => {
      const idx = activeTabRef.current;
      if (idx < 0 || tabsRef.current[idx]?.isDirty) return;
      setTabs(tabsRef.current.map((t, i) => i === idx ? { ...t, isDirty: true } : t));
    });

    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
      const path = currentFileRef.current;
      if (!path?.endsWith('.ffpy')) return;
      const code = editorRef.current?.getValue();
      if (!code || !wsRef.current) return;
      fetch(withToken(`/file?path=${encodeURIComponent(path)}`), {
        method: 'PUT',
        headers: { 'Content-Type': 'text/plain; charset=utf-8' },
        body: code,
      }).then(() => markActiveTabClean()).catch(() => {});
      setLines([]);
      setRunning(true);
      wsRef.current.send(JSON.stringify({ type: 'run', path }));
    });

    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () => {
      const path = currentFileRef.current;
      if (!path) return;
      const content = editorRef.current?.getValue() ?? '';
      fetch(withToken(`/file?path=${encodeURIComponent(path)}`), {
        method: 'PUT',
        headers: { 'Content-Type': 'text/plain; charset=utf-8' },
        body: content,
      }).then(async (r) => {
        if (r.ok) {
          markActiveTabClean();
        } else {
          const msg = await r.text();
          setLines((prev) => [...prev, { kind: 'err', text: `Config error: ${msg}` }]);
        }
      }).catch(() => {});
    });
  };

  function terminate() {
    wsRef.current?.send(JSON.stringify({ type: 'terminate' }));
  }

  // ── canvas layout ─────────────────────────────────────────────────────────
  useEffect(() => {
    const el = canvasContentRef.current;
    if (!el || sceneWidth == null || sceneHeight == null || sceneWidth <= 0 || sceneHeight <= 0) { setCanvasLayout(null); return; }

    const recompute = (w: number, h: number) => {
      if (w <= 0 || h <= 0) return;
      const cssScale = Math.min(w / sceneWidth, h / sceneHeight);
      const dpr = window.devicePixelRatio || 1;
      const pngWidth  = Math.round(sceneWidth  * cssScale * dpr);
      const pngHeight = Math.round(sceneHeight * cssScale * dpr);
      setCanvasLayout({
        serverScale: cssScale * dpr,
        cssWidth:  Math.round(sceneWidth  * cssScale),
        cssHeight: Math.round(sceneHeight * cssScale),
        pngWidth,
        pngHeight,
      });
    };

    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      recompute(width, height);
    });
    ro.observe(el);
    recompute(el.clientWidth, el.clientHeight);
    return () => ro.disconnect();
  }, [sceneWidth, sceneHeight]);

  // ── playback ──────────────────────────────────────────────────────────────
  function cacheKey(n: number, scaleKey: string) {
    return `${runId}-${sceneCacheKey}-${n}-${scaleKey}`;
  }

  function storeCachedFrame(n: number, sk: string, pngBase64: string) {
    const key = cacheKey(n, sk);
    if (imageCacheRef.current.has(key)) return;
    const bytes = Uint8Array.from(atob(pngBase64), c => c.charCodeAt(0));
    const blob = new Blob([bytes], { type: 'image/png' });
    imageCacheRef.current.set(key, URL.createObjectURL(blob));
  }

  async function handlePlay() {
    if (!canvasLayout || !hasScene) return;
    cancelledRef.current = false;

    const startFrame = frame;
    const totalFrames = frames;
    const capturedFps = fps;
    const capturedCueFrames = cueFrames;
    const capturedStopOnCue = stopOnCue;
    const sk = canvasLayout.serverScale.toFixed(3);

    // Find which frames need caching
    const uncachedImages: number[] = [];
    const uncachedTrees: number[] = [];
    for (let i = 0; i < totalFrames; i++) {
      if (!imageCacheRef.current.has(cacheKey(i, sk))) uncachedImages.push(i);
      if (!treeCacheRef.current.has(`${runId}-${sceneCacheKey}-${i}`)) uncachedTrees.push(i);
    }

    if (uncachedImages.length > 0 || uncachedTrees.length > 0) {
      setIsPrefetching(true);
      try {
        await Promise.all([
          // Images: one bulk request
          uncachedImages.length > 0
            ? fetch(withToken(`/frames?from=${uncachedImages[0]}&to=${uncachedImages.at(-1)}&scale=${sk}&v=${runId}${sceneParam}`))
                .then(r => { if (!r.ok) throw new Error(`HTTP ${r.status}`); return r.json() as Promise<RenderedFrameResponse[]>; })
                .then(data => {
                  for (const { n, png } of data) storeCachedFrame(n, sk, png);
                  setCacheVersion(v => v + 1);
                })
            : Promise.resolve(),
          // Trees: one bulk request
          uncachedTrees.length > 0
            ? fetch(withToken(`/trees?from=${uncachedTrees[0]}&to=${uncachedTrees.at(-1)}&v=${runId}${sceneParam}`))
                .then(r => r.ok ? r.json() as Promise<Array<{ n: number; scene: SceneData }>> : null)
                .then(data => {
                  if (!data) return;
                  for (const { n, scene } of data) treeCacheRef.current.set(`${runId}-${sceneCacheKey}-${n}`, scene);
                })
                .catch(() => {})
            : Promise.resolve(),
        ]);
      } catch (e) {
        console.error('prefetch failed', e);
      } finally {
        setIsPrefetching(false);
      }
    }

    if (cancelledRef.current) return;

    playReturnFrameRef.current = startFrame;
    setIsPlaying(true);

    let f = startFrame;
    playIntervalRef.current = setInterval(() => {
      f++;
      if (f >= totalFrames) {
        clearInterval(playIntervalRef.current!);
        playIntervalRef.current = null;
        setIsPlaying(false);
        setFrame(playReturnFrameRef.current);
      } else if (capturedStopOnCue && f > startFrame && capturedCueFrames.includes(f)) {
        clearInterval(playIntervalRef.current!);
        playIntervalRef.current = null;
        setIsPlaying(false);
        setFrame(f);
        playReturnFrameRef.current = f;
      } else {
        setFrame(f);
      }
    }, 1000 / capturedFps);
  }

  function handleStop() {
    cancelledRef.current = true;
    if (playIntervalRef.current) {
      clearInterval(playIntervalRef.current);
      playIntervalRef.current = null;
    }
    setIsPlaying(false);
    setIsPrefetching(false);
    setFrame(playReturnFrameRef.current);
  }

  // ── export menu close on outside click ───────────────────────────────────
  useEffect(() => {
    if (!exportMenuOpen) return;
    function onDown(e: MouseEvent) {
      if (exportMenuRef.current && !exportMenuRef.current.contains(e.target as Node)) {
        setExportMenuOpen(false);
      }
    }
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [exportMenuOpen]);

  // ── video export handler ───────────────────────────────────────────────────
  function openExportDialog() {
    setExportMenuOpen(false);
    setExportWidth(sceneWidth ?? 0);
    setExportHeight(sceneHeight ?? 0);
    setExportToFrame(maxFrame);
    setExportFps(fps);
    setExportStatus('idle');
    setExportProgress(0);
    setExportMessage('');
    setExportDonePath('');
    setExportErrorMsg('');
    setExportDialogOpen(true);
  }

  function handleCodecChange(codec: 'h264' | 'h265' | 'vp9') {
    setExportCodec(codec);
    // Auto-fix extension to match container
    const ext = codec === 'vp9' ? '.webm' : '.mp4';
    setExportFilename(prev => prev.replace(/\.(mp4|webm)$/i, '') + ext);
  }

  async function handleExport() {
    if (exportStatus === 'exporting') return;
    setExportStatus('exporting');
    setExportProgress(0);
    setExportMessage('Rendering frames…');
    setExportDonePath('');
    setExportErrorMsg('');

    const body = {
      width: exportWidth, height: exportHeight,
      filename: exportFilename,
      fps: exportFps,
      codec: exportCodec,
      crf: exportCrf,
      from_frame: exportFromFrame,
      to_frame: exportToFrame,
      scene_index: selectedScene === 'all' ? null : selectedScene,
    };

    let finished = false;
    try {
      const response = await fetch(withToken('/export-video'), {
        method: 'POST',
        headers: {'Content-Type': 'application/json'},
        body: JSON.stringify(body),
      });
      if (!response.ok || !response.body) {
        setExportStatus('error');
        setExportErrorMsg(`Server error: ${response.status}`);
        return;
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buf = '';
      while (true) {
        const {done, value} = await reader.read();
        if (done) break;
        buf += decoder.decode(value, {stream: true});
        const lines = buf.split('\n');
        buf = lines.pop() ?? '';
        for (const line of lines) {
          if (!line.startsWith('data: ')) continue;
          try {
            const ev = JSON.parse(line.slice(6));
            if (ev.type === 'progress') {
              setExportProgress(Math.round(ev.done / ev.total * 100));
              setExportMessage(`Rendering frames… ${ev.done}/${ev.total}`);
            } else if (ev.type === 'ffmpeg') {
              setExportProgress(100);
              setExportMessage('Encoding video…');
            } else if (ev.type === 'done') {
              finished = true;
              setExportStatus('done');
              setExportDonePath(ev.path);
            } else if (ev.type === 'error') {
              finished = true;
              setExportStatus('error');
              setExportErrorMsg(ev.message);
            }
          } catch { /* ignore malformed lines */ }
        }
      }
      if (!finished) {
        setExportStatus('error');
        setExportErrorMsg('Export ended unexpectedly.');
      }
    } catch (e) {
      setExportStatus('error');
      setExportErrorMsg(String(e));
    }
  }

  // ── sequence ──────────────────────────────────────────────────────────────
  const isFfsqActive = currentFile?.endsWith('.ffsq') ?? false;
  const currentSeqResult = isFfsqActive ? (seqRenderResults.get(currentFile!) ?? null) : null;

  const handleSeqRenderResult = (path: string, res: SequenceRenderResult) => {
    setSeqRenderResults(prev => {
      const old = prev.get(path);
      if (old) {
        for (const sc of old.scenes) for (const url of sc.frames) { try { URL.revokeObjectURL(url); } catch {} }
      }
      const next = new Map(prev);
      next.set(path, res);
      return next;
    });
  };

  // ── image src ──────────────────────────────────────────────────────────────
  const scaleKey = canvasLayout?.serverScale.toFixed(3) ?? '1.000';
  const imgSrc = imageCacheRef.current.get(cacheKey(frame, scaleKey))
    ?? (canvasLayout ? withToken(`/frame/${frame}?scale=${scaleKey}&v=${runId}${sceneParam}`) : '');

  const isActive = isPlaying || isPrefetching;

  function FrameSlider() {
    const ZOOM_WIN = Math.min(frames, 31);
    const half = Math.floor(ZOOM_WIN / 2);
    const rawStart = frame - half;
    const winStart = Math.max(0, Math.min(rawStart, maxFrame - ZOOM_WIN + 1));
    const winEnd = Math.min(maxFrame, winStart + ZOOM_WIN - 1);
    const winSize = winEnd - winStart + 1;

    const ovCursorPct = maxFrame > 0 ? (frame / maxFrame) * 100 : 0;
    const ovWinLeft  = maxFrame > 0 ? (winStart / maxFrame) * 100 : 0;
    const ovWinWidth = maxFrame > 0 ? Math.max(0.5, ((winEnd - winStart) / maxFrame) * 100) : 100;

    function handleOverviewPtr(e: React.PointerEvent<HTMLDivElement>) {
      const rect = e.currentTarget.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      setFrame(Math.round(pct * maxFrame));
      e.currentTarget.setPointerCapture(e.pointerId);
    }
    function handleOverviewMove(e: React.PointerEvent<HTMLDivElement>) {
      if (e.buttons === 0) return;
      const rect = e.currentTarget.getBoundingClientRect();
      const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
      setFrame(Math.round(pct * maxFrame));
    }

    return (
      <div className="tl-frame-slider">
        {/* Global overview */}
        <div
          className="tl-overview"
          onPointerDown={handleOverviewPtr}
          onPointerMove={handleOverviewMove}
        >
          <div className="tl-ov-track" />
          {keyFrames.map(kf => {
            const pct = maxFrame > 0 ? (kf / maxFrame) * 100 : 0;
            return <div key={`k${kf}`} className="tl-ov-kf" style={{ left: `${pct}%` }} />;
          })}
          {cueFrames.map(cf => {
            const pct = maxFrame > 0 ? (cf / maxFrame) * 100 : 0;
            return <div key={`c${cf}`} className="tl-ov-cue" style={{ left: `${pct}%` }} />;
          })}
          <div className="tl-ov-window" style={{ left: `${ovWinLeft}%`, width: `${ovWinWidth}%` }} />
          <div className="tl-ov-cursor" style={{ left: `${ovCursorPct}%` }} />
        </div>

        {/* Zoomed detail slider */}
        <div className="timeline-slider-wrap">
          <div className="timeline-track" />
          {Array.from({ length: winSize }, (_, i) => {
            const fi = winStart + i;
            const pct = winSize > 1 ? (i / (winSize - 1)) * 100 : 50;
            const isKey = keyFrameSet.has(fi);
            const isCue = cueFrameSet.has(fi);
            const showNum = fi === frame || fi % 5 === 0;
            const tickClass = `timeline-tick${fi === frame ? ' active' : ''}${isCue ? ' cue' : isKey ? ' key' : ''}`;
            const numClass = `timeline-tick-num${fi === frame ? ' active' : ''}${isCue ? ' cue' : isKey ? ' key' : ''}`;
            return (
              <div key={fi} className="timeline-tick-wrap" style={{ left: `${pct}%` }}>
                <div className={tickClass} />
                {showNum && (
                  <div className={numClass}>{fi}</div>
                )}
              </div>
            );
          })}
          <input type="range" min={winStart} max={winEnd} step={1}
            value={frame}
            onChange={(e) => setFrame(Number(e.target.value))}
            className="timeline-slider"
            disabled={isActive}
          />
        </div>
      </div>
    );
  }

  if (authStatus === 'pending') return null;
  if (authStatus === 'error') return (
    <div className="ws-overlay">
      <div className="ws-error-box">
        <div className="ws-error-title">Access denied</div>
        <div className="ws-error-body">Invalid or missing token. Open the URL printed by the server.</div>
      </div>
    </div>
  );

  return (
    <>
    <div className="app">
      <MenuBar />

      {wsStatus === 'reconnecting' && (
        <div className="ws-banner ws-banner-reconnecting">
          Connection lost — reconnecting…
        </div>
      )}
      {wsStatus === 'failed' && (
        <div className="ws-overlay">
          <div className="ws-error-box">
            <div className="ws-error-title">Connection lost</div>
            <div className="ws-error-body">Could not reconnect to the server. Restart the server and reload the page.</div>
            <button className="ws-error-reload" onClick={() => window.location.reload()}>Reload</button>
          </div>
        </div>
      )}

      <div className="app-body">
        {/* ── Permanent sidebar strip ── */}
        <div className="sidebar-strip">
          <div className="sidebar-menu-anchor" ref={sidebarMenuRef}>
            <button
              className="sidebar-icon-btn"
              onClick={() => setSidebarMenuOpen(v => !v)}
              title="Main Menu"
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
                <rect x="2" y="4" width="12" height="1.5" rx="0.75" />
                <rect x="2" y="7.25" width="12" height="1.5" rx="0.75" />
                <rect x="2" y="10.5" width="12" height="1.5" rx="0.75" />
              </svg>
            </button>
            {sidebarMenuOpen && (
              <div className="sidebar-menu">
                {(['New File', 'New Folder', null, 'Open in Terminal', 'Reveal in File Manager', null, 'Preferences'] as (string | null)[]).map((item, i) =>
                  item === null
                    ? <div key={i} className="sidebar-menu-sep" />
                    : <button key={item} className="sidebar-menu-item" onClick={() => setSidebarMenuOpen(false)}>{item}</button>
                )}
              </div>
            )}
          </div>
          <button
            className={`sidebar-icon-btn${fileTreeCollapsed ? '' : ' sidebar-icon-btn-active'}`}
            onClick={toggleFileTree}
            title={fileTreeCollapsed ? 'Show Explorer' : 'Hide Explorer'}
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
              <rect x="2" y="2" width="5" height="12" rx="1" opacity="0.5" />
              <rect x="8" y="2" width="6" height="3" rx="1" />
              <rect x="8" y="7" width="6" height="3" rx="1" />
              <rect x="8" y="12" width="4" height="2" rx="1" />
            </svg>
          </button>
        </div>

        <PanelGroup orientation="horizontal" className="main-content">
        {/* ── File tree ── */}
        <Panel
          panelRef={fileTreePanelRef}
          defaultSize={10}
          minSize={12}
          collapsible
          collapsedSize={0}
          onResize={handleFileTreeResize}
        >
          <FileTree activeFile={currentFile} onFileClick={handleFileClick} refreshTrigger={fileTreeRefresh} />
        </Panel>

        <PanelResizeHandle className="resize-handle horizontal" />

        {/* ── Left: editor + console ── */}
        <Panel defaultSize={40} minSize={20}>
          <PanelGroup orientation="vertical">
            <Panel defaultSize={80} minSize={20}>
              <div className="editor-with-tabs">
                {tabs.length > 0 && (
                  <div className="tab-bar">
                    {tabs.map((tab, idx) => {
                      const name = tab.path.split('/').pop() ?? tab.path;
                      return (
                        <div
                          key={tab.path}
                          className={`tab${idx === activeTab ? ' tab-active' : ''}`}
                          onClick={() => handleTabClick(idx)}
                        >
                          <span className="tab-name">{name}</span>
                          {tab.isDirty && <span className="tab-dirty">●</span>}
                          <button
                            className="tab-close-btn"
                            onClick={e => { e.stopPropagation(); handleCloseTab(idx); }}
                            title="Close"
                          >×</button>
                        </div>
                      );
                    })}
                  </div>
                )}
                <div className="panel-fill" style={{ position: 'relative' }}>
                  <div style={{
                    position: 'absolute', inset: 0,
                    visibility: isFfsqActive ? 'hidden' : 'visible',
                    pointerEvents: isFfsqActive ? 'none' : 'auto',
                  }}>
                    <Editor
                      height="100%"
                      theme="vs-dark"
                      onMount={handleEditorMount}
                      options={{
                        minimap: { enabled: false },
                        fontSize: 13,
                        lineHeight: 20,
                        scrollBeyondLastLine: false,
                        renderLineHighlight: 'line',
                        padding: { top: 10 },
                      }}
                    />
                    {tabs.length === 0 && (
                      <div className="editor-placeholder">
                        <span>Open a file from the explorer</span>
                      </div>
                    )}
                  </div>
                  {isFfsqActive && currentFile && (
                    <div style={{ position: 'absolute', inset: 0 }}>
                      <SequenceEditor
                        key={currentFile}
                        path={currentFile}
                        fps={fps}
                        wsRef={wsRef}
                        setWsOverride={setWsOverride}
                        setLines={setLines}
                        setRunning={setRunning}
                        onRenderResult={res => handleSeqRenderResult(currentFile, res)}
                        getPlayerAreaSize={getSeqPlayerAreaSize}
                        renderTrigger={seqRenderTrigger}
                        onExportDone={refreshFileTree}
                      />
                    </div>
                  )}
                </div>
              </div>
            </Panel>

            <PanelResizeHandle className="resize-handle vertical" />

            <Panel defaultSize={20} minSize={10}>
              <div className="console-panel">
                <div className="console-header">
                  <span className="console-label">Output</span>
                  {running && (
                    <button className="console-stop-btn" onClick={terminate}>■ Stop</button>
                  )}
                </div>
                <div className="console-body">
                  {lines.map((line, i) => (
                    <div key={i} className={`console-line console-${line.kind}`}>{line.text}</div>
                  ))}
                  <div ref={consoleEndRef} />
                </div>
              </div>
            </Panel>
          </PanelGroup>
        </Panel>

        <PanelResizeHandle className="resize-handle horizontal" />

        {/* ── Right column ── */}
        <Panel defaultSize={50} minSize={20}>
          <div className={`right-column${!isFfsqActive && running ? ' right-column-evaluating' : ''}${isFfsqActive ? ' right-column-seq' : ''}`}>

            {isFfsqActive ? (
              /* ── Sequence player ── */
              <SequencePlayer result={currentSeqResult} fps={fps} canvasAreaRef={seqCanvasAreaRef} onRequestRerender={handleSeqRerender} isRendering={running} />
            ) : (
              <>
                {/* Timeline bar */}
                <div className="timeline-bar">
                  {!hasScene ? (
                    <span className="timeline-no-scene">No scene</span>
                  ) : (
                    <>
                      <FrameSlider />
                      <div className="tl-controls">
                        <button className="tl-btn" onClick={() => setFrame(0)}             disabled={frame === 0 || isActive}>⏮</button>
                        <button className="tl-btn" onClick={() => setFrame(prevKeyFrame!)} disabled={prevKeyFrame === null || isActive} title="Prev key frame">◂◂</button>
                        <button className="tl-btn" onClick={() => setFrame(f => f - 1)}    disabled={frame === 0 || isActive}>◀</button>
                        <button className="tl-btn" onClick={() => setFrame(f => f + 1)}    disabled={frame === maxFrame || isActive}>▶</button>
                        <button className="tl-btn" onClick={() => setFrame(nextKeyFrame!)} disabled={nextKeyFrame === null || isActive} title="Next key frame">▸▸</button>
                        <button className="tl-btn" onClick={() => setFrame(maxFrame)}      disabled={frame === maxFrame || isActive}>⏭</button>

                        <span className="tl-sep" />

                        <button
                          className={`tl-btn tl-play-btn${isActive ? ' active' : ''}`}
                          onClick={isActive ? handleStop : handlePlay}
                          disabled={!canvasLayout}
                          title={isActive ? 'Stop' : 'Play animation'}
                        >
                          {isPrefetching ? '…' : isPlaying ? '■' : '▶ Play'}
                        </button>

                        <button className="tl-btn" onClick={() => setFrame(prevCueFrame!)} disabled={prevCueFrame === null || isActive} title="Prev cue frame">◂●</button>
                        <button className="tl-btn" onClick={() => setFrame(nextCueFrame!)} disabled={nextCueFrame === null || isActive} title="Next cue frame">●▸</button>

                        <label className="tl-stop-on-cue" title="Stop playback on the next cue frame">
                          <input
                            type="checkbox"
                            checked={stopOnCue}
                            onChange={e => setStopOnCue(e.target.checked)}
                            disabled={isActive}
                          />
                          stop on cue
                        </label>

                        <span className="tl-sep" />

                        <span className="timeline-label">{frame} / {maxFrame}</span>
                        {cueFrameSet.has(frame) && <span className="tl-cue-badge">cue</span>}
                        {keyFrameSet.has(frame) && <span className="tl-key-badge">key</span>}

                        {scenes.length > 1 && (
                          <>
                            <span className="tl-sep" />
                            <select
                              className="tl-scene-select"
                              value={selectedScene === 'all' ? 'all' : String(selectedScene)}
                              onChange={e => {
                                const v = e.target.value;
                                setSelectedScene(v === 'all' ? 'all' : Number(v));
                              }}
                              disabled={isActive}
                            >
                              <option value="all">All scenes</option>
                              {scenes.map((s, i) => (
                                <option key={i} value={String(i)}>{s.name}</option>
                              ))}
                            </select>
                          </>
                        )}

                        <span className="tl-sep" />

                        <div className="tl-menu-anchor" ref={exportMenuRef}>
                          <button
                            className="tl-btn tl-menu-btn"
                            title="More options"
                            onClick={() => setExportMenuOpen(v => !v)}
                          >☰</button>
                          {exportMenuOpen && (
                            <div className="tl-dropdown">
                              <button
                                className="tl-dropdown-item"
                                onClick={openExportDialog}
                                disabled={!hasScene}
                              >Export as video…</button>
                            </div>
                          )}
                        </div>
                      </div>
                    </>
                  )}
                </div>

                {/* Scene tree + canvas */}
                <PanelGroup orientation="vertical">
                  <Panel defaultSize={40} minSize={15}>
                    <div className="panel-fill">
                      <TreeView scene={sceneData} prevScene={prevSceneData} selectedNid={selectedNid} onSelect={setSelectedNid} />
                    </div>
                  </Panel>

                  <PanelResizeHandle className="resize-handle vertical" />

                  <Panel defaultSize={60} minSize={15}>
                    <div className="canvas-panel">
                      <div ref={canvasContentRef} className="canvas-content">
                        {hasScene && canvasLayout && (
                          <div className="canvas-image-wrap" style={{ width: canvasLayout.cssWidth, height: canvasLayout.cssHeight }}>
                            <img
                              src={imgSrc}
                              style={{ width: canvasLayout.cssWidth, height: canvasLayout.cssHeight }}
                              className="canvas-image"
                              alt="rendered frame"
                            />
                            {nodeBounds && sceneWidth != null && sceneHeight != null && (() => {
                              const sx = canvasLayout.cssWidth / sceneWidth;
                              const sy = canvasLayout.cssHeight / sceneHeight;
                              const bx = nodeBounds.x * sx;
                              const by = nodeBounds.y * sy;
                              const bw = nodeBounds.width * sx;
                              const bh = nodeBounds.height * sy;
                              const isPoint = bw < 2 && bh < 2;
                              if (isPoint) return (
                                <>
                                  <div className="nh-vline" style={{ left: bx }} />
                                  <div className="nh-hline" style={{ top: by }} />
                                  <div className="nh-dot" style={{ left: bx, top: by }} />
                                </>
                              );
                              return (
                                <>
                                  <div className="nh-vline" style={{ left: bx }} />
                                  <div className="nh-vline" style={{ left: bx + bw }} />
                                  <div className="nh-hline" style={{ top: by }} />
                                  <div className="nh-hline" style={{ top: by + bh }} />
                                  <div className="nh-rect" style={{ left: bx, top: by, width: bw, height: bh }} />
                                </>
                              );
                            })()}
                          </div>
                        )}
                      </div>
                      <div className="canvas-statusbar">
                        {canvasLayout
                          ? `Rendered as ${canvasLayout.pngWidth}×${canvasLayout.pngHeight}`
                          : ''}
                      </div>
                    </div>
                  </Panel>
                </PanelGroup>
              </>
            )}
          </div>
        </Panel>
        </PanelGroup>
      </div>
    </div>

    {/* ── Export dialog ──────────────────────────────────────────────────── */}
    {exportDialogOpen && (
      <div className="modal-overlay" onClick={() => { if (exportStatus !== 'exporting') setExportDialogOpen(false); }}>
        <div className="export-dialog" onClick={e => e.stopPropagation()}>
          <div className="export-dialog-header">
            <span>Export as Video</span>
            <button
              className="export-dialog-close"
              onClick={() => setExportDialogOpen(false)}
              disabled={exportStatus === 'exporting'}
            >✕</button>
          </div>

          <div className="export-dialog-body">
            <div className="export-row">
              <label>Resolution</label>
              <span className="export-wh">
                <input type="number" className="export-num" value={exportWidth}  min={0} onChange={e => setExportWidth(Number(e.target.value))} />
                <span>×</span>
                <input type="number" className="export-num" value={exportHeight} min={0} onChange={e => setExportHeight(Number(e.target.value))} />
              </span>
              <span className="export-hint">0 = original</span>
            </div>

            <div className="export-row">
              <label>FPS</label>
              <input type="number" className="export-num" value={exportFps} min={1} max={120}
                onChange={e => setExportFps(Number(e.target.value))} />
            </div>

            <div className="export-row">
              <label>Codec</label>
              <select value={exportCodec} onChange={e => handleCodecChange(e.target.value as typeof exportCodec)}>
                <option value="h264">H.264 (.mp4)</option>
                <option value="h265">H.265 (.mp4)</option>
                <option value="vp9">VP9 (.webm)</option>
              </select>
            </div>

            <div className="export-row">
              <label>Quality (CRF {exportCrf})</label>
              <input type="range" className="export-slider" min={0} max={51} value={exportCrf}
                onChange={e => setExportCrf(Number(e.target.value))} />
              <span className="export-hint">lower = better</span>
            </div>

            <div className="export-row">
              <label>Filename</label>
              <input type="text" className="export-text" value={exportFilename}
                onChange={e => setExportFilename(e.target.value)} />
            </div>

            <div className="export-row">
              <label>Frame range</label>
              <input type="number" className="export-num" value={exportFromFrame} min={0} max={maxFrame}
                onChange={e => setExportFromFrame(Number(e.target.value))} />
              <span>–</span>
              <input type="number" className="export-num" value={exportToFrame} min={0} max={maxFrame}
                onChange={e => setExportToFrame(Number(e.target.value))} />
            </div>
          </div>

          {exportStatus === 'exporting' && (
            <div className="export-progress-area">
              <div className="export-progress-label">{exportMessage}</div>
              <div className="export-progress-track">
                <div className="export-progress-fill" style={{width: `${exportProgress}%`}} />
              </div>
            </div>
          )}

          {exportStatus === 'done' && (
            <div className="export-result export-result-ok">
              Saved to <code>{exportDonePath}</code>
            </div>
          )}

          {exportStatus === 'error' && (
            <div className="export-result export-result-err">{exportErrorMsg}</div>
          )}

          <div className="export-dialog-footer">
            <button className="export-btn-cancel"
              onClick={() => setExportDialogOpen(false)}
              disabled={exportStatus === 'exporting'}>
              {exportStatus === 'done' || exportStatus === 'error' ? 'Close' : 'Cancel'}
            </button>
            <button className="export-btn-ok"
              onClick={handleExport}
              disabled={exportStatus === 'exporting' || !exportFilename.trim()}>
              {exportStatus === 'exporting' ? 'Exporting…' : 'Export'}
            </button>
          </div>
        </div>
      </div>
    )}
    </>
  );
}
