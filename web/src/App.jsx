import { useState, useEffect, useRef } from 'react';
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle } from 'react-resizable-panels';
import Editor from '@monaco-editor/react';
import MenuBar from './components/MenuBar';
import TreeView, { addIds } from './components/TreeView';
import './App.css';

const DEFAULT_CODE = `\
with node().size(300, 200):
    rect().size(30, 20).color("green")  # Add node into the root node

    with node():
        rect()
        circle().radius(10)
`;

export default function App() {
  // ── scene state ──────────────────────────────────────────────────────────
  const [treeNodes, setTreeNodes] = useState([]);
  const [steps, setSteps] = useState(1);
  const [step, setStep] = useState(0);

  // ── editor / console ─────────────────────────────────────────────────────
  const [lines, setLines] = useState([]);
  const [running, setRunning] = useState(false);
  const [wsStatus, setWsStatus] = useState('connecting'); // 'connected' | 'reconnecting' | 'failed'
  const wsRef = useRef(null);
  const editorRef = useRef(null);
  const pendingFileContent = useRef(null);
  const consoleEndRef = useRef(null);
  const reconnectCount = useRef(0);

  // ── derived ───────────────────────────────────────────────────────────────
  const maxStep = steps - 1;

  // ── websocket ─────────────────────────────────────────────────────────────
  useEffect(() => {
    let dead = false;
    let retryTimer = null;
    const MAX_RETRIES = 3;

    function connect() {
      const ws = new WebSocket(`ws://${window.location.host}/ws`);

      ws.onopen = () => {
        if (dead) { ws.close(); return; }
        reconnectCount.current = 0;
        setWsStatus('connected');
        wsRef.current = ws;
      };

      ws.onmessage = (e) => {
        const msg = JSON.parse(e.data);
        if (msg.type === 'file') {
          if (editorRef.current) editorRef.current.setValue(msg.content);
          else pendingFileContent.current = msg.content;
        } else if (msg.type === 'output') {
          setLines((prev) => [...prev, { kind: 'out', text: msg.text }]);
        } else if (msg.type === 'error') {
          setLines((prev) => [...prev, { kind: 'err', text: msg.text }]);
        } else if (msg.type === 'tree') {
          setTreeNodes(addIds(msg.nodes));
          setSteps(msg.steps);
          setStep(0);
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
      clearTimeout(retryTimer);
      wsRef.current?.close();
    };
  }, []);

  // ── auto-scroll console ────────────────────────────────────────────────────
  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  // ── editor ────────────────────────────────────────────────────────────────
  function handleEditorMount(editor, monaco) {
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
  }

  function terminate() {
    wsRef.current?.send(JSON.stringify({ type: 'terminate' }));
  }

  // ── timeline bar ──────────────────────────────────────────────────────────
  const hasScene = treeNodes.length > 0;

  function StepSlider() {
    return (
      <div className="timeline-slider-wrap">
        <div className="timeline-track" />
        {Array.from({ length: steps }, (_, i) => {
          const pct = maxStep > 0 ? (i / maxStep) * 100 : 0;
          return (
            <div key={i} className="timeline-tick-wrap" style={{ left: `${pct}%` }}>
              <div className={`timeline-tick${i === step ? ' active' : ''}`} />
              <div className={`timeline-tick-num${i === step ? ' active' : ''}`}>{i}</div>
            </div>
          );
        })}
        <input type="range" min={0} max={maxStep} step={1}
          value={step}
          onChange={(e) => setStep(Number(e.target.value))}
          className="timeline-slider"
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
                  <span className="timeline-label">{step} / {maxStep}</span>
                  <button className="tl-btn" onClick={() => setStep(0)}          disabled={step === 0}>⏮</button>
                  <button className="tl-btn" onClick={() => setStep(s => s - 1)} disabled={step === 0}>◀</button>
                  <button className="tl-btn" onClick={() => setStep(s => s + 1)} disabled={step === maxStep}>▶</button>
                  <button className="tl-btn" onClick={() => setStep(maxStep)}    disabled={step === maxStep}>⏭</button>
                  <StepSlider />
                </>
              )}
            </div>

            {/* Scene tree + canvas */}
            <PanelGroup orientation="vertical" style={{ flex: 1, minHeight: 0 }}>
              <Panel defaultSize={40} minSize={15}>
                <div className="panel-fill">
                  <TreeView nodes={treeNodes} step={step} />
                </div>
              </Panel>

              <PanelResizeHandle className="resize-handle vertical" />

              <Panel defaultSize={60} minSize={15}>
                <div className="canvas-panel">
                  <div className="canvas-rect" />
                </div>
              </Panel>
            </PanelGroup>
          </div>
        </Panel>
      </PanelGroup>
    </div>
  );
}
