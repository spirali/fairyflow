import { useState, useEffect, useRef } from 'react';
import { Panel, Group as PanelGroup, Separator as PanelResizeHandle } from 'react-resizable-panels';
import Editor from '@monaco-editor/react';
import MenuBar from './components/MenuBar';
import TreeView, { addIds } from './components/TreeView';
import './App.css';

const FRAMES_PER_STEP = 10;

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

  // ── mode ─────────────────────────────────────────────────────────────────
  const [mode, setMode] = useState('step');   // 'step' | 'animation'

  // ── step mode ────────────────────────────────────────────────────────────
  const [step, setStep] = useState(0);

  // ── animation mode ───────────────────────────────────────────────────────
  const [absFrame, setAbsFrame] = useState(0); // 0 .. steps*FRAMES_PER_STEP-1
  const [timePerStep, setTimePerStep] = useState(1.0);
  const [playing, setPlaying] = useState(false);

  // ── editor / console ─────────────────────────────────────────────────────
  const [lines, setLines] = useState([]);
  const [running, setRunning] = useState(false);
  const wsRef = useRef(null);
  const editorRef = useRef(null);
  const pendingFileContent = useRef(null);
  const consoleEndRef = useRef(null);

  // ── derived ───────────────────────────────────────────────────────────────
  const maxStep = steps - 1;
  const totalFrames = steps * FRAMES_PER_STEP;
  const currentStep = mode === 'step' ? step : Math.floor(absFrame / FRAMES_PER_STEP);
  const currentFrame = mode === 'step' ? 0 : absFrame % FRAMES_PER_STEP;

  // ── websocket ─────────────────────────────────────────────────────────────
  useEffect(() => {
    const ws = new WebSocket(`ws://${window.location.host}/ws`);
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
        setAbsFrame(0);
        setPlaying(false);
      } else if (msg.type === 'done') {
        setRunning(false);
        const label =
          msg.exit_code === 0 ? 'Process finished successfully'
          : msg.exit_code != null ? `Process exited with code ${msg.exit_code}`
          : 'Process terminated';
        setLines((prev) => [...prev, { kind: 'sys', text: label }]);
      }
    };
    wsRef.current = ws;
    return () => ws.close();
  }, []);

  // ── playback ──────────────────────────────────────────────────────────────
  useEffect(() => {
    if (!playing) return;
    const msPerFrame = Math.max(16, Math.round((timePerStep * 1000) / FRAMES_PER_STEP));
    const id = setInterval(() => {
      setAbsFrame((f) => {
        if (f >= totalFrames - 1) { setPlaying(false); return f; }
        return f + 1;
      });
    }, msPerFrame);
    return () => clearInterval(id);
  }, [playing, timePerStep, totalFrames]);

  // ── auto-scroll console ────────────────────────────────────────────────────
  useEffect(() => {
    consoleEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [lines]);

  // ── mode switch ───────────────────────────────────────────────────────────
  function switchMode(m) {
    setPlaying(false);
    if (m === 'animation') setAbsFrame(step * FRAMES_PER_STEP);
    else setStep(Math.floor(absFrame / FRAMES_PER_STEP));
    setMode(m);
  }

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
              <div className={`timeline-tick${i === currentStep ? ' active' : ''}`} />
              <div className={`timeline-tick-num${i === currentStep ? ' active' : ''}`}>{i}</div>
            </div>
          );
        })}
        <input type="range" min={0} max={maxStep} step={1}
          value={mode === 'step' ? step : currentStep}
          onChange={(e) => {
            const s = Number(e.target.value);
            if (mode === 'step') setStep(s);
            else setAbsFrame(s * FRAMES_PER_STEP);
          }}
          className="timeline-slider"
        />
      </div>
    );
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
              {/* Mode toggle — always visible */}
              <div className="mode-toggle">
                <button className={`mode-btn${mode === 'step' ? ' active' : ''}`}
                  onClick={() => switchMode('step')}>Step</button>
                <button className={`mode-btn${mode === 'animation' ? ' active' : ''}`}
                  onClick={() => switchMode('animation')}>Animation</button>
              </div>

              {!hasScene ? (
                <span className="timeline-no-scene">No scene</span>
              ) : mode === 'step' ? (
                /* ── Step mode ── */
                <>
                  <span className="timeline-label">{step} / {maxStep}</span>
                  <button className="tl-btn" onClick={() => setStep(0)}          disabled={step === 0}>⏮</button>
                  <button className="tl-btn" onClick={() => setStep(s => s - 1)} disabled={step === 0}>◀</button>
                  <button className="tl-btn" onClick={() => setStep(s => s + 1)} disabled={step === maxStep}>▶</button>
                  <button className="tl-btn" onClick={() => setStep(maxStep)}    disabled={step === maxStep}>⏭</button>
                  <StepSlider />
                </>
              ) : (
                /* ── Animation mode ── */
                <>
                  <span className="timeline-label">
                    s{currentStep}&thinsp;f{currentFrame}/{FRAMES_PER_STEP}
                  </span>
                  {/* Step nav */}
                  <button className="tl-btn" onClick={() => { setPlaying(false); setAbsFrame(0); }}
                    disabled={absFrame === 0}>⏮</button>
                  <button className="tl-btn" onClick={() => { setPlaying(false); setAbsFrame(f => Math.max(0, f - FRAMES_PER_STEP)); }}
                    disabled={absFrame === 0}>◀</button>
                  <button className="tl-btn" onClick={() => { setPlaying(false); setAbsFrame(f => Math.min(totalFrames - 1, f + FRAMES_PER_STEP)); }}
                    disabled={absFrame >= totalFrames - 1}>▶</button>
                  <button className="tl-btn" onClick={() => { setPlaying(false); setAbsFrame(totalFrames - 1); }}
                    disabled={absFrame >= totalFrames - 1}>⏭</button>
                  {/* Frame nav */}
                  <button className="tl-btn tl-btn-frame" title="Prev frame"
                    onClick={() => { setPlaying(false); setAbsFrame(f => Math.max(0, f - 1)); }}
                    disabled={absFrame === 0}>❮</button>
                  <button className="tl-btn tl-btn-frame" title="Next frame"
                    onClick={() => { setPlaying(false); setAbsFrame(f => Math.min(totalFrames - 1, f + 1)); }}
                    disabled={absFrame >= totalFrames - 1}>❯</button>
                  {/* Play */}
                  <button className="tl-btn tl-btn-play"
                    onClick={() => {
                      if (absFrame >= totalFrames - 1) setAbsFrame(0);
                      setPlaying(p => !p);
                    }}>
                    {playing ? '⏸' : '▶'}
                  </button>
                  {/* Time per step */}
                  <label className="tl-time-label">
                    <input type="number" className="tl-time-input"
                      min={0.1} max={60} step={0.1}
                      value={timePerStep}
                      onChange={(e) => setTimePerStep(Math.max(0.1, Number(e.target.value)))}
                    />
                    s/step
                  </label>
                  <StepSlider />
                </>
              )}
            </div>

            {/* Scene tree + canvas */}
            <PanelGroup orientation="vertical" style={{ flex: 1, minHeight: 0 }}>
              <Panel defaultSize={40} minSize={15}>
                <div className="panel-fill">
                  <TreeView nodes={treeNodes} step={currentStep} frame={currentFrame} framesPerStep={FRAMES_PER_STEP} />
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
