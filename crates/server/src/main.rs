use tracing::{debug, info, warn};
use axum::{
    Router,
    extract::{
        Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use renderer::Animation;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value_t = 3000)]
    port: u16,

    /// Python file to edit. Created with default content if it does not exist.
    file: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    file_path: Option<PathBuf>,
    animation: Arc<Mutex<Option<Animation>>>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMsg {
    Run { code: String },
    Terminate,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ServerMsg {
    File { content: String },
    Output { text: String },
    Error { text: String },
    Done { exit_code: Option<i32> },
    Tree { key_frames: Vec<u64>, frames: serde_json::Value },
}

static RUN_ID: AtomicU64 = AtomicU64::new(0);

const DEFAULT_CONTENT: &str = "\
with node().size(300, 200):
    rect().size(30, 20).color(\"green\")  # Add node into the root node

    with node():
        rect()
        circle().radius(10)
";

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
        )
        .init();

    let args = Args::parse();

    if let Some(ref path) = args.file {
        if !path.exists() {
            tokio::fs::write(path, DEFAULT_CONTENT).await.unwrap();
        }
    }

    let state = AppState {
        file_path: args.file,
        animation: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/frame/{n}", get(frame_handler))
        .fallback_service(ServeDir::new("web/dist"))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    info!("http://localhost:{}", args.port);
    println!("http://localhost:{}", args.port);

    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct FrameQuery {
    #[serde(default = "default_scale")]
    scale: f32,
}

fn default_scale() -> f32 { 1.0 }

async fn frame_handler(
    Path(n): Path<usize>,
    Query(params): Query<FrameQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let scale = params.scale.clamp(0.01, 256.0);
    let animation = state.animation.lock().unwrap();
    let Some(anim) = animation.as_ref() else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let Some(scene) = anim.frames.get(n) else {
        return (StatusCode::NOT_FOUND, "frame out of range").into_response();
    };
    let pixmap = renderer::render_scene(scene, scale);
    match pixmap.encode_png() {
        Ok(png) => ([(header::CONTENT_TYPE, "image/png")], png).into_response(),
        Err(e) => {
            warn!("failed to encode PNG: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response()
        }
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("WebSocket connection established");
    if let Some(ref path) = state.file_path {
        let content = tokio::fs::read_to_string(path).await.unwrap_or_default();
        let msg = serde_json::to_string(&ServerMsg::File { content }).unwrap();
        if socket.send(Message::Text(msg.into())).await.is_err() {
            return;
        }
    }

    let (out_tx, mut out_rx) = mpsc::channel::<ServerMsg>(64);
    let (kill_tx, _) = broadcast::channel::<()>(4);

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(ClientMsg::Run { code }) => {
                                info!(bytes = code.len(), "received Run request");
                                if let Some(ref path) = state.file_path {
                                    tokio::fs::write(path, &code).await.ok();
                                }
                                let n = kill_tx.send(()).unwrap_or(0);
                                debug!(killed_receivers = n, "sent kill signal");
                                let tx = out_tx.clone();
                                let kill_rx = kill_tx.subscribe();
                                debug!("subscribed new kill_rx, spawning run_python");
                                tokio::spawn(run_python(code, tx, kill_rx, state.animation.clone()));
                            }
                            Ok(ClientMsg::Terminate) => {
                                info!("received Terminate request");
                                kill_tx.send(()).ok();
                            }
                            _ => {}
                        }
                    }
                    None | Some(Err(_)) => {
                        info!("WebSocket closed or errored — exiting handle_socket");
                        break;
                    }
                    _ => {}
                }
            }
            Some(msg) = out_rx.recv() => {
                let kind = match &msg {
                    ServerMsg::Output { .. } => "output",
                    ServerMsg::Error  { .. } => "error",
                    ServerMsg::Tree   { .. } => "tree",
                    ServerMsg::Done   { .. } => "done",
                    ServerMsg::File   { .. } => "file",
                };
                debug!(kind, "forwarding message to WebSocket");
                let text = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(text.into())).await.is_err() {
                    warn!("WebSocket send failed — exiting handle_socket");
                    break;
                }
            }
        }
    }
}

async fn run_python(
    code: String,
    tx: mpsc::Sender<ServerMsg>,
    mut kill_rx: broadcast::Receiver<()>,
    animation_cache: Arc<Mutex<Option<Animation>>>,
) {
    let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
    info!(run_id = id, "run_python started");
    let tmp = std::env::temp_dir().join(format!("alsie_{id}.py"));
    let tree_path = std::env::temp_dir().join(format!("alsie_{id}_tree.json"));

    if let Err(e) = tokio::fs::write(&tmp, &code).await {
        warn!(run_id = id, error = %e, "failed to write temp script");
        tx.send(ServerMsg::Error { text: e.to_string() }).await.ok();
        tx.send(ServerMsg::Done { exit_code: None }).await.ok();
        return;
    }

    let mut child = match Command::new("python3")
        .args(["-m", "alsie"])
        .arg(&tmp)
        .arg(&tree_path)
        .env("PYTHONPATH", "python")
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
            tx.send(ServerMsg::Error { text: format!("Failed to start python3: {e}") }).await.ok();
            tx.send(ServerMsg::Done { exit_code: None }).await.ok();
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
            if tx_out.send(ServerMsg::Output { text: line }).await.is_err() {
                break;
            }
        }
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = stderr.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx_err.send(ServerMsg::Error { text: line }).await.is_err() {
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
                // Send tree to frontend for the scene tree view
                match serde_json::from_str::<serde_json::Value>(&json_str) {
                    Ok(mut data) => {
                        let key_frames = data["key_frames"].as_array()
                            .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
                            .unwrap_or_default();
                        let frames = data["frames"].take();
                        info!(run_id = id, "sending Tree message");
                        tx.send(ServerMsg::Tree { key_frames, frames }).await.ok();
                    }
                    Err(e) => warn!(run_id = id, error = %e, "failed to parse tree JSON"),
                }
                // Parse and cache the animation for on-demand rendering
                match renderer::parse_scene(&json_str) {
                    Ok(anim) => {
                        info!(run_id = id, frames = anim.frames.len(), "animation cached");
                        *animation_cache.lock().unwrap() = Some(anim);
                    }
                    Err(e) => warn!(run_id = id, error = %e, "failed to parse animation for renderer"),
                }
            }
            Err(e) => warn!(run_id = id, error = %e, "failed to read tree file"),
        }
    }

    info!(run_id = id, exit_code = ?exit_code, "sending Done message");
    tx.send(ServerMsg::Done { exit_code }).await.ok();

    tokio::fs::remove_file(&tmp).await.ok();
    tokio::fs::remove_file(&tree_path).await.ok();
    info!(run_id = id, "run_python finished");
}
