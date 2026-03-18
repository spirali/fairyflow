import { useState, useEffect, useRef } from 'react';
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle } from 'react-resizable-panels';
import Editor from '@monaco-editor/react';
import type { OnMount } from '@monaco-editor/react';
import MenuBar from './components/MenuBar';
import TreeView, { addIds } from './components/TreeView';
import type { ConsoleLine, NodeBounds, RawNode, ServerMsg, TreeNodeData, WsStatus } from './types';
import './App.css';

type MonacoEditor = Parameters<OnMount>[0];

const DEFAULT_CODE = `\
with node().size(300, 200):
    rect().size(30, 20).color("green")  # Add node into the root node

    with node():
        rect()
        circle().radius(10)
`;

interface RenderedFrameResponse { n: number; png: string }

export default function App() {
  // ── scene state ──────────────────────────────────────────────────────────
  const [allFrames, setAllFrames] = useState<RawNode[]>([]);
  const [frames, setFrames] = useState(1);
  const [keyFrames, setKeyFrames] = useState<number[]>([]);
  const [frame, setFrame] = useState(0);
  const [runId, setRunId] = useState(0);

  // ── canvas layout ─────────────────────────────────────────────────────────
  interface CanvasLayout { serverScale: number; cssWidth: number; cssHeight: number; pngWidth: number; pngHeight: number }
  const [canvasLayout, setCanvasLayout] = useState<CanvasLayout | null>(null);
  const canvasContentRef = useRef<HTMLDivElement | null>(null);

  // ── playback ──────────────────────────────────────────────────────────────
  const [fps, setFps] = useState(24);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isPrefetching, setIsPrefetching] = useState(false);
  const imageCacheRef = useRef<Map<string, string>>(new Map());
  const playIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const playReturnFrameRef = useRef(0);
  const cancelledRef = useRef(false);

  // ── node selection ────────────────────────────────────────────────────────
  const [selectedNid, setSelectedNid] = useState<number | null>(null);
  const [nodeBounds, setNodeBounds] = useState<NodeBounds | null>(null);

  // ── editor / console ─────────────────────────────────────────────────────
  const [lines, setLines] = useState<ConsoleLine[]>([]);
  const [running, setRunning] = useState(false);
  const [wsStatus, setWsStatus] = useState<WsStatus>('connecting');
  const wsRef = useRef<WebSocket | null>(null);
  const editorRef = useRef<MonacoEditor | null>(null);
  const pendingFileContent = useRef<string | null>(null);
  const consoleEndRef = useRef<HTMLDivElement | null>(null);
  const reconnectCount = useRef(0);

  // ── derived ───────────────────────────────────────────────────────────────
  const maxFrame = frames - 1;
  const keyFrameSet = new Set(keyFrames);
  const prevKeyFrame: number | null = keyFrames.filter(kf => kf < frame).at(-1) ?? null;
  const nextKeyFrame: number | null = keyFrames.find(kf => kf > frame) ?? null;

  // ── websocket ─────────────────────────────────────────────────────────────
  useEffect(() => {
    let dead = false;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    const MAX_RETRIES = 3;

    function connect() {
      const ws = new WebSocket(`ws://${window.location.host}/ws`);

      ws.onopen = () => {
        if (dead) { ws.close(); return; }
        reconnectCount.current = 0;
        setWsStatus('connected');
        wsRef.current = ws;
      };

      ws.onmessage = (e: MessageEvent<string>) => {
        const msg = JSON.parse(e.data) as ServerMsg;
        if (msg.type === 'file') {
          if (editorRef.current) editorRef.current.setValue(msg.content);
          else pendingFileContent.current = msg.content;
        } else if (msg.type === 'output') {
          setLines((prev) => [...prev, { kind: 'out', text: msg.text }]);
        } else if (msg.type === 'error') {
          setLines((prev) => [...prev, { kind: 'err', text: msg.text }]);
        } else if (msg.type === 'tree') {
          setAllFrames(msg.frames);
          setFrames(msg.frames.length);
          setKeyFrames(msg.key_frames ?? []);
          setFrame(0);
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
  }, []);

  // ── auto-scroll console ────────────────────────────────────────────────────
  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  // ── cache invalidation + selection clear when a new run arrives ───────────
  useEffect(() => {
    const cache = imageCacheRef.current;
    cache.forEach(url => URL.revokeObjectURL(url));
    cache.clear();
    setSelectedNid(null);
    setNodeBounds(null);
  }, [runId]);

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
    fetch(`/node/${selectedNid}?frame=${frame}`, { signal: ctrl.signal })
      .then(r => r.ok ? r.json() as Promise<NodeBounds> : null)
      .then(data => setNodeBounds(data))
      .catch(() => {});
    return () => ctrl.abort();
  }, [selectedNid, frame]);

  // ── editor ────────────────────────────────────────────────────────────────
  const handleEditorMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    if (pendingFileContent.current !== null) {
      editor.setValue(pendingFileContent.current);
      pendingFileContent.current = null;
    }
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
      const code = editorRef.current?.getValue();
      if (!code || !wsRef.current) return;
      setLines([]);
      setRunning(true);
      wsRef.current.send(JSON.stringify({ type: 'run', code }));
    });
  };

  function terminate() {
    wsRef.current?.send(JSON.stringify({ type: 'terminate' }));
  }

  // ── canvas layout ─────────────────────────────────────────────────────────
  const sceneWidth  = allFrames[0]?.width  ?? null;
  const sceneHeight = allFrames[0]?.height ?? null;

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
    return `${runId}-${n}-${scaleKey}`;
  }

  function storeCachedFrame(n: number, scaleKey: string, pngBase64: string) {
    const key = cacheKey(n, scaleKey);
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
    const scaleKey = canvasLayout.serverScale.toFixed(3);

    // Find which frames are not yet cached
    const uncached: number[] = [];
    for (let i = 0; i < totalFrames; i++) {
      if (!imageCacheRef.current.has(cacheKey(i, scaleKey))) uncached.push(i);
    }

    if (uncached.length > 0) {
      setIsPrefetching(true);
      try {
        const from = uncached[0];
        const to = uncached[uncached.length - 1];
        const res = await fetch(`/frames?from=${from}&to=${to}&scale=${scaleKey}&v=${runId}`);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const data: RenderedFrameResponse[] = await res.json();
        for (const { n, png } of data) storeCachedFrame(n, scaleKey, png);
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

  // ── timeline ──────────────────────────────────────────────────────────────
  const treeNodes: TreeNodeData[] = allFrames[frame] ? addIds([allFrames[frame]]) : [];
  const prevTreeNodes: TreeNodeData[] = frame > 0 && allFrames[frame - 1] ? addIds([allFrames[frame - 1]]) : [];
  const hasScene = treeNodes.length > 0;

  // Resolve the image src: use cached blob URL if available, else fetch from server
  const scaleKey = canvasLayout?.serverScale.toFixed(3) ?? '1.000';
  const imgSrc = imageCacheRef.current.get(cacheKey(frame, scaleKey))
    ?? (canvasLayout ? `/frame/${frame}?scale=${scaleKey}&v=${runId}` : '');

  const isActive = isPlaying || isPrefetching;

  function FrameSlider() {
    return (
      <div className="timeline-slider-wrap">
        <div className="timeline-track" />
        {Array.from({ length: frames }, (_, i) => {
          const pct = maxFrame > 0 ? (i / maxFrame) * 100 : 0;
          const isKey = keyFrameSet.has(i);
          return (
            <div key={i} className="timeline-tick-wrap" style={{ left: `${pct}%` }}>
              <div className={`timeline-tick${i === frame ? ' active' : ''}${isKey ? ' key' : ''}`} />
              <div className={`timeline-tick-num${i === frame ? ' active' : ''}${isKey ? ' key' : ''}`}>{i}</div>
            </div>
          );
        })}
        <input type="range" min={0} max={maxFrame} step={1}
          value={frame}
          onChange={(e) => setFrame(Number(e.target.value))}
          className="timeline-slider"
          disabled={isActive}
        />
      </div>
    );
  }

  return (
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

      <PanelGroup orientation="horizontal" className="main-content">
        {/* ── Left: editor + console ── */}
        <Panel defaultSize={50} minSize={20}>
          <PanelGroup orientation="vertical">
            <Panel defaultSize={80} minSize={20}>
              <div className="panel-fill">
                <Editor
                  height="100%"
                  defaultLanguage="python"
                  defaultValue={DEFAULT_CODE}
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
          <div className="right-column">

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

                    <label className="tl-fps-label">
                      FPS
                      <input
                        type="number"
                        className="tl-fps-input"
                        value={fps}
                        min={1}
                        max={120}
                        disabled={isActive}
                        onChange={e => setFps(Math.max(1, Math.min(120, Number(e.target.value))))}
                      />
                    </label>

                    <span className="tl-sep" />

                    <span className="timeline-label">{frame} / {maxFrame}</span>
                    {keyFrameSet.has(frame) && <span className="tl-key-badge">key</span>}
                  </div>
                </>
              )}
            </div>

            {/* Scene tree + canvas */}
            <PanelGroup orientation="vertical" style={{ flex: 1, minHeight: 0 }}>
              <Panel defaultSize={40} minSize={15}>
                <div className="panel-fill">
                  <TreeView nodes={treeNodes} prevNodes={prevTreeNodes} selectedNid={selectedNid} onSelect={setSelectedNid} />
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
          </div>
        </Panel>
      </PanelGroup>
    </div>
  );
}
