use std::net::SocketAddr;
use std::path::{PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU64;
use axum::extract::{Path, Query, State, WebSocketUpgrade};
use axum::extract::ws::{Message, WebSocket};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Router;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
use tracing::{debug, info, warn};
use engine::{AnimationDef, FrameId};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use crate::lancher::{run_python, BuildProcessMsg};

#[derive(Clone)]
struct AppState {
    animation: Arc<Mutex<Option<Arc<AnimationDef>>>>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMsg {
    Run { code: String },
    Terminate,
}

pub async fn start_service(directory: &std::path::Path, port: u16) {
    info!(directory = %directory.display(), "serving project");

    let web_dist = match std::fs::canonicalize("../web/dist") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: could not resolve web/dist: {e}");
            std::process::exit(1);
        }
    };

    let state = AppState {
        animation: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/ls", get(ls_handler))
        .route("/file", get(file_handler).put(file_save_handler))
        .route("/frame/{n}", get(frame_handler))
        .route("/frames", get(frames_handler))
        .route("/tree/{n}", get(tree_handler))
        .route("/trees", get(trees_handler))
        .route("/node/{id}", get(node_handler))
        .fallback_service(ServeDir::new(web_dist))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    info!("http://localhost:{}", port);
    println!("http://localhost:{}", port);

    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize)]
struct FsEntry {
    name: String,
    is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<FsEntry>>,
}

fn build_dir_tree(path: &std::path::Path, depth: u32) -> Vec<FsEntry> {
    if depth == 0 {
        return vec![];
    }
    let Ok(rd) = std::fs::read_dir(path) else {
        return vec![];
    };
    let mut entries: Vec<FsEntry> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                return None;
            }
            let Ok(ft) = e.file_type() else { return None };
            let is_dir = ft.is_dir();
            let children = if is_dir {
                Some(build_dir_tree(&e.path(), depth - 1))
            } else {
                None
            };
            Some(FsEntry { name, is_dir, children })
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    entries
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

