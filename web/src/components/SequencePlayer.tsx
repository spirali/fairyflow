import { useState, useEffect, useRef, useCallback, type RefObject } from "react";
import type { SequenceRenderResult } from "../types";
import { notesForSegment } from "../notes";

interface Props {
  result: SequenceRenderResult | null;
  fps: number;
  canvasAreaRef: RefObject<HTMLDivElement | null>;
  onRequestRerender?: () => void;
  isRendering?: boolean;
}

function globalToScene(result: SequenceRenderResult, frame: number) {
  let offset = 0;
  for (let i = 0; i < result.scenes.length; i++) {
    const sc = result.scenes[i];
    if (frame < offset + sc.frameCount) {
      return { sceneIdx: i, localFrame: frame - offset };
    }
    offset += sc.frameCount;
  }
  const last = result.scenes.length - 1;
  return {
    sceneIdx: last,
    localFrame: (result.scenes[last]?.frameCount ?? 1) - 1,
  };
}

export default function SequencePlayer({
  result,
  fps,
  canvasAreaRef,
  onRequestRerender,
  isRendering = false,
}: Props) {
  const [frame, setFrame] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const [showNotes, setShowNotes] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const playIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const totalFrames = result?.totalFrames ?? 0;
  const maxFrame = Math.max(0, totalFrames - 1);

  const { sceneIdx, localFrame } = result
    ? globalToScene(result, frame)
    : { sceneIdx: 0, localFrame: 0 };
  const currentScene = result?.scenes[sceneIdx] ?? null;
  const imgSrc = currentScene?.frames[localFrame] ?? "";

  const cueFrames = result?.globalCueFrames ?? [];
  const prevCue = cueFrames.filter((c) => c < frame).at(-1) ?? null;
  const nextCue = cueFrames.find((c) => c > frame) ?? null;

  const currentNotes = currentScene
    ? notesForSegment(
        currentScene.cueFrames,
        currentScene.notes,
        localFrame,
        currentScene.frameCount,
      )
    : [];

  // Scene boundary markers for the slider
  const sceneBoundaries: number[] = [];
  let bOffset = 0;
  for (let i = 0; i < (result?.scenes.length ?? 0) - 1; i++) {
    bOffset += result!.scenes[i].frameCount;
    sceneBoundaries.push(bOffset);
  }

  // CSS display size: render at server-specified PNG size, divided by DPR
  const dpr = window.devicePixelRatio || 1;
  const imgCssWidth = currentScene ? currentScene.pngWidth / dpr : undefined;
  const imgCssHeight = currentScene ? currentScene.pngHeight / dpr : undefined;

  const gotoPrevCue = useCallback(() => {
    if (prevCue !== null) setFrame(prevCue);
  }, [prevCue]);

  const gotoNextCue = useCallback(() => {
    if (nextCue !== null) setFrame(nextCue);
  }, [nextCue]);

  // Refs so the keyboard handler never needs re-registering as state changes
  const isPlayingRef = useRef(isPlaying);
  isPlayingRef.current = isPlaying;
  const gotoPrevCueRef = useRef(gotoPrevCue);
  gotoPrevCueRef.current = gotoPrevCue;
  const gotoNextCueRef = useRef(gotoNextCue);
  gotoNextCueRef.current = gotoNextCue;
  const handlePlayStopRef = useRef<() => void>(() => {});

  // Keyboard handler — PageUp = prev cue, PageDown = next cue, ArrowRight = play/resume
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "PageUp") {
        e.preventDefault();
        gotoPrevCueRef.current();
      }
      if (e.key === "PageDown") {
        e.preventDefault();
        gotoNextCueRef.current();
      }
      if (e.key === "ArrowRight" && !isPlayingRef.current) {
        e.preventDefault();
        handlePlayStopRef.current();
      }
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

  // Keep a ref so the fullscreen handler can read the latest result/callback without
  // needing them as effect dependencies (avoids re-registering the listener).
  const resultRef = useRef(result);
  resultRef.current = result;
  const onRequestRerenderRef = useRef(onRequestRerender);
  onRequestRerenderRef.current = onRequestRerender;

  // Fullscreen change listener — also triggers re-render so frames match the new size
  useEffect(() => {
    let rerenderTimer: ReturnType<typeof setTimeout> | null = null;
    const handler = () => {
      setIsFullscreen(!!document.fullscreenElement);
      if (resultRef.current) {
        // Wait for the browser to finish the fullscreen layout transition
        // before measuring the canvas area and re-rendering.
        rerenderTimer = setTimeout(() => {
          onRequestRerenderRef.current?.();
        }, 100);
      }
    };
    document.addEventListener("fullscreenchange", handler);
    return () => {
      document.removeEventListener("fullscreenchange", handler);
      if (rerenderTimer !== null) clearTimeout(rerenderTimer);
    };
  }, []);

  // Play/stop — always stops at cue frames; ArrowRight resumes from current position
  const handlePlayStop = () => {
    if (isPlaying) {
      clearInterval(playIntervalRef.current!);
      playIntervalRef.current = null;
      setIsPlaying(false);
      return;
    }
    setIsPlaying(true);
    let f = frame >= maxFrame ? 0 : frame;
    const capturedPauseFrames = result?.globalPauseFrames ?? [];
    const stop = (at: number) => {
      clearInterval(playIntervalRef.current!);
      playIntervalRef.current = null;
      setIsPlaying(false);
      setFrame(at);
    };
    playIntervalRef.current = setInterval(
      () => {
        f++;
        if (capturedPauseFrames.includes(f)) {
          stop(f);
        } else if (f > maxFrame) {
          stop(maxFrame);
        } else {
          setFrame(f);
        }
      },
      1000 / Math.max(1, fps),
    );
  };
  handlePlayStopRef.current = handlePlayStop;

  useEffect(() => {
    return () => {
      if (playIntervalRef.current) clearInterval(playIntervalRef.current);
    };
  }, []);

  // Reset when result changes (including to null)
  useEffect(() => {
    setFrame(0);
    setIsPlaying(false);
    if (playIntervalRef.current) {
      clearInterval(playIntervalRef.current);
      playIntervalRef.current = null;
    }
  }, [result]);

  const enterFullscreen = () => containerRef.current?.requestFullscreen?.();
  const exitFullscreen = () => document.exitFullscreen?.();
  const toggleFullscreen = isFullscreen ? exitFullscreen : enterFullscreen;

  return (
    <div ref={containerRef} className="seqp">
      {/* Header */}
      <div className="seqp-header">
        <span className="seqp-scene-label">
          {result ? (
            <>
              <span className="seqp-scene-num">
                {sceneIdx + 1}/{result.scenes.length}
              </span>
              <span className="seqp-scene-path">{currentScene?.path ?? ""}</span>
            </>
          ) : (
            <span className="seqp-scene-path seqp-scene-path--placeholder">
              Press Render to preview the sequence
            </span>
          )}
        </span>
        <div className="seqp-header-actions">
          <button
            className="seqp-action-btn seqp-notes-btn"
            onClick={() => setShowNotes((v) => !v)}
            title={showNotes ? "Hide speaker notes" : "Show speaker notes"}
            disabled={!result}
            aria-pressed={showNotes}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
              <path d="M1 1h12v8H4l-3 3V9H1V1zm2 2v1h8V3H3zm0 3v1h6V6H3z" />
            </svg>
          </button>
          <button
            className="seqp-action-btn seqp-fullscreen-btn"
            onClick={toggleFullscreen}
            title={isFullscreen ? "Exit fullscreen (Esc)" : "Fullscreen"}
            disabled={!result}
          >
            {isFullscreen ? (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
                <path d="M2 5H0V0h5v2H2v3zM9 2V0h5v5h-2V2H9zM0 9h2v3h3v2H0V9zM12 9h2v5H9v-2h3V9z" />
              </svg>
            ) : (
              <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor">
                <path d="M0 0h5v2H2v3H0V0zM9 0h5v5h-2V2H9V0zM0 9h2v3h3v2H0V9zM12 12H9v2h5V9h-2v3z" />
              </svg>
            )}
          </button>
        </div>
      </div>

      {/* Canvas */}
      <div ref={canvasAreaRef} className="seqp-canvas-area">
        {imgSrc ? (
          <img
            src={imgSrc}
            className="seqp-canvas-img"
            alt=""
            draggable={false}
            style={imgCssWidth != null ? { width: imgCssWidth, height: imgCssHeight } : undefined}
          />
        ) : (
          <div className="seqp-canvas-empty">{result ? "No frames" : ""}</div>
        )}
        {isRendering && result && (
          <div className="seqp-rendering-overlay">
            <div className="seqp-rendering-spinner" />
          </div>
        )}
      </div>

      {/* Controls */}
      <div className="seqp-controls">
        {/* Slider with cue/scene markers */}
        <div className="seqp-slider-wrap">
          <div className="seqp-slider-track">
            {cueFrames.map((cf) => {
              const pct = maxFrame > 0 ? (cf / maxFrame) * 100 : 0;
              return (
                <div key={`cue-${cf}`} className="seqp-cue-tick" style={{ left: `${pct}%` }} />
              );
            })}
            {sceneBoundaries.map((b) => {
              const pct = maxFrame > 0 ? (b / maxFrame) * 100 : 0;
              return (
                <div key={`sb-${b}`} className="seqp-scene-tick" style={{ left: `${pct}%` }} />
              );
            })}
          </div>
          <input
            type="range"
            min={0}
            max={maxFrame}
            value={frame}
            className="seqp-slider"
            disabled={!result}
            onChange={(e) => {
              if (isPlaying) {
                clearInterval(playIntervalRef.current!);
                playIntervalRef.current = null;
                setIsPlaying(false);
              }
              setFrame(Number(e.target.value));
            }}
          />
        </div>

        {/* Buttons row */}
        <div className="seqp-btns">
          <button
            className="seqp-btn"
            onClick={gotoPrevCue}
            disabled={prevCue === null || !result}
            title="Previous cue (Page Up)"
          >
            ◀ Prev
          </button>

          <button className="seqp-btn seqp-play-btn" onClick={handlePlayStop} disabled={!result}>
            {isPlaying ? "■ Stop" : "▶ Play"}
          </button>

          <button
            className="seqp-btn"
            onClick={gotoNextCue}
            disabled={nextCue === null || !result}
            title="Next cue (Page Down)"
          >
            Next ▶
          </button>

          <span className="seqp-frame-label">{result ? `${frame} / ${maxFrame}` : "—"}</span>
        </div>
      </div>

      {/* Speaker notes for the current segment */}
      {showNotes && result && (
        <div className="seqp-notes-panel">
          {currentNotes.length > 0 ? (
            currentNotes.map((text, i) => (
              <p key={i} className="seqp-notes-para">
                {text}
              </p>
            ))
          ) : (
            <p className="seqp-notes-empty">No notes for this segment.</p>
          )}
        </div>
      )}
    </div>
  );
}
