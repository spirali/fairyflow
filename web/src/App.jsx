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
  const [step, setStep] = useState(0);
  const [steps, setSteps] = useState(1);
  const [lines, setLines] = useState([]);
  const [running, setRunning] = useState(false);
  const [treeNodes, setTreeNodes] = useState([]);
  const wsRef = useRef(null);
  const editorRef = useRef(null);
  const pendingFileContent = useRef(null);
  const consoleEndRef = useRef(null);

  useEffect(() => {
    const ws = new WebSocket(`ws://${window.location.host}/ws`);
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      if (msg.type === 'file') {
        if (editorRef.current) {
          editorRef.current.setValue(msg.content);
        } else {
          pendingFileContent.current = msg.content;
        }
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

  const maxStep = steps - 1;

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
            {/* Timeline bar */}
            <div className="timeline-bar">
              {treeNodes.length === 0 ? (
                <span className="timeline-no-scene">No scene</span>
              ) : (
                <>
                  <span className="timeline-label" style={{ minWidth: 0, marginRight: 4 }}>{step} / {maxStep}</span>
                  <button className="tl-btn" title="Start"    onClick={() => setStep(0)}          disabled={step === 0}>⏮</button>
                  <button className="tl-btn" title="Previous" onClick={() => setStep(s => s - 1)} disabled={step === 0}>◀</button>
                  <button className="tl-btn" title="Next"     onClick={() => setStep(s => s + 1)} disabled={step === maxStep}>▶</button>
                  <button className="tl-btn" title="End"      onClick={() => setStep(maxStep)}    disabled={step === maxStep}>⏭</button>
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
                    <input
                      type="range"
                      min={0}
                      max={maxStep}
                      step={1}
                      value={step}
                      onChange={(e) => setStep(Number(e.target.value))}
                      className="timeline-slider"
                    />
                  </div>
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
