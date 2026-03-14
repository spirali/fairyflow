use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;

#[derive(Parser)]
struct Args {
    #[arg(short, long, default_value_t = 3000)]
    port: u16,
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
    Output { text: String },
    Error { text: String },
    Done { exit_code: Option<i32> },
}

static RUN_ID: AtomicU64 = AtomicU64::new(0);

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .fallback_service(ServeDir::new("web/dist"));

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    println!("http://localhost:{}", args.port);

    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    let (out_tx, mut out_rx) = mpsc::channel::<ServerMsg>(64);
    let (kill_tx, _) = broadcast::channel::<()>(4);

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(ClientMsg::Run { code }) => {
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

async fn run_python(code: String, tx: mpsc::Sender<ServerMsg>, mut kill_rx: broadcast::Receiver<()>) {
    let id = RUN_ID.fetch_add(1, Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("alsie_{id}.py"));

    if let Err(e) = tokio::fs::write(&tmp, &code).await {
        tx.send(ServerMsg::Error { text: e.to_string() }).await.ok();
        tx.send(ServerMsg::Done { exit_code: None }).await.ok();
        return;
    }

    let mut child = match Command::new("python3")
        .arg(&tmp)
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

    tx.send(ServerMsg::Done { exit_code }).await.ok();
    tokio::fs::remove_file(&tmp).await.ok();
}
