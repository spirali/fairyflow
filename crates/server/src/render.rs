use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use engine::{AnimationDef, FrameId};

pub async fn render_anim_to_dir(anim: AnimationDef, output_dir: PathBuf, threads: Option<usize>, frames: Option<Vec<u32>>, target_resolution: Option<(u32, u32)>) {
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        eprintln!("error: failed to create output directory {}: {e}", output_dir.display());
        std::process::exit(1);
    }

    let key_frames = anim.key_frames();
    let frame_count = key_frames.last().map(|f| f.as_u32() + 1).unwrap_or(1);
    let frames_to_render: Vec<u32> = frames.unwrap_or_else(|| (0..frame_count).collect());
    info!(count = frames_to_render.len(), "rendering frames");

    let anim = Arc::new(anim);
    let output_dir = Arc::new(output_dir);
    let output_dir_display = output_dir.display().to_string();
    let render_count = frames_to_render.len();

    let result = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        let render = || {
            frames_to_render.into_par_iter().try_for_each(|n| -> Result<(), String> {
                let scene = anim.build_scene(FrameId::new(n)).map_err(|e| e.to_string())?;
                let pixmap = match target_resolution {
                    Some((w, h)) => renderer::render_scene_fitted(&scene, w, h),
                    None => renderer::render_scene(&scene, 1.0),
                };
                let png = pixmap.encode_png().map_err(|e| e.to_string())?;
                let path = output_dir.join(format!("frame{n}.png"));
                std::fs::write(&path, &png).map_err(|e| e.to_string())?;
                info!(frame = n, "wrote {}", path.display());
                Ok(())
            })
        };
        let mut pool_builder = rayon::ThreadPoolBuilder::new();
        if let Some(n) = threads {
            pool_builder = pool_builder.num_threads(n);
        }
        pool_builder.build()
        .map_err(|e| e.to_string())
        .and_then(|pool| pool.install(render))
    })
        .await;

    renderer::prune_text_cache();
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