async fn file_save_handler(Query(params): Query<FileQuery>, body: String) -> impl IntoResponse {
    match tokio::fs::write(&params.path, body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn file_handler(Query(params): Query<FileQuery>) -> impl IntoResponse {
    match tokio::fs::read_to_string(&params.path).await {
        Ok(content) => (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain; charset=utf-8")], content).into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (StatusCode::NOT_FOUND, "file not found").into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "could not read file").into_response(),
    }
}

async fn ls_handler() -> impl IntoResponse {
    let entries = tokio::task::spawn_blocking(|| build_dir_tree(std::path::Path::new("."), 6))
        .await
        .unwrap_or_default();
    axum::Json(entries)
}

fn get_animation(state: &AppState) -> Option<Arc<AnimationDef>> {
    state.animation.lock().unwrap().as_ref().map(Arc::clone)
}

#[derive(Deserialize)]
struct FrameQuery {
    #[serde(default = "default_scale")]
    scale: f32,
}

#[derive(Deserialize)]
struct RangeQuery {
    from: u32,
    to: u32,
}

#[derive(Deserialize)]
struct FramesQuery {
    from: u32,
    to: u32,
    #[serde(default = "default_scale")]
    scale: f32,
}

#[derive(Serialize)]
struct RenderedFrame {
    n: u32,
    png: String, // base64-encoded PNG
}

#[derive(Serialize)]
struct TreeFrame {
    n: u32,
    scene: renderer::Scene,
}

fn default_scale() -> f32 { 1.0 }

#[derive(Deserialize)]
struct NodeQuery {
    #[serde(default)]
    frame: u32,
}

async fn tree_handler(
    Path(n): axum::extract::Path<u32>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let result = anim.build_scene(FrameId::new(n));
    renderer::prune_text_cache();
    match result {
        Ok(scene) => axum::Json(scene).into_response(),
        Err(e) => {
            warn!(frame = n, error = %e, "build_scene failed in tree_handler");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn trees_handler(
    Query(params): Query<RangeQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    if params.from > params.to {
        return (StatusCode::BAD_REQUEST, "invalid range").into_response();
    }
    let result = tokio::task::spawn_blocking(move || {
        (params.from..=params.to)
            .map(|n| anim.build_scene(FrameId::new(n as u32)).map(|scene| TreeFrame { n, scene }))
            .collect::<Result<Vec<_>, _>>()
    })
        .await;
    renderer::prune_text_cache();
    match result {
        Ok(Ok(frames)) => axum::Json(frames).into_response(),
        Ok(Err(e)) => {
            warn!(from = params.from, to = params.to, error = %e, "build_scene failed in trees_handler");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
        Err(e) => {
            warn!("trees_handler panicked: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "build failed").into_response()
        }
    }
}

async fn node_handler(
    Path(id): axum::extract::Path<u64>,
    Query(params): Query<NodeQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let scene = match anim.build_scene(FrameId::new(params.frame)) {
        Ok(s) => s,
        Err(e) => {
            warn!(node = id, frame = params.frame, error = %e, "build_scene failed in node_handler");
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };
    renderer::prune_text_cache();
    match renderer::find_node_bounds(&scene, id) {
        Some(bounds) => axum::Json(bounds).into_response(),
        None => (StatusCode::NOT_FOUND, "node not found or has no bounds").into_response(),
    }
}

async fn frame_handler(
    Path(n): axum::extract::Path<u32>,
    Query(params): Query<FrameQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let scale = params.scale.clamp(0.01, 256.0);
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let result: Result<anyhow::Result<Vec<u8>>, _> = tokio::task::spawn_blocking(move || {
        let scene = anim.build_scene(FrameId::new(n))?;
        let pixmap = renderer::render_scene(&scene, scale);
        renderer::prune_text_cache();
        Ok(pixmap.encode_png()?)
    }).await;
    match result {
        Ok(Ok(png)) => ([(header::CONTENT_TYPE, "image/png")], png).into_response(),
        Ok(Err(e)) => {
            warn!("rending failed: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response()
        }
        Err(e) => {
            warn!("internal error: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response()
        }
    }
}

async fn frames_handler(
    Query(params): Query<FramesQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let scale = params.scale.clamp(0.01, 256.0);
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let from = params.from;
    let to = params.to;
    if from > to {
        return (StatusCode::BAD_REQUEST, "invalid range").into_response();
    }

    let results = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        (from..=to).into_par_iter().filter_map(|n| {
            let scene = anim.build_scene(FrameId::new(n as u32)).ok()?;
            let pixmap = renderer::render_scene(&scene, scale);
            let png = pixmap.encode_png().ok()?;
            Some(RenderedFrame { n, png: B64.encode(&png) })
        }).collect::<Vec<_>>()
    })
        .await;
    renderer::prune_text_cache();

    match results {
        Ok(frames) => axum::Json(frames).into_response(),
        Err(e) => {
            warn!("rayon render panicked: {e}");
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

    let (out_tx, mut out_rx) = mpsc::channel::<BuildProcessMsg>(64);
    let (kill_tx, _) = broadcast::channel::<()>(4);

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(ClientMsg::Run { code }) => {
                                info!(bytes = code.len(), "received Run request");
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
                        info!("WebSocket closed or errored - exiting handle_socket");
                        break;
                    }
                    _ => {}
                }
            }
            Some(msg) = out_rx.recv() => {
                let kind = match &msg {
                    BuildProcessMsg::Output { .. } => "output",
                    BuildProcessMsg::Error  { .. } => "error",
                    BuildProcessMsg::Tree   { .. } => "tree",
                    BuildProcessMsg::Done   { .. } => "done",
                };
                debug!(kind, "forwarding message to WebSocket");
                let text = serde_json::to_string(&msg).unwrap();
                if socket.send(Message::Text(text.into())).await.is_err() {
                    warn!("WebSocket send failed - exiting handle_socket");
                    break;
                }
            }
        }
    }
}