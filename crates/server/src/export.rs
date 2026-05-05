use crate::service::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use engine::{AnimationDef, FrameId, SceneSelection};
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

fn default_fps() -> u32 {
    24
}
fn default_codec() -> String {
    "h264".into()
}
fn default_crf() -> u32 {
    23
}

pub async fn export_handler(
    State(state): State<AppState>,
    Json(params): Json<ExportParams>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();

    tokio::spawn(do_export(state, params, tx));

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|data| (Ok::<_, Infallible>(Event::default().data(data)), rx))
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
            send(
                &tx,
                serde_json::json!({"type":"error","message":"No animation loaded"}),
            );
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
        send(
            &tx,
            serde_json::json!({
                "type": "error",
                "message": "ffmpeg is not installed or not in PATH. Please install ffmpeg to use video export."
            }),
        );
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
        send(
            &tx,
            serde_json::json!({"type":"error","message": format!("Invalid frame range {from}–{to}")}),
        );
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
        send(
            &tx,
            serde_json::json!({"type":"error","message": format!("Failed to create temp dir: {e}")}),
        );
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
        frames_arc
            .par_iter()
            .enumerate()
            .try_for_each(|(idx, &frame_n)| -> Result<(), String> {
                let scene = anim
                    .build_scene(FrameId::new(frame_n), sel)
                    .map_err(|e| e.to_string())?;
                let pixmap = match target_res {
                    Some((w, h)) => renderer_skia::render_scene_fitted(&scene, w, h),
                    None => renderer_skia::render_scene(&scene, 1.0),
                };
                let png = pixmap.encode_png().map_err(|e| e.to_string())?;
                std::fs::write(temp_arc.join(format!("frame{idx}.png")), &png)
                    .map_err(|e| e.to_string())?;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = tx2.send(
                    serde_json::json!({"type":"progress","done":n,"total":total}).to_string(),
                );
                Ok(())
            })
    })
    .await;

    renderer_skia::prune_text_cache();

    match render_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message": format!("Render failed: {e}")}),
            );
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }
        Err(e) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message": format!("Render task panicked: {e}")}),
            );
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return;
        }
    }

    // ── prepare output path ──────────────────────────────────────────────────
    send(&tx, serde_json::json!({"type":"ffmpeg"}));

    if let Err(e) = tokio::fs::create_dir_all("exports").await {
        send(
            &tx,
            serde_json::json!({"type":"error","message": format!("Failed to create exports dir: {e}")}),
        );
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return;
    }
    let output_path = std::path::Path::new("exports").join(&params.filename);
    let output_str = output_path.to_string_lossy().into_owned();

    // ── run ffmpeg ───────────────────────────────────────────────────────────
    match crate::render::run_ffmpeg(
        &temp_dir,
        &output_path,
        params.fps,
        &params.codec,
        params.crf,
    )
    .await
    {
        Ok(()) => {
            send(&tx, serde_json::json!({"type":"done","path": output_str}));
        }
        Err(e) => {
            send(&tx, serde_json::json!({"type":"error","message": e}));
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

// ── Sequence video export ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ExportSeqVideoParams {
    pub seq_path: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    pub filename: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default = "default_codec")]
    pub codec: String,
    #[serde(default = "default_crf")]
    pub crf: u32,
}

pub async fn export_seq_video_handler(
    State(state): State<AppState>,
    Json(params): Json<ExportSeqVideoParams>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(do_export_seq_video(state, params, tx));
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv()
            .await
            .map(|data| (Ok::<_, Infallible>(Event::default().data(data)), rx))
    });
    Sse::new(stream)
}

