import { useState, useEffect, useCallback, useRef } from "react";
import { withToken } from "../auth";
import type {
  ConsoleLine,
  SceneData,
  ServerMsg,
  SequenceRenderResult,
  SequenceSceneResult,
} from "../types";

interface Props {
  path: string;
  fps: number;
  wsRef: React.RefObject<WebSocket | null>;
  setWsOverride: (fn: ((msg: ServerMsg) => void) | null) => void;
  setLines: React.Dispatch<React.SetStateAction<ConsoleLine[]>>;
  setRunning: React.Dispatch<React.SetStateAction<boolean>>;
  onRenderResult: (result: SequenceRenderResult) => void;
  getPlayerAreaSize: () => { width: number; height: number } | null;
  renderTrigger?: number;
  onExportDone?: () => void;
}

export default function SequenceEditor({
  path,
  fps,
  wsRef,
  setWsOverride,
  setLines,
  setRunning,
  onRenderResult,
  getPlayerAreaSize,
  renderTrigger,
  onExportDone,
}: Props) {
  const [sceneFiles, setSceneFiles] = useState<string[]>([]);
  const [isDirty, setIsDirty] = useState(false);
  const [isRendering, setIsRendering] = useState(false);
  const [renderStatus, setRenderStatus] = useState("");
  const [dragOverIdx, setDragOverIdx] = useState<number | null>(null);
  const [dragSrcIdx, setDragSrcIdx] = useState<number | null>(null);
  const [dropZoneActive, setDropZoneActive] = useState(false);

  // Export-to-player dialog
  const [exportOpen, setExportOpen] = useState(false);
  const [exportFilename, setExportFilename] = useState("");
  type ExportStatus = "idle" | "exporting" | "done" | "error";
  const [exportStatus, setExportStatus] = useState<ExportStatus>("idle");
  const [exportResultPath, setExportResultPath] = useState("");
  const [exportError, setExportError] = useState("");

  // Export as video dialog
  const [videoExportOpen, setVideoExportOpen] = useState(false);
  const [videoFilename, setVideoFilename] = useState("");
  const [videoWidth, setVideoWidth] = useState(0);
  const [videoHeight, setVideoHeight] = useState(0);
  const [videoFps, setVideoFps] = useState(fps);
  const [videoCodec, setVideoCodec] = useState<"h264" | "h265" | "vp9">("h264");
  const [videoCrf, setVideoCrf] = useState(23);
  const [videoExportStatus, setVideoExportStatus] = useState<ExportStatus>("idle");
  const [videoExportProgress, setVideoExportProgress] = useState(0);
  const [videoExportMessage, setVideoExportMessage] = useState("");
  const [videoExportDonePath, setVideoExportDonePath] = useState("");
  const [videoExportError, setVideoExportError] = useState("");

  // Export-to-PDF dialog
  type FrameSelection = "cue_frames" | "cue_plus_key_frames" | "all_frames";
  const [pdfExportOpen, setPdfExportOpen] = useState(false);
  const [pdfFilename, setPdfFilename] = useState("");
  const [pdfFrameSelection, setPdfFrameSelection] = useState<FrameSelection>("cue_frames");
  const [pdfExportStatus, setPdfExportStatus] = useState<ExportStatus>("idle");
  const [pdfExportResultPath, setPdfExportResultPath] = useState("");
  const [pdfExportError, setPdfExportError] = useState("");

  // Load .ffsq file
  useEffect(() => {
    fetch(withToken(`/file?path=${encodeURIComponent(path)}`))
      .then((r) => (r.ok ? r.text() : null))
      .then((text) => {
        if (!text) {
          setSceneFiles([]);
          return;
        }
        try {
          const data = JSON.parse(text);
          setSceneFiles(Array.isArray(data.scene_files) ? data.scene_files : []);
        } catch {
          setSceneFiles([]);
        }
        setIsDirty(false);
      })
      .catch(() => {});
  }, [path]);

  // Clear override on unmount only — not on every re-render
  useEffect(() => {
    return () => {
      setWsOverride(null);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const save = useCallback(
    async (files: string[]) => {
      const content = JSON.stringify({ scene_files: files }, null, 2);
      await fetch(withToken(`/file?path=${encodeURIComponent(path)}`), {
        method: "PUT",
        headers: { "Content-Type": "text/plain; charset=utf-8" },
        body: content,
      });
      setIsDirty(false);
    },
    [path],
  );

  // Ctrl+S handler
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        if (isDirty) save(sceneFiles);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isDirty, sceneFiles, save]);

  const removeItem = (idx: number) => {
    setSceneFiles((prev) => prev.filter((_, i) => i !== idx));
    setIsDirty(true);
  };

  // ── Drag-to-reorder within list ────────────────────────────────────────────

  const handleItemDragStart = (e: React.DragEvent, idx: number) => {
    setDragSrcIdx(idx);
    e.dataTransfer.setData("application/seq-reorder", String(idx));
    e.dataTransfer.effectAllowed = "move";
  };

  const handleItemDragEnd = () => {
    setDragSrcIdx(null);
    setDragOverIdx(null);
  };

  const handleItemDragOver = (e: React.DragEvent, idx: number) => {
    if (
      e.dataTransfer.types.includes("application/seq-reorder") ||
      e.dataTransfer.types.includes("application/ffpy-path")
    ) {
      e.preventDefault();
      e.dataTransfer.dropEffect = e.dataTransfer.types.includes("application/seq-reorder")
        ? "move"
        : "copy";
      setDragOverIdx(idx);
    }
  };

  const handleItemDrop = (e: React.DragEvent, idx: number) => {
    e.preventDefault();
    setDragOverIdx(null);
    setDragSrcIdx(null);

    const reorderData = e.dataTransfer.getData("application/seq-reorder");
    if (reorderData) {
      const srcIdx = parseInt(reorderData);
      if (srcIdx === idx) return;
      setSceneFiles((prev) => {
        const next = [...prev];
        const [removed] = next.splice(srcIdx, 1);
        next.splice(srcIdx < idx ? idx - 1 : idx, 0, removed);
        return next;
      });
      setIsDirty(true);
      return;
    }

    const ffpyPath = e.dataTransfer.getData("application/ffpy-path");
    if (ffpyPath) {
      setSceneFiles((prev) => {
        const next = [...prev];
        next.splice(idx, 0, ffpyPath);
        return next;
      });
      setIsDirty(true);
    }
  };

  // ── Drop zone (append at end) ──────────────────────────────────────────────

  const handleDropZoneDragOver = (e: React.DragEvent) => {
    if (e.dataTransfer.types.includes("application/ffpy-path")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      setDropZoneActive(true);
    }
  };

  const handleDropZoneDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setDropZoneActive(false);
    const ffpyPath = e.dataTransfer.getData("application/ffpy-path");
    if (ffpyPath) {
      setSceneFiles((prev) => [...prev, ffpyPath]);
      setIsDirty(true);
    }
  };

  // ── Rendering ──────────────────────────────────────────────────────────────

  const runScene = (filePath: string): Promise<SequenceSceneResult> => {
    return new Promise((resolve, reject) => {
      let frameCount = 0;
      let cueFrames: number[] = [];
      let treeDone = false;

      const handler = (msg: ServerMsg) => {
        if (msg.type === "output") {
          setLines((prev) => [...prev, { kind: "out", text: msg.text }]);
        } else if (msg.type === "error") {
          setLines((prev) => [...prev, { kind: "err", text: msg.text }]);
        } else if (msg.type === "tree") {
          frameCount = msg.frame_count;
          cueFrames = msg.cue_frames ?? [];
          treeDone = true;
        } else if (msg.type === "done") {
          if (!treeDone) {
            setWsOverride(null);
            reject(new Error(`No animation data produced by ${filePath}`));
            return;
          }

          const v = Date.now();

          const fetchScene = fetch(withToken(`/tree/0?v=${v}`)).then((r) =>
            r.ok ? (r.json() as Promise<SceneData>) : Promise.reject("tree request failed"),
          );

          fetchScene
            .then((scene) => {
              // Compute render scale to match the player canvas area
              const containerSize = getPlayerAreaSize();
              const dpr = window.devicePixelRatio || 1;
              let serverScale = dpr; // default: 1 CSS pixel = 1 PNG pixel at dpr scale
              if (
                containerSize &&
                containerSize.width > 0 &&
                containerSize.height > 0 &&
                scene.width > 0 &&
                scene.height > 0
              ) {
                const cssScale = Math.min(
                  containerSize.width / scene.width,
                  containerSize.height / scene.height,
                );
                serverScale = cssScale * dpr;
              }
              serverScale = Math.max(0.01, Math.min(256, serverScale));
              const pngWidth = Math.round(scene.width * serverScale);
              const pngHeight = Math.round(scene.height * serverScale);

              const fetchFrames =
                frameCount > 0
                  ? fetch(
                      withToken(
                        `/frames?from=0&to=${frameCount - 1}&scale=${serverScale.toFixed(3)}&v=${v}`,
                      ),
                    ).then((r) =>
                      r.ok
                        ? (r.json() as Promise<{ n: number; png: string }[]>)
                        : Promise.reject("frames request failed"),
                    )
                  : Promise.resolve([] as { n: number; png: string }[]);

              fetchFrames
                .then((frameData) => {
                  const frames = new Array<string>(frameCount).fill("");
                  for (const { n, png } of frameData) {
                    const bytes = Uint8Array.from(atob(png), (c) => c.charCodeAt(0));
                    const blob = new Blob([bytes], { type: "image/png" });
                    frames[n] = URL.createObjectURL(blob);
                  }
                  setWsOverride(null);
                  resolve({
                    path: filePath,
                    frameCount,
                    cueFrames,
                    frames,
                    width: scene.width,
                    height: scene.height,
                    pngWidth,
                    pngHeight,
                  });
                })
                .catch((err) => {
                  setWsOverride(null);
                  reject(err);
                });
            })
            .catch((err) => {
              setWsOverride(null);
              reject(err);
            });
        }
      };

      setWsOverride(handler);
      wsRef.current?.send(JSON.stringify({ type: "run", path: filePath }));
    });
  };

  const handleRender = async () => {
    if (isRendering || sceneFiles.length === 0) return;
    await save(sceneFiles);
    setIsRendering(true);
    setRunning(true);
    setLines([]);

    const sceneResults: SequenceSceneResult[] = [];

    for (let i = 0; i < sceneFiles.length; i++) {
      const filePath = sceneFiles[i];
      setRenderStatus(`Rendering ${i + 1} / ${sceneFiles.length}: ${filePath}`);
      try {
        const result = await runScene(filePath);
        sceneResults.push(result);
      } catch (err) {
        setLines((prev) => [...prev, { kind: "err", text: `Failed: ${err}` }]);
        setIsRendering(false);
        setRunning(false);
        setRenderStatus("");
        return;
      }
    }

    // Build global cue frames — include 0 and last frame as cues
    let offset = 0;
    const globalCues: number[] = [];
    for (const sr of sceneResults) {
      for (const cf of sr.cueFrames) globalCues.push(offset + cf);
      offset += sr.frameCount;
    }
    const totalFrames = offset;
    if (!globalCues.includes(0)) globalCues.unshift(0);
    if (totalFrames > 0 && !globalCues.includes(totalFrames - 1)) globalCues.push(totalFrames - 1);
    globalCues.sort((a, b) => a - b);

    onRenderResult({
      scenes: sceneResults,
      totalFrames,
      globalCueFrames: [...new Set(globalCues)],
    });

    setLines((prev) => [
      ...prev,
      {
        kind: "sys",
        text: `Sequence rendered: ${sceneFiles.length} scene(s), ${totalFrames} frame(s) total`,
      },
    ]);
    setIsRendering(false);
    setRunning(false);
    setRenderStatus("");
  };

  // ── Export to player ───────────────────────────────────────────────────────

  const openExportDialog = () => {
    const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
    const suggested = base.endsWith(".ffsq") ? base.slice(0, -5) + ".ffpkg" : base + ".ffpkg";
    setExportFilename(suggested);
    setExportStatus("idle");
    setExportError("");
    setExportResultPath("");
    setExportOpen(true);
  };

  const doExport = async () => {
    setExportStatus("exporting");
    try {
      const res = await fetch(withToken("/export-player"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ seq_path: path, filename: exportFilename, fps }),
      });
      const data = await res.json();
      if (data.success) {
        setExportResultPath(data.path ?? exportFilename);
        setExportStatus("done");
        onExportDone?.();
      } else {
        setExportError(data.error ?? "Unknown error");
        setExportStatus("error");
      }
    } catch (e) {
      setExportError(String(e));
      setExportStatus("error");
    }
  };

  // ── Export as video ───────────────────────────────────────────────────────

  const openVideoExportDialog = () => {
    const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
    const suggested = base.endsWith(".ffsq") ? base.slice(0, -5) + ".mp4" : base + ".mp4";
    setVideoFilename(suggested);
    setVideoWidth(0);
    setVideoHeight(0);
    setVideoFps(fps);
    setVideoCodec("h264");
    setVideoCrf(23);
    setVideoExportStatus("idle");
    setVideoExportProgress(0);
    setVideoExportMessage("");
    setVideoExportDonePath("");
    setVideoExportError("");
    setVideoExportOpen(true);
  };

  const handleVideoCodecChange = (codec: "h264" | "h265" | "vp9") => {
    setVideoCodec(codec);
    const ext = codec === "vp9" ? ".webm" : ".mp4";
    setVideoFilename((prev) => prev.replace(/\.(mp4|webm)$/i, "") + ext);
  };

  const doVideoExport = async () => {
    if (videoExportStatus === "exporting") return;
    setVideoExportStatus("exporting");
    setVideoExportProgress(0);
    setVideoExportMessage("Rendering frames…");
    setVideoExportDonePath("");
    setVideoExportError("");

    let finished = false;
    try {
      const response = await fetch(withToken("/export-seq-video"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          seq_path: path,
          filename: videoFilename,
          fps: videoFps,
          codec: videoCodec,
          crf: videoCrf,
          width: videoWidth,
          height: videoHeight,
        }),
      });
      if (!response.ok || !response.body) {
        setVideoExportStatus("error");
        setVideoExportError(`Server error: ${response.status}`);
        return;
      }

      const reader = response.body.getReader();
      const decoder = new TextDecoder();
      let buf = "";
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        buf += decoder.decode(value, { stream: true });
        const lines = buf.split("\n");
        buf = lines.pop() ?? "";
        for (const line of lines) {
          if (!line.startsWith("data: ")) continue;
          try {
            const ev = JSON.parse(line.slice(6));
            if (ev.type === "progress") {
              setVideoExportProgress(Math.round((ev.done / ev.total) * 100));
              setVideoExportMessage(`Rendering frames… ${ev.done}/${ev.total}`);
            } else if (ev.type === "ffmpeg") {
              setVideoExportProgress(100);
              setVideoExportMessage("Encoding video…");
            } else if (ev.type === "done") {
              finished = true;
              setVideoExportStatus("done");
              setVideoExportDonePath(ev.path);
            } else if (ev.type === "error") {
              finished = true;
              setVideoExportStatus("error");
              setVideoExportError(ev.message);
            }
          } catch {
            /* ignore malformed lines */
          }
        }
      }
      if (!finished) {
        setVideoExportStatus("error");
        setVideoExportError("Export ended unexpectedly.");
      }
    } catch (e) {
      setVideoExportStatus("error");
      setVideoExportError(String(e));
    }
  };

  // ── Export to PDF ──────────────────────────────────────────────────────────

  const openPdfExportDialog = () => {
    const base = path.replace(/\\/g, "/").split("/").pop() ?? path;
    const suggested = base.endsWith(".ffsq") ? base.slice(0, -5) + ".pdf" : base + ".pdf";
    setPdfFilename(suggested);
    setPdfExportStatus("idle");
    setPdfExportError("");
    setPdfExportResultPath("");
    setPdfExportOpen(true);
  };

  const doPdfExport = async () => {
    setPdfExportStatus("exporting");
    try {
      const res = await fetch(withToken("/export-pdf"), {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          seq_path: path,
          filename: pdfFilename,
          fps,
          frame_selection: pdfFrameSelection,
        }),
      });
      const data = await res.json();
      if (data.success) {
        setPdfExportResultPath(data.path ?? pdfFilename);
        setPdfExportStatus("done");
      } else {
        setPdfExportError(data.error ?? "Unknown error");
        setPdfExportStatus("error");
      }
    } catch (e) {
      setPdfExportError(String(e));
      setPdfExportStatus("error");
    }
  };

  // ── External render trigger (e.g. fullscreen resize) ──────────────────────
  const handleRenderRef = useRef(handleRender);
  handleRenderRef.current = handleRender;
  useEffect(() => {
    if (renderTrigger && renderTrigger > 0) handleRenderRef.current();
  }, [renderTrigger]);

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div className="seq-editor">
      <div className="seq-editor-header">
        <span className="seq-editor-title">Sequence</span>
        <div className="seq-editor-actions">
          {isDirty && (
            <button className="seq-btn" onClick={() => save(sceneFiles)}>
              Save
            </button>
          )}
          <button
            className={`seq-btn seq-btn-render${isRendering ? " seq-btn-busy" : ""}`}
            onClick={handleRender}
            disabled={isRendering || sceneFiles.length === 0}
          >
            {isRendering ? "…" : "▶ Render"}
          </button>
          <button
            className="seq-btn"
            onClick={openExportDialog}
            disabled={isRendering || sceneFiles.length === 0}
          >
            Export to Player
          </button>
          <button
            className="seq-btn"
            onClick={openPdfExportDialog}
            disabled={isRendering || sceneFiles.length === 0}
          >
            Export to PDF
          </button>
          <button
            className="seq-btn"
            onClick={openVideoExportDialog}
            disabled={isRendering || sceneFiles.length === 0}
          >
            Export as Video
          </button>
        </div>
      </div>

      {isRendering && renderStatus && <div className="seq-render-status">{renderStatus}</div>}

      <div className="seq-list-container">
        {sceneFiles.length === 0 ? (
          <div
            className={`seq-empty${dropZoneActive ? " seq-empty-active" : ""}`}
            onDragOver={handleDropZoneDragOver}
            onDragLeave={() => setDropZoneActive(false)}
            onDrop={handleDropZoneDrop}
          >
            Drag .ffpy files from the explorer to add scenes
          </div>
        ) : (
          <div
            className="seq-list"
            onDragLeave={(e) => {
              if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragOverIdx(null);
            }}
          >
            {sceneFiles.map((file, idx) => (
              <div
                key={`${idx}-${file}`}
                className={`seq-item${dragOverIdx === idx ? " seq-item-over" : ""}${dragSrcIdx === idx ? " seq-item-dragging" : ""}`}
                onDragOver={(e) => handleItemDragOver(e, idx)}
                onDragLeave={(e) => {
                  if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragOverIdx(null);
                }}
                onDrop={(e) => handleItemDrop(e, idx)}
              >
                <span
                  className="seq-item-handle"
                  draggable
                  onDragStart={(e) => handleItemDragStart(e, idx)}
                  onDragEnd={handleItemDragEnd}
                  title="Drag to reorder"
                >
                  ⠿
                </span>
                <span className="seq-item-num">{idx + 1}</span>
                <span className="seq-item-name" title={file}>
                  {file}
                </span>
                <button className="seq-item-remove" onClick={() => removeItem(idx)} title="Remove">
                  ×
                </button>
              </div>
            ))}

            <div
              className={`seq-drop-zone${dropZoneActive ? " seq-drop-zone-active" : ""}`}
              onDragOver={handleDropZoneDragOver}
              onDragLeave={() => setDropZoneActive(false)}
              onDrop={handleDropZoneDrop}
            >
              Drop .ffpy to append
            </div>
          </div>
        )}
      </div>

      {exportOpen && (
        <div
          className="seq-dialog-backdrop"
          onClick={() => {
            if (exportStatus !== "exporting") setExportOpen(false);
          }}
        >
          <div className="seq-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="seq-dialog-title">Export to Player</div>

            {exportStatus === "idle" && (
              <>
                <div className="seq-dialog-body">
                  <label className="seq-dialog-label">Output filename</label>
                  <input
                    className="seq-dialog-input"
                    value={exportFilename}
                    onChange={(e) => setExportFilename(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") doExport();
                      if (e.key === "Escape") setExportOpen(false);
                    }}
                    autoFocus
                    spellCheck={false}
                  />
                </div>
                <div className="seq-dialog-footer">
                  <button className="seq-btn" onClick={() => setExportOpen(false)}>
                    Cancel
                  </button>
                  <button
                    className="seq-btn seq-btn-render"
                    onClick={doExport}
                    disabled={!exportFilename.trim()}
                  >
                    Export
                  </button>
                </div>
              </>
            )}

            {exportStatus === "exporting" && (
              <div className="seq-dialog-body seq-dialog-status">Exporting package…</div>
            )}

            {exportStatus === "done" && (
              <>
                <div className="seq-dialog-body seq-dialog-status seq-dialog-ok">
                  Package created: <span className="seq-dialog-path">{exportResultPath}</span>
                </div>
                <div className="seq-dialog-footer">
                  <button className="seq-btn seq-btn-render" onClick={() => setExportOpen(false)}>
                    Close
                  </button>
                </div>
              </>
            )}

            {exportStatus === "error" && (
              <>
                <div className="seq-dialog-body seq-dialog-status seq-dialog-err">
                  {exportError}
                </div>
                <div className="seq-dialog-footer">
                  <button className="seq-btn" onClick={() => setExportStatus("idle")}>
                    Back
                  </button>
                  <button className="seq-btn" onClick={() => setExportOpen(false)}>
                    Close
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
      {videoExportOpen && (
        <div
          className="modal-overlay"
          onClick={() => {
            if (videoExportStatus !== "exporting") setVideoExportOpen(false);
          }}
        >
          <div className="export-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="export-dialog-header">
              <span>Export as Video</span>
              <button
                className="export-dialog-close"
                onClick={() => setVideoExportOpen(false)}
                disabled={videoExportStatus === "exporting"}
              >
                ✕
              </button>
            </div>

            <div className="export-dialog-body">
              <div className="export-row">
                <label>Resolution</label>
                <span className="export-wh">
                  <input
                    type="number"
                    className="export-num"
                    value={videoWidth}
                    min={0}
                    onChange={(e) => setVideoWidth(Number(e.target.value))}
                  />
                  <span>×</span>
                  <input
                    type="number"
                    className="export-num"
                    value={videoHeight}
                    min={0}
                    onChange={(e) => setVideoHeight(Number(e.target.value))}
                  />
                </span>
                <span className="export-hint">0 = original</span>
              </div>

              <div className="export-row">
                <label>FPS</label>
                <input
                  type="number"
                  className="export-num"
                  value={videoFps}
                  min={1}
                  max={120}
                  onChange={(e) => setVideoFps(Number(e.target.value))}
                />
              </div>

              <div className="export-row">
                <label>Codec</label>
                <select
                  value={videoCodec}
                  onChange={(e) => handleVideoCodecChange(e.target.value as typeof videoCodec)}
                >
                  <option value="h264">H.264 (.mp4)</option>
                  <option value="h265">H.265 (.mp4)</option>
                  <option value="vp9">VP9 (.webm)</option>
                </select>
              </div>

              <div className="export-row">
                <label>Quality (CRF {videoCrf})</label>
                <input
                  type="range"
                  className="export-slider"
                  min={0}
                  max={51}
                  value={videoCrf}
                  onChange={(e) => setVideoCrf(Number(e.target.value))}
                />
                <span className="export-hint">lower = better</span>
              </div>

              <div className="export-row">
                <label>Filename</label>
                <input
                  type="text"
                  className="export-text"
                  value={videoFilename}
                  onChange={(e) => setVideoFilename(e.target.value)}
                />
              </div>
            </div>

            {videoExportStatus === "exporting" && (
              <div className="export-progress-area">
                <div className="export-progress-label">{videoExportMessage}</div>
                <div className="export-progress-track">
                  <div
                    className="export-progress-fill"
                    style={{ width: `${videoExportProgress}%` }}
                  />
                </div>
              </div>
            )}

            {videoExportStatus === "done" && (
              <div className="export-result export-result-ok">
                Saved to <code>{videoExportDonePath}</code>
              </div>
            )}

            {videoExportStatus === "error" && (
              <div className="export-result export-result-err">{videoExportError}</div>
            )}

            <div className="export-dialog-footer">
              <button
                className="export-btn-cancel"
                onClick={() => setVideoExportOpen(false)}
                disabled={videoExportStatus === "exporting"}
              >
                {videoExportStatus === "done" || videoExportStatus === "error" ? "Close" : "Cancel"}
              </button>
              <button
                className="export-btn-ok"
                onClick={doVideoExport}
                disabled={videoExportStatus === "exporting" || !videoFilename.trim()}
              >
                {videoExportStatus === "exporting" ? "Exporting…" : "Export"}
              </button>
            </div>
          </div>
        </div>
      )}
      {pdfExportOpen && (
        <div
          className="seq-dialog-backdrop"
          onClick={() => {
            if (pdfExportStatus !== "exporting") setPdfExportOpen(false);
          }}
        >
          <div className="seq-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="seq-dialog-title">Export to PDF</div>

            {pdfExportStatus === "idle" && (
              <>
                <div className="seq-dialog-body">
                  <label className="seq-dialog-label">Output filename</label>
                  <input
                    className="seq-dialog-input"
                    value={pdfFilename}
                    onChange={(e) => setPdfFilename(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") doPdfExport();
                      if (e.key === "Escape") setPdfExportOpen(false);
                    }}
                    autoFocus
                    spellCheck={false}
                  />
                  <label className="seq-dialog-label" style={{ marginTop: 10 }}>
                    Frames to export
                  </label>
                  <div className="seq-dialog-radio-group">
                    <label className="seq-dialog-radio-label">
                      <input
                        type="radio"
                        name="pdfFrameSelection"
                        value="cue_frames"
                        checked={pdfFrameSelection === "cue_frames"}
                        onChange={() => setPdfFrameSelection("cue_frames")}
                      />
                      Cue frames
                    </label>
                    <label className="seq-dialog-radio-label">
                      <input
                        type="radio"
                        name="pdfFrameSelection"
                        value="cue_plus_key_frames"
                        checked={pdfFrameSelection === "cue_plus_key_frames"}
                        onChange={() => setPdfFrameSelection("cue_plus_key_frames")}
                      />
                      Cue + key frames
                    </label>
                    <label className="seq-dialog-radio-label">
                      <input
                        type="radio"
                        name="pdfFrameSelection"
                        value="all_frames"
                        checked={pdfFrameSelection === "all_frames"}
                        onChange={() => setPdfFrameSelection("all_frames")}
                      />
                      All frames
                    </label>
                  </div>
                </div>
                <div className="seq-dialog-footer">
                  <button className="seq-btn" onClick={() => setPdfExportOpen(false)}>
                    Cancel
                  </button>
                  <button
                    className="seq-btn seq-btn-render"
                    onClick={doPdfExport}
                    disabled={!pdfFilename.trim()}
                  >
                    Export
                  </button>
                </div>
              </>
            )}

            {pdfExportStatus === "exporting" && (
              <div className="seq-dialog-body seq-dialog-status">Exporting PDF…</div>
            )}

            {pdfExportStatus === "done" && (
              <>
                <div className="seq-dialog-body seq-dialog-status seq-dialog-ok">
                  PDF created: <span className="seq-dialog-path">{pdfExportResultPath}</span>
                </div>
                <div className="seq-dialog-footer">
                  <button
                    className="seq-btn seq-btn-render"
                    onClick={() => setPdfExportOpen(false)}
                  >
                    Close
                  </button>
                </div>
              </>
            )}

            {pdfExportStatus === "error" && (
              <>
                <div className="seq-dialog-body seq-dialog-status seq-dialog-err">
                  {pdfExportError}
                </div>
                <div className="seq-dialog-footer">
                  <button className="seq-btn" onClick={() => setPdfExportStatus("idle")}>
                    Back
                  </button>
                  <button className="seq-btn" onClick={() => setPdfExportOpen(false)}>
                    Close
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
