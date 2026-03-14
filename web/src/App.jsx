import { useState, useEffect, useRef } from 'react';
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle } from 'react-resizable-panels';
import Editor from '@monaco-editor/react';
import MenuBar from './components/MenuBar';
import TreeView from './components/TreeView';
import './App.css';

const DEFAULT_CODE = `\
from alsie import Scene, Rect, animate

scene = Scene(width=800, height=533)

r = Rect(x=100, y=100, width=200, height=150, color="#4a9eff")
scene.add(r)

@animate(duration=2.0)
def intro(t):
    r.x = 100 + t * 300
    r.opacity = t

scene.render()
`;

export default function App() {
  const [frame, setFrame] = useState(0);
  const [lines, setLines] = useState([]);
  const [running, setRunning] = useState(false);
  const wsRef = useRef(null);
  const editorRef = useRef(null);
  const consoleEndRef = useRef(null);

  useEffect(() => {
    const ws = new WebSocket(`ws://${window.location.host}/ws`);
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      if (msg.type === 'output') {
        setLines((prev) => [...prev, { kind: 'out', text: msg.text }]);
      } else if (msg.type === 'error') {
        setLines((prev) => [...prev, { kind: 'err', text: msg.text }]);
      } else if (msg.type === 'done') {
        setRunning(false);
        const label =
          msg.exit_code === 0
            ? 'Process finished successfully'
            : msg.exit_code != null
            ? `Process exited with code ${msg.exit_code}`
            : 'Process terminated';
        setLines((prev) => [...prev, { kind: 'sys', text: label }]);
      }
    };
    wsRef.current = ws;
    return () => ws.close();
  }, []);

  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  function handleEditorMount(editor, monaco) {
    editorRef.current = editor;
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

  return (
    <div className="app">
      <MenuBar />

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
                    <button className="console-stop-btn" onClick={terminate} title="Terminate">
                      ■ Stop
                    </button>
                  )}
                </div>
                <div className="console-body">
                  {lines.map((line, i) => (
                    <div key={i} className={`console-line console-${line.kind}`}>
                      {line.text}
                    </div>
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
            {/* Timeline slider */}
            <div className="timeline-bar">
              <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>Frame</span>
              <input
                type="range"
                min={0}
                max={120}
                value={frame}
                onChange={(e) => setFrame(Number(e.target.value))}
                className="timeline-slider"
              />
              <span className="timeline-label">{frame} / 120</span>
            </div>

            {/* Scene tree + canvas */}
            <PanelGroup orientation="vertical" style={{ flex: 1, minHeight: 0 }}>
              <Panel defaultSize={40} minSize={15}>
                <div className="panel-fill">
                  <TreeView />
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
