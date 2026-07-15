use engine::{AnimationDef, SceneSelection};
use serde::Serialize;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tracing::info;
use tracing::warn;

pub(crate) static RUN_ID: AtomicU64 = AtomicU64::new(0);

/// Builds the base `python3 -m fairyflow` command with the standard arguments
/// and `PYTHONPATH`. The caller is responsible for configuring stdio and spawning.
pub(crate) fn build_python_cmd(
    source_path: &str,
    prologue: &Option<std::path::PathBuf>,
    fps: u32,
    debug: bool,
    output_path: &std::path::Path,
) -> Command {
    let mut cmd = Command::new("python3");
    cmd.args(["-m", "fairyflow"]);
    if let Some(p) = prologue {
        cmd.arg("--prologue").arg(p);
    }
    if debug {
        cmd.arg("--debug");
    }
    cmd.arg(source_path).arg(output_path).arg(fps.to_string());
    cmd
}

#[derive(Serialize)]
pub struct SceneInfoMsg {
    pub name: String,
    pub key_frames: Vec<u32>,
    pub cue_frames: Vec<u32>,
    pub frame_count: u32,
    pub info: Vec<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildProcessMsg {
    Config {
        fps: u32,
    },
    Output {
        text: String,
    },
    Error {
        text: String,
    },
    Done {
        exit_code: Option<i32>,
    },
    FileChanged {
        path: String,
    },
    Tree {
        /// Combined key frames and frame count for "All scenes" mode.
        key_frames: Vec<u32>,
        /// Combined cue frames for "All scenes" mode (absolute frame numbers).
        cue_frames: Vec<u32>,
        frame_count: u32,
        /// Per-scene metadata.
        scenes: Vec<SceneInfoMsg>,
    },
}

pub async fn run_python(
    source_path: String,
    prologue: Option<std::path::PathBuf>,
    fps: u32,
    debug: bool,
    tx: mpsc::Sender<BuildProcessMsg>,
    mut kill_rx: broadcast::Receiver<()>,
    animation_cache: Arc<Mutex<Option<Arc<AnimationDef>>>>,
) {
    let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
    info!(run_id = id, path = source_path, "run_python started");
    let tree_path = std::env::temp_dir().join(format!("fairyflow_{id}_tree.json"));

    let mut child = match build_python_cmd(&source_path, &prologue, fps, debug, &tree_path)
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
            tx.send(BuildProcessMsg::Error {
                text: format!("Failed to start python3: {e}"),
            })
            .await
            .ok();
            tx.send(BuildProcessMsg::Done { exit_code: None })
                .await
                .ok();
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
            if tx_out
                .send(BuildProcessMsg::Output { text: line })
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = stderr.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_err
                .send(BuildProcessMsg::Error { text: line })
                .await
                .is_err()
            {
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
            Ok(json_str) => match AnimationDef::from_json(&json_str) {
                Ok(anim) => {
                    let key_frames: Vec<_> = anim
                        .key_frames(SceneSelection::All)
                        .iter()
                        .map(|f| f.as_u32())
                        .collect();
                    let frame_count = anim.frame_count(SceneSelection::All);
                    let scene_infos = anim.scene_infos();
                    // Compute combined cue frames across all scenes (with offsets).
                    let mut cue_frames: Vec<u32> = Vec::new();
                    {
                        let mut offset = 0u32;
                        for si in &scene_infos {
                            for &cf in &si.cue_frames {
                                cue_frames.push(cf + offset);
                            }
                            offset += si.frame_count;
                        }
                        cue_frames.sort_unstable();
                        cue_frames.dedup();
                    }
                    let scenes: Vec<SceneInfoMsg> = scene_infos
                        .into_iter()
                        .map(|si| SceneInfoMsg {
                            name: si.name,
                            key_frames: si.key_frames,
                            cue_frames: si.cue_frames,
                            frame_count: si.frame_count,
                            info: si.info,
                        })
                        .collect();
                    info!(
                        run_id = id,
                        frame_count,
                        scenes = scenes.len(),
                        "animation cached"
                    );
                    *animation_cache.lock().unwrap() = Some(Arc::new(anim));
                    tx.send(BuildProcessMsg::Tree {
                        key_frames,
                        cue_frames,
                        frame_count,
                        scenes,
                    })
                    .await
                    .ok();
                }
                Err(e) => {
                    warn!(run_id = id, error = %e, "failed to parse animation");
                    tx.send(BuildProcessMsg::Error {
                        text: format!("failed to parse animation: {e}"),
                    })
                    .await
                    .ok();
                }
            },
            Err(e) => warn!(run_id = id, error = %e, "failed to read tree file"),
        }
    }

    info!(run_id = id, exit_code = ?exit_code, "sending Done message");
    tx.send(BuildProcessMsg::Done { exit_code }).await.ok();

    tokio::fs::remove_file(&tree_path).await.ok();
    info!(run_id = id, "run_python finished");
}
