use crate::service::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use engine::{AnimationDef, FrameId, SceneSelection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum FrameSelection {
    CueFrames,
    CuePlusKeyFrames,
    AllFrames,
}

#[derive(Deserialize)]
pub struct ExportPdfParams {
    pub seq_path: String,
    pub filename: String,
    pub fps: u32,
    pub frame_selection: FrameSelection,
}

#[derive(Serialize)]
pub struct ExportPdfResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn err(status: StatusCode, msg: impl Into<String>) -> axum::response::Response {
    (
        status,
        Json(ExportPdfResult {
            success: false,
            path: None,
            error: Some(msg.into()),
        }),
    )
        .into_response()
}

pub async fn export_pdf_handler(
    State(state): State<AppState>,
    Json(params): Json<ExportPdfParams>,
) -> impl IntoResponse {
    let filename = params.filename.trim().to_string();
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return err(StatusCode::BAD_REQUEST, "Invalid filename");
    }
    let filename = if filename.ends_with(".pdf") {
        filename
    } else {
        format!("{filename}.pdf")
    };

    // Read and parse the .ffsq sequence file.
    let seq_content = match tokio::fs::read_to_string(&params.seq_path).await {
        Ok(c) => c,
        Err(e) => {
            return err(
                StatusCode::BAD_REQUEST,
                format!("Failed to read sequence: {e}"),
            );
        }
    };
    let seq_data: serde_json::Value = match serde_json::from_str(&seq_content) {
        Ok(v) => v,
        Err(e) => {
            return err(
                StatusCode::BAD_REQUEST,
                format!("Failed to parse sequence: {e}"),
            );
        }
    };
    let scene_files: Vec<String> = match seq_data.get("scene_files").and_then(|v| v.as_array()) {
        Some(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        None => return err(StatusCode::BAD_REQUEST, "Sequence has no scene_files"),
    };
    if scene_files.is_empty() {
        return err(StatusCode::BAD_REQUEST, "Sequence has no scene files");
    }

    let fps = params.fps.max(1);
    let prologue = {
        let cfg = state.config.lock().unwrap();
        cfg.prologue
            .as_deref()
            .and_then(|p| std::env::current_dir().ok().map(|d| d.join(p)))
    };

    // Temp dir for intermediate scene JSON files.
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("fairyflow-pdf-{ts}"));
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create temp dir: {e}"),
        );
    }

    // Run each .ffpy scene and collect the resulting JSON paths.
    let mut json_paths: Vec<PathBuf> = Vec::with_capacity(scene_files.len());
    for (i, scene_file) in scene_files.iter().enumerate() {
        let json_path = temp_dir.join(format!("scene_{i}.json"));
        if let Err(e) =
            crate::package::run_python_to_json(scene_file, &prologue, fps, &json_path).await
        {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to process '{}': {e}", scene_file),
            );
        }
        json_paths.push(json_path);
    }

    // Ensure the exports directory exists.
    if let Err(e) = tokio::fs::create_dir_all("exports").await {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create exports dir: {e}"),
        );
    }

    let output_path = PathBuf::from("exports").join(&filename);
    let frame_selection = params.frame_selection;

    // Build scenes and render PDF in a blocking task.
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let mut all_scenes: Vec<renderer_core::Scene> = Vec::new();

        for json_path in &json_paths {
            let text = std::fs::read_to_string(json_path)?;
            let anim = AnimationDef::from_json(&text)?;

            // Collect frame indices based on selection.
            let frames: Vec<u32> = match frame_selection {
                FrameSelection::CueFrames => {
                    let infos = anim.scene_infos();
                    let mut set = BTreeSet::new();
                    for si in &infos {
                        for &f in &si.cue_frames {
                            set.insert(f);
                        }
                    }
                    set.into_iter().collect()
                }
                FrameSelection::CuePlusKeyFrames => {
                    let infos = anim.scene_infos();
                    let mut set = BTreeSet::new();
                    for si in &infos {
                        for &f in &si.cue_frames {
                            set.insert(f);
                        }
                        for &f in &si.key_frames {
                            set.insert(f);
                        }
                    }
                    set.into_iter().collect()
                }
                FrameSelection::AllFrames => (0..anim.frame_count(SceneSelection::All)).collect(),
            };

            renderer_core::clear_image_cache();
            for f in frames {
                let scene = anim.build_scene(FrameId::new(f), SceneSelection::All)?;
                all_scenes.push(scene);
            }
            renderer_core::prune_text_cache();
        }

        let scene_refs: Vec<&renderer_core::Scene> = all_scenes.iter().collect();
        let pdf_bytes = renderer_pdf::render_to_pdf(&scene_refs)?;
        std::fs::write(&output_path, &pdf_bytes)?;
        Ok(output_path.to_string_lossy().into_owned())
    })
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    match result {
        Ok(Ok(path)) => Json(ExportPdfResult {
            success: true,
            path: Some(path),
            error: None,
        })
        .into_response(),
        Ok(Err(e)) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create PDF: {e}"),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("PDF task panicked: {e}"),
        ),
    }
}
