import { useState, useEffect, useRef, useCallback } from 'react';
import type { SequenceRenderResult } from '../types';

interface Props {
  result: SequenceRenderResult;
  fps: number;
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
  return { sceneIdx: last, localFrame: (result.scenes[last]?.frameCount ?? 1) - 1 };
}

export default function SequencePlayer({ result, fps }: Props) {
  const [frame, setFrame] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const playIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const totalFrames = result.totalFrames;
  const maxFrame = Math.max(0, totalFrames - 1);

  const { sceneIdx, localFrame } = globalToScene(result, frame);
  const currentScene = result.scenes[sceneIdx];
  const imgSrc = currentScene?.frames[localFrame] ?? '';

  const cueFrames = result.globalCueFrames;
  const prevCue = cueFrames.filter(c => c < frame).at(-1) ?? null;
  const nextCue = cueFrames.find(c => c > frame) ?? null;

  // Scene boundary markers for the slider
  const sceneBoundaries: number[] = [];
  let bOffset = 0;
  for (let i = 0; i < result.scenes.length - 1; i++) {
    bOffset += result.scenes[i].frameCount;
    sceneBoundaries.push(bOffset);
  }

  const gotoPrevCue = useCallback(() => {
    if (prevCue !== null) setFrame(prevCue);
  }, [prevCue]);

  const gotoNextCue = useCallback(() => {
    if (nextCue !== null) setFrame(nextCue);
  }, [nextCue]);

  // Keyboard handler — Left = prev cue, Right = next cue
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.key === 'ArrowLeft') { e.preventDefault(); gotoPrevCue(); }
      if (e.key === 'ArrowRight') { e.preventDefault(); gotoNextCue(); }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [gotoPrevCue, gotoNextCue]);

  // Fullscreen change listener
  useEffect(() => {
    const handler = () => setIsFullscreen(!!document.fullscreenElement);
    document.addEventListener('fullscreenchange', handler);
    return () => document.removeEventListener('fullscreenchange', handler);
  }, []);

  // Play/stop
  const handlePlayStop = () => {
    if (isPlaying) {
      clearInterval(playIntervalRef.current!);
      playIntervalRef.current = null;
      setIsPlaying(false);
      return;
    }
    setIsPlaying(true);
    let f = frame >= maxFrame ? 0 : frame;
    playIntervalRef.current = setInterval(() => {
      f++;
      if (f > maxFrame) {
        clearInterval(playIntervalRef.current!);
        playIntervalRef.current = null;
        setIsPlaying(false);
        setFrame(maxFrame);
      } else {
        setFrame(f);
      }
    }, 1000 / Math.max(1, fps));
  };

  useEffect(() => {
    return () => { if (playIntervalRef.current) clearInterval(playIntervalRef.current); };
  }, []);

  // Reset when result changes
  useEffect(() => {
    setFrame(0);
    setIsPlaying(false);
    if (playIntervalRef.current) { clearInterval(playIntervalRef.current); playIntervalRef.current = null; }
  }, [result]);

  const enterFullscreen = () => containerRef.current?.requestFullscreen?.();
  const exitFullscreen = () => document.exitFullscreen?.();
  const toggleFullscreen = isFullscreen ? exitFullscreen : enterFullscreen;

  return (
    <div ref={containerRef} className="seqp">
      {/* Header */}
      <div className="seqp-header">
        <span className="seqp-scene-label">
          <span className="seqp-scene-num">{sceneIdx + 1}/{result.scenes.length}</span>
          <span className="seqp-scene-path">{currentScene?.path ?? ''}</span>
        </span>
        <div className="seqp-header-actions">
          <button className="seqp-action-btn seqp-fullscreen-btn" onClick={toggleFullscreen}
            title={isFullscreen ? 'Exit fullscreen (Esc)' : 'Fullscreen'}>
            {isFullscreen
              ? <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor"><path d="M2 5H0V0h5v2H2v3zM9 2V0h5v5h-2V2H9zM0 9h2v3h3v2H0V9zM12 9h2v5H9v-2h3V9z"/></svg>
              : <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor"><path d="M0 0h5v2H2v3H0V0zM9 0h5v5h-2V2H9V0zM0 9h2v3h3v2H0V9zM12 12H9v2h5V9h-2v3z"/></svg>
            }
          </button>
        </div>
      </div>

      {/* Canvas */}
      <div className="seqp-canvas-area">
        {imgSrc
          ? <img src={imgSrc} className="seqp-canvas-img" alt="" draggable={false} />
          : <div className="seqp-canvas-empty">No frames</div>
        }
      </div>

      {/* Controls */}
      <div className="seqp-controls">
        {/* Slider with cue/scene markers */}
        <div className="seqp-slider-wrap">
          <div className="seqp-slider-track">
            {cueFrames.map(cf => {
              const pct = maxFrame > 0 ? (cf / maxFrame) * 100 : 0;
              return <div key={`cue-${cf}`} className="seqp-cue-tick" style={{ left: `${pct}%` }} />;
            })}
            {sceneBoundaries.map(b => {
              const pct = maxFrame > 0 ? (b / maxFrame) * 100 : 0;
              return <div key={`sb-${b}`} className="seqp-scene-tick" style={{ left: `${pct}%` }} />;
            })}
          </div>
          <input
            type="range"
            min={0}
            max={maxFrame}
            value={frame}
            className="seqp-slider"
            onChange={e => {
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
            disabled={prevCue === null}
            title="Previous cue (←)"
          >◀ Prev</button>

          <button
            className="seqp-btn seqp-play-btn"
            onClick={handlePlayStop}
          >{isPlaying ? '■ Stop' : '▶ Play'}</button>

          <button
            className="seqp-btn"
            onClick={gotoNextCue}
            disabled={nextCue === null}
            title="Next cue (→)"
          >Next ▶</button>

          <span className="seqp-frame-label">{frame} / {maxFrame}</span>
        </div>
      </div>
    </div>
  );
}
