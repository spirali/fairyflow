use engine::{AnimationDef, FrameId, SceneSelection};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

pub async fn render_anim_to_dir(
    anim: AnimationDef,
    output_dir: PathBuf,
    threads: Option<usize>,
    frames: Option<Vec<u32>>,
    target_resolution: Option<(u32, u32)>,
    write_tree: bool,
) {
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        eprintln!(
            "error: failed to create output directory {}: {e}",
            output_dir.display()
        );
        std::process::exit(1);
    }

    let frame_count = anim.frame_count(SceneSelection::All);
    let frames_to_render: Vec<u32> = frames.unwrap_or_else(|| (0..frame_count).collect());
    info!(count = frames_to_render.len(), "rendering frames");

    let anim = Arc::new(anim);
    let output_dir = Arc::new(output_dir);
    let output_dir_display = output_dir.display().to_string();
    let render_count = frames_to_render.len();

    let result = tokio::task::spawn_blocking(move || {
        renderer_skia::clear_image_cache();
        use rayon::prelude::*;
        let render = || {
            frames_to_render
                .into_par_iter()
                .try_for_each(|n| -> Result<(), String> {
                    let scene = anim
                        .build_scene(FrameId::new(n), SceneSelection::All)
                        .map_err(|e| e.to_string())?;
                    let pixmap = match target_resolution {
                        Some((w, h)) => renderer_skia::render_scene_fitted(&scene, w, h),
                        None => renderer_skia::render_scene(&scene, 1.0),
                    };
                    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
                    let path = output_dir.join(format!("frame{n}.png"));
                    std::fs::write(&path, &png).map_err(|e| e.to_string())?;
                    info!(frame = n, "wrote {}", path.display());
                    if write_tree {
                        let json = serde_json::to_string(&scene).map_err(|e| e.to_string())?;
                        let tree_path = output_dir.join(format!("frame{n}.json"));
                        std::fs::write(&tree_path, &json).map_err(|e| e.to_string())?;
                        info!(frame = n, "wrote {}", tree_path.display());
                    }
                    Ok(())
                })
        };
        let mut pool_builder = rayon::ThreadPoolBuilder::new();
        if let Some(n) = threads {
            pool_builder = pool_builder.num_threads(n);
        }
        pool_builder
            .build()
            .map_err(|e| e.to_string())
            .and_then(|pool| pool.install(render))
    })
    .await;

    renderer_skia::prune_text_cache();
    match result {
        Ok(Ok(())) => {
            println!("rendered {render_count} frame(s) to {output_dir_display}");
        }
        Ok(Err(e)) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: render task panicked: {e}");
            std::process::exit(1);
        }
    }
}

/// Shared ffmpeg helper used by both the CLI and the HTTP export handler.
pub(crate) async fn run_ffmpeg(
    frames_dir: &std::path::Path,
    output_path: &std::path::Path,
    fps: u32,
    codec: &str,
    crf: u32,
) -> Result<(), String> {
    let (codec_lib, mut extra): (&str, Vec<String>) = match codec {
        "h265" => ("libx265", vec!["-pix_fmt".into(), "yuv420p".into()]),
        "vp9"  => ("libvpx-vp9", vec!["-b:v".into(), "0".into()]),
        _      => ("libx264", vec!["-pix_fmt".into(), "yuv420p".into()]),
    };
    extra.push("-crf".into());
    extra.push(crf.to_string());

    let input_pattern = frames_dir.join("frame%d.png").to_string_lossy().into_owned();
    let fps_str = fps.to_string();
    let output_str = output_path.to_string_lossy().into_owned();

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.args(["-y", "-framerate", &fps_str, "-i", &input_pattern, "-c:v", codec_lib]);
    for arg in &extra { cmd.arg(arg); }
    cmd.arg(&output_str)
       .stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::piped());

    match cmd.output().await {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let snippet: String = stderr.lines().rev().take(4).collect::<Vec<_>>()
                .into_iter().rev().collect::<Vec<_>>().join(" | ");
            Err(format!("ffmpeg failed: {snippet}"))
        }
        Err(e) => Err(format!("failed to run ffmpeg: {e}")),
    }
}

/// Render all frames from an animation to a video file via ffmpeg.
pub async fn render_anim_to_video(
    anim: AnimationDef,
    output_path: PathBuf,
    threads: Option<usize>,
    target_resolution: Option<(u32, u32)>,
    fps: u32,
    codec: String,
    crf: u32,
) {
    // Check that ffmpeg is available
    let ffmpeg_ok = tokio::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    if !ffmpeg_ok {
        eprintln!("error: ffmpeg is not installed or not in PATH");
        std::process::exit(1);
    }

    // Render all frames into a temp directory
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("fairyflow-video-{ts}"));

    render_anim_to_dir(
        anim,
        temp_dir.clone(),
        threads,
        None,   // render all frames
        target_resolution,
        false,  // write_tree = false
    )
    .await;

    // Ensure output parent directory exists
    if let Some(parent) = output_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            eprintln!("error: failed to create output directory: {e}");
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            std::process::exit(1);
        }
    }

    // Run ffmpeg to produce the video
    match run_ffmpeg(&temp_dir, &output_path, fps, &codec, crf).await {
        Ok(()) => println!("video written to {}", output_path.display()),
        Err(e) => {
            eprintln!("error: {e}");
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            std::process::exit(1);
        }
    }
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}
