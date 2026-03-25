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
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use clap::{Parser, Subcommand};
use engine::{AnimationDef, FrameId};
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
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the HTTP server for a project directory
    Serve {
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Project directory to serve (must contain alsie.toml)
        directory: PathBuf,
    },
    /// Render all frames from a JSON animation file to PNG images
    RenderJson {
        /// Path to the JSON animation file
        json_path: PathBuf,

        /// Directory to write frame{n}.png files into
        output_dir: PathBuf,

        /// Number of threads to use for rendering (default: all available)
        #[arg(short, long)]
        threads: Option<usize>,
    },
    /// Evaluate a Python animation file and render all frames to PNG images
    RenderPy {
        /// Path to the Python animation file
        py_file: PathBuf,

        /// Directory to write frame{n}.png files into
        output_dir: PathBuf,
    },
    /// Initialize a new project directory
    Init {
        /// Directory to create the project in
        directory: PathBuf,
    },
}

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

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ServerMsg {
    File { content: String },
    Output { text: String },
    Error { text: String },
    Done { exit_code: Option<i32> },
    Tree { key_frames: Vec<u32>, frame_count: u32 },
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

    renderer::Resources::init();

    let args = Args::parse();

    match args.command {
        Cmd::Serve { port, directory } => run_serve(port, directory).await,
        Cmd::RenderJson { json_path, output_dir, threads } => run_render_json(json_path, output_dir, threads).await,
        Cmd::RenderPy { py_file, output_dir } => run_render_py(py_file, output_dir).await,
        Cmd::Init { directory } => run_init(directory).await,
    }
}

async fn run_serve(port: u16, directory: PathBuf) {
    let toml_path = directory.join("alsie.toml");
    if !toml_path.exists() {
        eprintln!(
            "error: {} does not contain alsie.toml — run `alsie init {}` first",
            directory.display(),
            directory.display()
        );
        std::process::exit(1);
    }

    let web_dist = match std::fs::canonicalize("web/dist") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: could not resolve web/dist: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = std::env::set_current_dir(&directory) {
        eprintln!("error: could not enter directory {}: {e}", directory.display());
        std::process::exit(1);
    }

    info!(directory = %directory.display(), "serving project");

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

async fn run_render_json(json_path: PathBuf, output_dir: PathBuf, threads: Option<usize>) {
    let json_str = match tokio::fs::read_to_string(&json_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {e}", json_path.display());
            std::process::exit(1);
        }
    };

    let anim = match AnimationDef::from_str(&json_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to parse animation JSON: {e}");
            std::process::exit(1);
        }
    };

    render_anim_to_dir(anim, output_dir, threads).await;
}

async fn run_render_py(py_file: PathBuf, output_dir: PathBuf) {
    let tree_path = std::env::temp_dir().join(format!(
        "alsie_render_{}.json",
        std::process::id()
    ));

    info!(py_file = %py_file.display(), tree = %tree_path.display(), "running python");

    let status = tokio::process::Command::new("python3")
        .args(["-m", "alsie"])
        .arg(&py_file)
        .arg(&tree_path)
        .env("PYTHONPATH", "crates/alsie/python")
        .status()
        .await;

    match status {
        Ok(s) if !s.success() => {
            eprintln!("error: python3 exited with {s}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: failed to run python3: {e}");
            std::process::exit(1);
        }
        _ => {}
    }

    let json_str = match tokio::fs::read_to_string(&tree_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read animation JSON from {}: {e}", tree_path.display());
            std::process::exit(1);
        }
    };
    tokio::fs::remove_file(&tree_path).await.ok();

    let anim = match AnimationDef::from_str(&json_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to parse animation JSON: {e}");
            std::process::exit(1);
        }
    };

    render_anim_to_dir(anim, output_dir, None).await;
}

async fn run_init(directory: PathBuf) {
    if let Err(e) = tokio::fs::create_dir_all(&directory).await {
        eprintln!("error: failed to create directory {}: {e}", directory.display());
        std::process::exit(1);
    }

    let files = ["alsie.toml", "scene1.apy", "main.asq"];
    for name in &files {
        let path = directory.join(name);
        if path.exists() {
            eprintln!("warning: {} already exists, skipping", path.display());
            continue;
        }
        if let Err(e) = tokio::fs::write(&path, "").await {
            eprintln!("error: failed to create {}: {e}", path.display());
            std::process::exit(1);
        }
        println!("created {}", path.display());
    }

    println!("initialized project in {}", directory.display());
}

