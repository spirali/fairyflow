use crate::service::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use engine::{FrameId, SceneSelection};
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Deserialize)]
pub struct ExportParams {
    /// 0 means use native scene resolution
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    pub filename: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    /// "h264", "h265", or "vp9"
    #[serde(default = "default_codec")]
    pub codec: String,
    #[serde(default = "default_crf")]
    pub crf: u32,
    pub from_frame: Option<u32>,
    pub to_frame: Option<u32>,
    /// Which scene to export. None = all scenes concatenated.
    pub scene_index: Option<u32>,
}

fn default_fps() -> u32 { 24 }
fn default_codec() -> String { "h264".into() }
fn default_crf() -> u32 { 23 }

pub async fn export_handler(
    State(state): State<AppState>,
    Json(params): Json<ExportParams>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    tokio::spawn(do_export(state, params, tx));

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|data| (Ok::<_, Infallible>(Event::default().data(data)), rx))
    });

    Sse::new(stream)
}

fn send(tx: &UnboundedSender<String>, msg: serde_json::Value) {
    let _ = tx.send(msg.to_string());
}

async fn do_export(state: AppState, params: ExportParams, tx: UnboundedSender<String>) {
    // ── animation must be loaded ─────────────────────────────────────────────
    let anim = match state.animation.lock().unwrap().as_ref().map(Arc::clone) {
        Some(a) => a,
        None => {
            send(&tx, serde_json::json!({"type":"error","message":"No animation loaded"}));
            return;
        }
    };

    // ── ffmpeg check ─────────────────────────────────────────────────────────
    let ffmpeg_ok = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);

    if !ffmpeg_ok {
        send(&tx, serde_json::json!({
            "type": "error",
            "message": "ffmpeg is not installed or not in PATH. Please install ffmpeg to use video export."
        }));
        return;
    }

    // ── frame range ──────────────────────────────────────────────────────────
    let sel = match params.scene_index {
        Some(i) => SceneSelection::Single(i as usize),
        None => SceneSelection::All,
    };
    let frame_count = anim.frame_count(sel);
    let last = frame_count.saturating_sub(1);
    let from = params.from_frame.unwrap_or(0).min(last);
    let to = params.to_frame.unwrap_or(last).min(last);
    if from > to {
        send(&tx, serde_json::json!({"type":"error","message": format!("Invalid frame range {from}–{to}")}));
        return;
    }
    let frames: Vec<u32> = (from..=to).collect();
    let total = frames.len() as u64;

    // ── temp directory ───────────────────────────────────────────────────────
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("fairyflow-export-{ts}"));
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        send(&tx, serde_json::json!({"type":"error","message": format!("Failed to create temp dir: {e}")}));
        return;
    }

    // ── render frames in parallel ────────────────────────────────────────────
    let target_res = if params.width > 0 && params.height > 0 {
        Some((params.width, params.height))
    } else {
        None
    };
    let done_count = Arc::new(AtomicU64::new(0));
    let tx2 = tx.clone();
    let temp_arc = Arc::new(temp_dir.clone());
    let frames_arc = Arc::new(frames);

    let render_result = tokio::task::spawn_blocking(move || {
        renderer_skia::clear_image_cache();
        use rayon::prelude::*;
        let done = done_count;
        frames_arc.par_iter().enumerate().try_for_each(|(idx, &frame_n)| -> Result<(), String> {
            let scene = anim.build_scene(FrameId::new(frame_n), sel).map_err(|e| e.to_string())?;
            let pixmap = match target_res {
                Some((w, h)) => renderer_skia::render_scene_fitted(&scene, w, h),
                None => renderer_skia::render_scene(&scene, 1.0),
            };
            let png = pixmap.encode_png().map_err(|e| e.to_string())?;
            std::fs::write(temp_arc.join(format!("frame{idx}.png")), &png)
                .map_err(|e| e.to_string())?;
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            let _ = tx2.send(serde_json::json!({"type":"progress","done":n,"total":total}).to_string());
            Ok(())
        })
    }).await;

    renderer_skia::prune_text_cache();

    match render_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            send(&tx, serde_json::json!({"type":"error","message": format!("Render failed: {e}")}));
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }
        Err(e) => {
            send(&tx, serde_json::json!({"type":"error","message": format!("Render task panicked: {e}")}));
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }
    }

    // ── prepare output path ──────────────────────────────────────────────────
    send(&tx, serde_json::json!({"type":"ffmpeg"}));

    if let Err(e) = tokio::fs::create_dir_all("exports").await {
        send(&tx, serde_json::json!({"type":"error","message": format!("Failed to create exports dir: {e}")}));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return;
    }
    let output_path = std::path::Path::new("exports").join(&params.filename);
    let output_str = output_path.to_string_lossy().into_owned();

    // ── run ffmpeg ───────────────────────────────────────────────────────────
    match crate::render::run_ffmpeg(&temp_dir, &output_path, params.fps, &params.codec, params.crf).await {
        Ok(()) => {
            send(&tx, serde_json::json!({"type":"done","path": output_str}));
        }
        Err(e) => {
            send(&tx, serde_json::json!({"type":"error","message": e}));
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}