async fn do_export_seq_video(
    state: AppState,
    params: ExportSeqVideoParams,
    tx: UnboundedSender<String>,
) {
    // Validate filename
    let filename = params.filename.trim().to_string();
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        send(
            &tx,
            serde_json::json!({"type":"error","message":"Invalid filename"}),
        );
        return;
    }

    // Check ffmpeg
    let ffmpeg_ok = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    if !ffmpeg_ok {
        send(
            &tx,
            serde_json::json!({"type":"error","message":"ffmpeg is not installed or not in PATH. Please install ffmpeg to use video export."}),
        );
        return;
    }

    // Read and parse sequence file
    let seq_content = match tokio::fs::read_to_string(&params.seq_path).await {
        Ok(c) => c,
        Err(e) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Failed to read sequence: {e}")}),
            );
            return;
        }
    };
    let seq_data: serde_json::Value = match serde_json::from_str(&seq_content) {
        Ok(v) => v,
        Err(e) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Failed to parse sequence: {e}")}),
            );
            return;
        }
    };
    let scene_files: Vec<String> = match seq_data.get("scene_files").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        None => {
            send(
                &tx,
                serde_json::json!({"type":"error","message":"Sequence has no scene_files"}),
            );
            return;
        }
    };
    if scene_files.is_empty() {
        send(
            &tx,
            serde_json::json!({"type":"error","message":"Sequence has no scene files"}),
        );
        return;
    }

    let fps = params.fps.max(1);
    let prologue = {
        let cfg = state.config.lock().unwrap();
        cfg.prologue
            .as_deref()
            .and_then(|p| std::env::current_dir().ok().map(|d| d.join(p)))
    };

    // Create temp directories
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_base = std::env::temp_dir().join(format!("fairyflow-seqvideo-{ts}"));
    let json_dir = temp_base.join("json");
    let frames_dir = temp_base.join("frames");

    for dir in [&json_dir, &frames_dir] {
        if let Err(e) = tokio::fs::create_dir_all(dir).await {
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Failed to create temp dir: {e}")}),
            );
            return;
        }
    }

    // Run each .ffpy scene to produce a JSON animation definition
    let mut json_paths: Vec<std::path::PathBuf> = Vec::with_capacity(scene_files.len());
    for (i, scene_file) in scene_files.iter().enumerate() {
        let json_path = json_dir.join(format!("scene_{i}.json"));
        if let Err(e) =
            crate::package::run_python_to_json(scene_file, &prologue, fps, &json_path).await
        {
            let _ = tokio::fs::remove_dir_all(&temp_base).await;
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Failed to process '{}': {e}", scene_file)}),
            );
            return;
        }
        json_paths.push(json_path);
    }

    let target_res = if params.width > 0 && params.height > 0 {
        Some((params.width, params.height))
    } else {
        None
    };

    let tx2 = tx.clone();
    let frames_dir2 = frames_dir.clone();
    let temp_base2 = temp_base.clone();

    let render_result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use rayon::prelude::*;

        // Parse all animation definitions and compute total frame count
        let mut anims: Vec<AnimationDef> = Vec::new();
        for json_path in &json_paths {
            let text = std::fs::read_to_string(json_path)?;
            let anim = AnimationDef::from_json(&text)?;
            anims.push(anim);
        }

        let total_frames: u64 = anims
            .iter()
            .map(|a| a.frame_count(SceneSelection::All) as u64)
            .sum();

        let done_count = Arc::new(AtomicU64::new(0));

        renderer_core::clear_image_cache();

        let mut global_offset: u64 = 0;
        for anim in &anims {
            let frame_count = anim.frame_count(SceneSelection::All);
            let frames: Vec<u32> = (0..frame_count).collect();

            let done = Arc::clone(&done_count);
            let tx3 = tx2.clone();
            let fd = frames_dir2.clone();
            let total = total_frames;
            let offset = global_offset;

            frames
                .par_iter()
                .try_for_each(|&frame_n| -> Result<(), String> {
                    let scene = anim
                        .build_scene(FrameId::new(frame_n), SceneSelection::All)
                        .map_err(|e| e.to_string())?;
                    let pixmap = match target_res {
                        Some((w, h)) => renderer_skia::render_scene_fitted(&scene, w, h),
                        None => renderer_skia::render_scene(&scene, 1.0),
                    };
                    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
                    let file_idx = offset + frame_n as u64;
                    std::fs::write(fd.join(format!("frame{file_idx}.png")), &png)
                        .map_err(|e| e.to_string())?;
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = tx3.send(
                        serde_json::json!({"type":"progress","done":n,"total":total}).to_string(),
                    );
                    Ok(())
                })
                .map_err(|e| anyhow::anyhow!("{e}"))?;

            global_offset += frame_count as u64;
            renderer_core::clear_image_cache();
        }

        renderer_core::prune_text_cache();
        Ok(())
    })
    .await;

    match render_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Render failed: {e}")}),
            );
            let _ = tokio::fs::remove_dir_all(&temp_base2).await;
            return;
        }
        Err(e) => {
            send(
                &tx,
                serde_json::json!({"type":"error","message":format!("Render task panicked: {e}")}),
            );
            let _ = tokio::fs::remove_dir_all(&temp_base2).await;
            return;
        }
    }

    send(&tx, serde_json::json!({"type":"ffmpeg"}));

    if let Err(e) = tokio::fs::create_dir_all("exports").await {
        send(
            &tx,
            serde_json::json!({"type":"error","message":format!("Failed to create exports dir: {e}")}),
        );
        let _ = tokio::fs::remove_dir_all(&temp_base2).await;
        return;
    }

    let output_path = std::path::Path::new("exports").join(&filename);
    let output_str = output_path.to_string_lossy().into_owned();

    match crate::render::run_ffmpeg(&frames_dir, &output_path, fps, &params.codec, params.crf).await
    {
        Ok(()) => send(&tx, serde_json::json!({"type":"done","path": output_str})),
        Err(e) => send(&tx, serde_json::json!({"type":"error","message": e})),
    }

    let _ = tokio::fs::remove_dir_all(&temp_base2).await;
}
