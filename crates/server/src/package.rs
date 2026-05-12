use crate::service::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use player::{CreateConfig, create_package};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tracing::info;

#[derive(Deserialize)]
pub struct ExportPlayerParams {
    pub seq_path: String,
    pub filename: String,
    pub fps: u32,
}

#[derive(Serialize)]
pub struct ExportPlayerResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn err(status: StatusCode, msg: impl Into<String>) -> axum::response::Response {
    (
        status,
        Json(ExportPlayerResult {
            success: false,
            path: None,
            error: Some(msg.into()),
        }),
    )
        .into_response()
}

pub async fn export_player_handler(
    State(state): State<AppState>,
    Json(params): Json<ExportPlayerParams>,
) -> impl IntoResponse {
    let filename = params.filename.trim().to_string();
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
    {
        return err(StatusCode::BAD_REQUEST, "Invalid filename");
    }
    let filename = if filename.ends_with(".ffpkg") {
        filename
    } else {
        format!("{filename}.ffpkg")
    };

    // Read and parse the .ffsq sequence file
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

    // Create a temp dir for intermediate scene JSON files
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("fairyflow-pkg-{ts}"));
    if let Err(e) = tokio::fs::create_dir_all(&temp_dir).await {
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create temp dir: {e}"),
        );
    }

    // Run each .ffpy scene and collect the resulting JSON paths
    let mut json_paths: Vec<PathBuf> = Vec::with_capacity(scene_files.len());
    for (i, scene_file) in scene_files.iter().enumerate() {
        let json_path = temp_dir.join(format!("scene_{i}.json"));
        if let Err(e) = run_python_to_json(scene_file, &prologue, fps, &json_path).await {
            let _ = tokio::fs::remove_dir_all(&temp_dir).await;
            return err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to process '{}': {e}", scene_file),
            );
        }
        json_paths.push(json_path);
    }

    // Ensure the exports directory exists
    if let Err(e) = tokio::fs::create_dir_all("exports").await {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        return err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create exports dir: {e}"),
        );
    }

    // Call create_package in a blocking task (it does zip I/O)
    let output_path = PathBuf::from("exports").join(&filename);
    let result = tokio::task::spawn_blocking(move || {
        let refs: Vec<&Path> = json_paths.iter().map(|p| p.as_path()).collect();
        create_package(Path::new("."), &refs, CreateConfig { fps }, &output_path)
            .map(|()| output_path.to_string_lossy().into_owned())
    })
    .await;

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    match result {
        Ok(Ok(path)) => Json(ExportPlayerResult {
            success: true,
            path: Some(path),
            error: None,
        })
        .into_response(),
        Ok(Err(e)) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create package: {e}"),
        ),
        Err(e) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Package task panicked: {e}"),
        ),
    }
}

pub(crate) async fn run_python_to_json(
    source_path: &str,
    prologue: &Option<PathBuf>,
    fps: u32,
    output_path: &Path,
) -> anyhow::Result<()> {
    let id = crate::lancher::RUN_ID.fetch_add(1, Ordering::Relaxed);
    info!(
        run_id = id,
        path = source_path,
        "run_python_to_json for player package"
    );

    let output = crate::lancher::build_python_cmd(source_path, prologue, fps, false, output_path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("failed to spawn python3: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "python3 exited with code {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    if !output_path.exists() {
        return Err(anyhow::anyhow!("python3 did not produce output file"));
    }
    Ok(())
}
