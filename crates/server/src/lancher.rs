use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::Serialize;
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use tracing::warn;
use engine::AnimationDef;
use tokio::io::{AsyncBufReadExt, BufReader};

static RUN_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BuildProcessMsg {
    Output { text: String },
    Error { text: String },
    Done { exit_code: Option<i32> },
    Tree { key_frames: Vec<u32>, frame_count: u32 },
}

pub async fn run_python(
    source_path: String,
    prologue: Option<std::path::PathBuf>,
    tx: mpsc::Sender<BuildProcessMsg>,
    mut kill_rx: broadcast::Receiver<()>,
    animation_cache: Arc<Mutex<Option<Arc<AnimationDef>>>>,
) {
    let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
    info!(run_id = id, path = source_path, "run_python started");
    let tree_path = std::env::temp_dir().join(format!("alsie_{id}_tree.json"));

    let mut cmd = Command::new("python3");
    cmd.args(["-m", "alsie"]);
    if let Some(p) = &prologue {
        cmd.arg("--prologue").arg(p);
    }
    cmd.arg(&source_path).arg(&tree_path);

    let mut child = match cmd
        .env("PYTHONPATH", "crates/alsie/python")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => {
            info!(run_id = id, pid = c.id(), "python3 process spawned");
            c
        }
        Err(e) => {
            warn!(run_id = id, error = %e, "failed to spawn python3");
            tx.send(BuildProcessMsg::Error { text: format!("Failed to start python3: {e}") }).await.ok();
            tx.send(BuildProcessMsg::Done { exit_code: None }).await.ok();
            return;
        }
    };

    let stdout = BufReader::new(child.stdout.take().unwrap());
    let stderr = BufReader::new(child.stderr.take().unwrap());

    let tx_out = tx.clone();
    let tx_err = tx.clone();

    let stdout_task = tokio::spawn(async move {
        let mut lines = stdout.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_out.send(BuildProcessMsg::Output { text: line }).await.is_err() {
                break;
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = stderr.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_err.send(BuildProcessMsg::Error { text: line }).await.is_err() {
                break;
            }
        }
    });

    let exit_code = tokio::select! {
        result = child.wait() => {
            info!(run_id = id, "python3 process exited, draining stdout/stderr");
            stdout_task.await.ok();
            stderr_task.await.ok();
            let code = result.ok().and_then(|s| s.code());
            info!(run_id = id, exit_code = ?code, "stdout/stderr drained");
            code
        }
        res = kill_rx.recv() => {
            info!(run_id = id, kill_result = ?res, "kill signal received, terminating process");
            child.kill().await.ok();
            stdout_task.abort();
            stderr_task.abort();
            None
        }
    };

    if exit_code == Some(0) {
        match tokio::fs::read_to_string(&tree_path).await {
            Ok(json_str) => {
                match AnimationDef::from_str(&json_str) {
                    Ok(anim) => {
                        let key_frames: Vec<_> = anim.key_frames()
                            .iter().map(|f| f.as_u32()).collect();
                        let frame_count = key_frames.last().copied().unwrap_or(0) + 1;
                        info!(run_id = id, frame_count, "animation cached");
                        *animation_cache.lock().unwrap() = Some(Arc::new(anim));
                        tx.send(BuildProcessMsg::Tree { key_frames, frame_count }).await.ok();
                    }
                    Err(e) => {
                        warn!(run_id = id, error = %e, "failed to parse animation");
                        tx.send(BuildProcessMsg::Error { text: format!("failed to parse animation: {e}") }).await.ok();
                    }
                }
            }
            Err(e) => warn!(run_id = id, error = %e, "failed to read tree file"),
        }
    }

    info!(run_id = id, exit_code = ?exit_code, "sending Done message");
    tx.send(BuildProcessMsg::Done { exit_code }).await.ok();

    tokio::fs::remove_file(&tree_path).await.ok();
    info!(run_id = id, "run_python finished");
}
