use crate::config::ProjectConfig;
use crate::lancher::{BuildProcessMsg, run_python};
use axum::Router;
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, Query, Request, State, WebSocketUpgrade};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use engine::{AnimationDef, FrameId, SceneSelection};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tower_http::services::ServeDir;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) animation: Arc<Mutex<Option<Arc<AnimationDef>>>>,
    pub(crate) config: Arc<Mutex<ProjectConfig>>,
    config_tx: broadcast::Sender<ProjectConfig>,
    token: Arc<String>,
}

#[derive(Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

async fn auth_layer(
    State(state): State<AppState>,
    Query(params): Query<TokenQuery>,
    req: Request,
    next: Next,
) -> Response {
    if params.token.as_deref() == Some(state.token.as_str()) {
        next.run(req).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ClientMsg {
    Run { path: String },
    Terminate,
}

fn print_banner(port: u16, token: &str) {
    // ANSI escape codes
    let r    = "\x1b[0m";    // reset
    let b    = "\x1b[1m";    // bold
    let pink = "\x1b[95m";   // bright magenta / pink
    let blue = "\x1b[94m";   // bright blue
    let gray = "\x1b[90m";   // dark gray (dim)

    // FAIRY — pink → magenta → cyan
    println!();
    println!("  {b}{pink}Fairy{r}");
    println!("  {b}{blue}   Flow{r}");
    println!();
    println!("  {b}       http://localhost:{port}/{r}{gray}?token={token}{r}");
    println!();
}

pub async fn start_service(directory: &std::path::Path, port: u16, config: ProjectConfig, token: String) {
    info!(directory = %directory.display(), "serving project");

    let web_dist = match std::fs::canonicalize("../web/dist") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: could not resolve web/dist: {e}");
            std::process::exit(1);
        }
    };

    let (config_tx, _) = broadcast::channel::<ProjectConfig>(4);
    let state = AppState {
        animation: Arc::new(Mutex::new(None)),
        config: Arc::new(Mutex::new(config)),
        config_tx,
        token: Arc::new(token.clone()),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/ls", get(ls_handler))
        .route("/file", get(file_handler).put(file_save_handler))
        .route("/new-file", post(new_file_handler))
        .route("/new-dir", post(new_dir_handler))
        .route("/frame/{n}", get(frame_handler))
        .route("/frames", get(frames_handler))
        .route("/tree/{n}", get(tree_handler))
        .route("/trees", get(trees_handler))
        .route("/node-bounds", get(node_bounds_handler))
        .route("/export-video", post(crate::export::export_handler))
        .route("/export-seq-video", post(crate::export::export_seq_video_handler))
        .route("/export-player", post(crate::package::export_player_handler))
        .route("/export-pdf", post(crate::pdf_export::export_pdf_handler))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_layer))
        .fallback_service(ServeDir::new(web_dist))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    print_banner(port, &token);
    info!("listening at http://localhost:{}/?token={}", port, token);

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
            Some(FsEntry {
                name,
                is_dir,
                children,
            })
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
    entries
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

async fn file_save_handler(
    Query(params): Query<FileQuery>,
    State(state): State<AppState>,
    body: String,
) -> impl IntoResponse {
    if let Err(e) = tokio::fs::write(&params.path, &body).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    let is_config = std::path::Path::new(&params.path)
        .file_name()
        .map_or(false, |n| n == "fairyflow.toml");

    if is_config {
        match ProjectConfig::load(std::path::Path::new("fairyflow.toml")) {
            Ok(new_cfg) => {
                renderer_skia::Resources::get().load_font_directories(&new_cfg.font_directories);
                *state.config.lock().unwrap() = new_cfg.clone();
                state.config_tx.send(new_cfg).ok();
            }
            Err(e) => {
                return (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response();
            }
        }
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn file_handler(Query(params): Query<FileQuery>) -> impl IntoResponse {
    match tokio::fs::read_to_string(&params.path).await {
        Ok(content) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            content,
        )
            .into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (StatusCode::NOT_FOUND, "file not found").into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "could not read file").into_response(),
    }
}

#[derive(Deserialize)]
struct NewItemBody {
    dir: String,
    name: String,
}

/// Returns `Some(path)` if `dir/name` is safe (no `..`, no slashes in name).
fn safe_create_path(dir: &str, name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        return None;
    }
    let base = std::path::Path::new(".");
    let dir_path = if dir.is_empty() || dir == "." {
        base.to_path_buf()
    } else {
        base.join(dir)
    };
    let full = dir_path.join(name);
    for component in full.components() {
        if component == std::path::Component::ParentDir {
            return None;
        }
    }
    Some(full)
}

