use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;

const PREAMBLE: &str = include_str!("../../../python/preamble.py");

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
    Tree { steps: u64, nodes: serde_json::Value },
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
    let args = Args::parse();

    if let Some(ref path) = args.file {
        if !path.exists() {
            tokio::fs::write(path, DEFAULT_CONTENT).await.unwrap();
        }
    }

    let state = AppState { file_path: args.file };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("web/dist"))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("http://localhost:{}", args.port);

    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Send file content immediately on connect
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
                                // Persist to file before running
                                if let Some(ref path) = state.file_path {
                                    tokio::fs::write(path, &code).await.ok();
                                }
                                kill_tx.send(()).ok();
                                let tx = out_tx.clone();
                                let kill_rx = kill_tx.subscribe();
                                tokio::spawn(run_python(code, tx, kill_rx));
                            }
                            Ok(ClientMsg::Terminate) => {
                                kill_tx.send(()).ok();
                            }
                            _ => {}
                        }
                    }
                    None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            Some(msg) = out_rx.recv() => {
                let text = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(text.into())).await.is_err() {
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
) {
    let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("alsie_{id}.py"));
    let tree_path = std::env::temp_dir().join(format!("alsie_{id}_tree.json"));

    let full_code = format!("{PREAMBLE}\n{code}");

    if let Err(e) = tokio::fs::write(&tmp, &full_code).await {
        tx.send(ServerMsg::Error { text: e.to_string() }).await.ok();
        tx.send(ServerMsg::Done { exit_code: None }).await.ok();
        return;
    }

    let mut child = match Command::new("python3")
        .arg(&tmp)
        .env("ALSIE_TREE_PATH", &tree_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
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
            stdout_task.await.ok();
            stderr_task.await.ok();
            result.ok().and_then(|s| s.code())
        }
        _ = kill_rx.recv() => {
            child.kill().await.ok();
            stdout_task.abort();
            stderr_task.abort();
            None
        }
    };

    if exit_code == Some(0) {
        if let Ok(json_str) = tokio::fs::read_to_string(&tree_path).await {
            if let Ok(mut data) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let steps = data["steps"].as_u64().unwrap_or(1);
                let nodes = data["nodes"].take();
                tx.send(ServerMsg::Tree { steps, nodes }).await.ok();
            }
        }
    }

    tx.send(ServerMsg::Done { exit_code }).await.ok();

    tokio::fs::remove_file(&tmp).await.ok();
    tokio::fs::remove_file(&tree_path).await.ok();
}
