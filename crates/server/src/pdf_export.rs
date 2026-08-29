use crate::service::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use engine::{AnimationDef, FrameId, SceneSelection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[allow(clippy::enum_variant_names)]
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

/// Frames of one scene file to turn into PDF pages.
///
/// `scene_infos()` reports *local* frame numbers, so each scene's frames are
/// shifted by the running offset — the caller renders in the concatenated
/// (`SceneSelection::All`) frame space.
///
/// Pages follow the points where the player stops: every cue frame, plus the
/// last frame of every non-`flow` scene (`crates/player`'s `scene_end_frames`)
/// — without the latter a scene that has no cues would contribute nothing at
/// all. `is_last_file` marks the file that ends the sequence, whose final frame
/// is a stop point too, `flow` or not (same rule as the sequence preview).
fn selected_frames(anim: &AnimationDef, selection: FrameSelection, is_last_file: bool) -> Vec<u32> {
    let total_frames = anim.frame_count(SceneSelection::All);
    match selection {
        FrameSelection::AllFrames => (0..total_frames).collect(),
        _ => {
            let with_key = matches!(selection, FrameSelection::CuePlusKeyFrames);
            let mut set = BTreeSet::new();
            let mut offset = 0u32;
            for si in anim.scene_infos() {
                for &f in &si.cue_frames {
                    set.insert(offset + f);
                }
                if with_key {
                    for &f in &si.key_frames {
                        set.insert(offset + f);
                    }
                }
                offset += si.frame_count;
                if !si.flow && si.frame_count > 0 {
                    set.insert(offset - 1);
                }
            }
            if is_last_file && total_frames > 0 {
                set.insert(total_frames - 1);
            }
            set.into_iter().collect()
        }
    }
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

        let last_json = json_paths.len().saturating_sub(1);
        for (idx, json_path) in json_paths.iter().enumerate() {
            let text = std::fs::read_to_string(json_path)?;
            let anim = AnimationDef::from_json(&text)?;
            let frames = selected_frames(&anim, frame_selection, idx == last_json);

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// One scene of the v2 wire format: `frames` long, optional cues, optional
    /// `flow`, and a node living from `start` to `end` so the scene has key
    /// frames beyond frame 0.
    fn scene(frames: u32, cues: &[u32], flow: bool, node_span: (u32, u32)) -> serde_json::Value {
        json!({
            "name": "Test", "width": 100, "height": 100, "background": "white",
            "frames": frames, "cues": cues, "flow": flow,
            "children": [0],
            "nodes": [{"kind": "rect", "start": node_span.0, "end": node_span.1,
                       "x": 0, "y": 0, "w": 10, "h": 10, "fill": "green"}]
        })
    }

    fn anim(scenes: Vec<serde_json::Value>) -> AnimationDef {
        AnimationDef::from_json(&json!({"version": 2, "scenes": scenes}).to_string()).unwrap()
    }

    /// Regression: a scene with no `cue()` at all used to contribute zero pages,
    /// even though the player pauses at its end just like it does at a cue.
    #[test]
    fn scene_without_cues_still_yields_its_end_frame() {
        let a = anim(vec![scene(10, &[], false, (0, 5))]);
        assert_eq!(selected_frames(&a, FrameSelection::CueFrames, false), [9]);
    }

    /// A `flow` scene runs straight into the next one — no pause, no page —
    /// unless it is the very end of the sequence.
    #[test]
    fn flow_scene_without_cues_yields_nothing_mid_sequence() {
        let a = anim(vec![scene(10, &[], true, (0, 5))]);
        assert!(selected_frames(&a, FrameSelection::CueFrames, false).is_empty());
        assert_eq!(selected_frames(&a, FrameSelection::CueFrames, true), [9]);
    }

    /// Regression: `scene_infos()` frame numbers are scene-local, but pages are
    /// rendered from the concatenated (`SceneSelection::All`) frame space, so
    /// every scene after the first must be shifted by its start frame.
    #[test]
    fn frames_are_offset_by_scene_start() {
        let a = anim(vec![
            scene(10, &[2], false, (0, 5)),
            scene(10, &[3], false, (0, 5)),
        ]);
        // Scene 0: cue 2, end 9. Scene 1 (offset 10): cue 13, end 19.
        assert_eq!(
            selected_frames(&a, FrameSelection::CueFrames, true),
            [2, 9, 13, 19]
        );
    }

    /// Key frames are scene-local too, and get the same shift.
    #[test]
    fn cue_plus_key_frames_offsets_key_frames() {
        let a = anim(vec![
            scene(10, &[2], false, (0, 4)),
            scene(10, &[], false, (1, 6)),
        ]);
        // Every scene's key frames start at its own local 0, so scene 1
        // (offset 10) contributes 10, 11 and 16 next to scene 0's 0 and 4.
        assert_eq!(
            selected_frames(&a, FrameSelection::CuePlusKeyFrames, true),
            [0, 2, 4, 9, 10, 11, 16, 19]
        );
    }

    /// A cue and a scene end landing on the same frame produce one page, not two.
    #[test]
    fn cue_on_the_end_frame_is_not_duplicated() {
        let a = anim(vec![scene(10, &[9], false, (0, 5))]);
        assert_eq!(selected_frames(&a, FrameSelection::CueFrames, true), [9]);
    }

    /// "All frames" is unaffected by cues, `flow`, or scene boundaries.
    #[test]
    fn all_frames_covers_the_whole_concatenated_range() {
        let a = anim(vec![
            scene(3, &[1], false, (0, 2)),
            scene(2, &[], true, (0, 1)),
        ]);
        assert_eq!(
            selected_frames(&a, FrameSelection::AllFrames, true),
            [0, 1, 2, 3, 4]
        );
    }
}