async fn new_file_handler(axum::Json(body): axum::Json<NewItemBody>) -> impl IntoResponse {
    let Some(path) = safe_create_path(&body.dir, &body.name) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    if path.exists() {
        return (StatusCode::CONFLICT, "already exists").into_response();
    }
    match tokio::fs::write(&path, b"").await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn new_dir_handler(axum::Json(body): axum::Json<NewItemBody>) -> impl IntoResponse {
    let Some(path) = safe_create_path(&body.dir, &body.name) else {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    };
    if path.exists() {
        return (StatusCode::CONFLICT, "already exists").into_response();
    }
    match tokio::fs::create_dir(&path).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
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

fn scene_selection(scene: Option<u32>) -> SceneSelection {
    match scene {
        Some(i) => SceneSelection::Single(i as usize),
        None => SceneSelection::All,
    }
}

#[derive(Deserialize)]
struct FrameQuery {
    #[serde(default = "default_scale")]
    scale: f32,
    scene: Option<u32>,
}

#[derive(Deserialize)]
struct SceneQuery {
    scene: Option<u32>,
}

#[derive(Deserialize)]
struct RangeQuery {
    from: u32,
    to: u32,
    scene: Option<u32>,
}

#[derive(Deserialize)]
struct FramesQuery {
    from: u32,
    to: u32,
    #[serde(default = "default_scale")]
    scale: f32,
    scene: Option<u32>,
}

#[derive(Serialize)]
struct RenderedFrame {
    n: u32,
    png: String, // base64-encoded PNG
}

#[derive(Serialize)]
struct TreeFrame {
    n: u32,
    scene: renderer_skia::Scene,
}

fn default_scale() -> f32 {
    1.0
}

#[derive(Deserialize)]
struct NodeQuery {
    #[serde(default)]
    frame: u32,
    scene: Option<u32>,
}

async fn tree_handler(
    Path(n): axum::extract::Path<u32>,
    Query(params): Query<SceneQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let sel = scene_selection(params.scene);
    renderer_skia::clear_image_cache();
    let result = anim.build_scene(FrameId::new(n), sel);
    renderer_skia::prune_text_cache();
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
    let sel = scene_selection(params.scene);
    let result = tokio::task::spawn_blocking(move || {
        renderer_skia::clear_image_cache();
        (params.from..=params.to)
            .map(|n| {
                anim.build_scene(FrameId::new(n as u32), sel)
                    .map(|scene| TreeFrame { n, scene })
            })
            .collect::<Result<Vec<_>, _>>()
    })
    .await;
    renderer_skia::prune_text_cache();
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

async fn node_bounds_handler(
    Query(params): Query<NodeQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let sel = scene_selection(params.scene);
    match anim.all_node_bounds(FrameId::new(params.frame), sel) {
        Ok(bounds) => axum::Json(bounds).into_response(),
        Err(e) => {
            warn!(frame = params.frame, error = %e, "all_node_bounds failed in node_bounds_handler");
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
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
    let sel = scene_selection(params.scene);
    let result: Result<anyhow::Result<Vec<u8>>, _> = tokio::task::spawn_blocking(move || {
        renderer_skia::clear_image_cache();
        let scene = anim.build_scene(FrameId::new(n), sel)?;
        let pixmap = renderer_skia::render_scene(&scene, scale);
        renderer_skia::prune_text_cache();
        Ok(pixmap.encode_png()?)
    })
    .await;
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
    let sel = scene_selection(params.scene);

    let results = tokio::task::spawn_blocking(move || {
        renderer_skia::clear_image_cache();
        use rayon::prelude::*;
        (from..=to)
            .into_par_iter()
            .filter_map(|n| {
                let scene = anim.build_scene(FrameId::new(n as u32), sel).ok()?;
                let pixmap = renderer_skia::render_scene(&scene, scale);
                let png = pixmap.encode_png().ok()?;
                Some(RenderedFrame {
                    n,
                    png: B64.encode(&png),
                })
            })
            .collect::<Vec<_>>()
    })
    .await;
    renderer_skia::prune_text_cache();

    match results {
        Ok(frames) => axum::Json(frames).into_response(),
        Err(e) => {
            warn!("rayon render panicked: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "render failed").into_response()
        }
    }
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    info!("WebSocket connection established");

    let (out_tx, mut out_rx) = mpsc::channel::<BuildProcessMsg>(64);
    let (kill_tx, _) = broadcast::channel::<()>(4);
    let mut config_rx = state.config_tx.subscribe();

    // Send current config immediately on connect.
    let initial_fps = state.config.lock().unwrap().fps;
    let init_msg = serde_json::to_string(&BuildProcessMsg::Config { fps: initial_fps }).unwrap();
    if socket.send(Message::Text(init_msg.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMsg>(&text) {
                            Ok(ClientMsg::Run { path }) => {
                                info!(path, "received Run request");
                                let n = kill_tx.send(()).unwrap_or(0);
                                debug!(killed_receivers = n, "sent kill signal");
                                let tx = out_tx.clone();
                                let kill_rx = kill_tx.subscribe();
                                let cfg = state.config.lock().unwrap();
                                let prologue = cfg.prologue.as_deref()
                                    .and_then(|p| std::env::current_dir().ok().map(|d| d.join(p)));
                                let fps = cfg.fps;
                                drop(cfg);
                                debug!("subscribed new kill_rx, spawning run_python");
                                tokio::spawn(run_python(path, prologue, fps, tx, kill_rx, state.animation.clone()));
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
                    BuildProcessMsg::Config { .. } => "config",
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
            Ok(cfg) = config_rx.recv() => {
                let text = serde_json::to_string(&BuildProcessMsg::Config { fps: cfg.fps }).unwrap();
                if socket.send(Message::Text(text.into())).await.is_err() {
                    warn!("WebSocket send failed - exiting handle_socket");
                    break;
                }
            }
        }
    }
}