async fn render_anim_to_dir(anim: AnimationDef, output_dir: PathBuf, threads: Option<usize>) {
    if let Err(e) = tokio::fs::create_dir_all(&output_dir).await {
        eprintln!("error: failed to create output directory {}: {e}", output_dir.display());
        std::process::exit(1);
    }

    let key_frames = anim.key_frames();
    let frame_count = key_frames.last().map(|f| f.as_u32() + 1).unwrap_or(1);
    info!(frame_count, "rendering frames");

    let anim = Arc::new(anim);
    let output_dir = Arc::new(output_dir);
    let output_dir_display = output_dir.display().to_string();

    let result = tokio::task::spawn_blocking(move || {
        use rayon::prelude::*;
        let render = || {
            (0..frame_count).into_par_iter().try_for_each(|n| -> Result<(), String> {
                let scene = anim.build_scene(FrameId::new(n)).map_err(|e| e.to_string())?;
                let pixmap = renderer::render_scene(&scene, 1.0);
                let png = pixmap.encode_png().map_err(|e| e.to_string())?;
                let path = output_dir.join(format!("frame{n}.png"));
                std::fs::write(&path, &png).map_err(|e| e.to_string())?;
                info!(frame = n, "wrote {}", path.display());
                Ok(())
            })
        };
        if let Some(n) = threads {
            rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .map_err(|e| e.to_string())
                .and_then(|pool| pool.install(render))
        } else {
            render()
        }
    })
    .await;

    renderer::prune_text_cache();
    match result {
        Ok(Ok(())) => {
            println!("rendered {frame_count} frame(s) to {output_dir_display}");
        }
        Ok(Err(e)) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: render task panicked: {e}");
            std::process::exit(1);
        }
    }
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
    from: usize,
    to: usize,
}

#[derive(Deserialize)]
struct FramesQuery {
    from: usize,
    to: usize,
    #[serde(default = "default_scale")]
    scale: f32,
}

#[derive(Serialize)]
struct RenderedFrame {
    n: usize,
    png: String, // base64-encoded PNG
}

#[derive(Serialize)]
struct TreeFrame {
    n: usize,
    scene: renderer::Scene,
}

fn default_scale() -> f32 { 1.0 }

#[derive(Deserialize)]
struct NodeQuery {
    #[serde(default)]
    frame: u32,
}

async fn tree_handler(
    Path(n): Path<u32>,
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
    Path(id): Path<u64>,
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
    Path(n): Path<u32>,
    Query(params): Query<FrameQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let scale = params.scale.clamp(0.01, 256.0);
    let Some(anim) = get_animation(&state) else {
        return (StatusCode::NOT_FOUND, "no animation").into_response();
    };
    let scene = match anim.build_scene(FrameId::new(n)) {
        Ok(s) => s,
        Err(e) => {
            warn!(frame = n, error = %e, "build_scene failed in frame_handler");
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };
    let pixmap = renderer::render_scene(&scene, scale);
    renderer::prune_text_cache();
    match pixmap.encode_png() {
        Ok(png) => ([(header::CONTENT_TYPE, "image/png")], png).into_response(),
        Err(e) => {
            warn!("failed to encode PNG: {e}");
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
    animation_cache: Arc<Mutex<Option<Arc<AnimationDef>>>>,
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
                match AnimationDef::from_str(&json_str) {
                    Ok(anim) => {
                        let key_frames: Vec<_> = anim.key_frames()
                            .iter().map(|f| f.as_u32()).collect();
                        let frame_count = key_frames.last().copied().unwrap_or(0) + 1;
                        info!(run_id = id, frame_count, "animation cached");
                        *animation_cache.lock().unwrap() = Some(Arc::new(anim));
                        tx.send(ServerMsg::Tree { key_frames, frame_count }).await.ok();
                    }
                    Err(e) => {
                        warn!(run_id = id, error = %e, "failed to parse animation");
                        tx.send(ServerMsg::Error { text: format!("failed to parse animation: {e}") }).await.ok();
                    }
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
