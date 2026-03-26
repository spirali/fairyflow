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
use crate::render::render_anim_to_dir;
use crate::service::start_service;

mod service;
mod lancher;
mod render;
mod config;

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

        /// Comma-separated list of frame numbers to render (default: all frames)
        #[arg(long, value_delimiter = ',')]
        frames: Option<Vec<u32>>,

        /// Directories to load additional fonts from before rendering
        #[arg(long = "font-dir")]
        font_dirs: Vec<PathBuf>,

        /// Fit frames into this resolution, letterboxing with black (e.g. 1920x1080)
        #[arg(long = "target-resolution", value_parser = parse_resolution)]
        target_resolution: Option<(u32, u32)>,
    },
    /// Initialize a new project directory
    Init {
        /// Directory to create the project in
        directory: PathBuf,
    },
}

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
        Cmd::RenderJson { json_path, output_dir, threads, frames, font_dirs, target_resolution } => run_render_json(json_path, output_dir, threads, frames, font_dirs, target_resolution).await,
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

    let config = match crate::config::ProjectConfig::load(&toml_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load alsie.toml: {e}");
            std::process::exit(1);
        }
    };
    renderer::Resources::get().load_font_directories(&config.font_directories);

    if let Err(e) = std::env::set_current_dir(&directory) {
        eprintln!("error: could not enter directory {}: {e}", directory.display());
        std::process::exit(1);
    }

    info!(directory = %directory.display(), "serving project");

    start_service(&directory, port, config).await;
}

fn parse_resolution(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s.split_once('x')
        .ok_or_else(|| format!("expected WxH format, got '{s}'"))?;
    let w = w.parse::<u32>().map_err(|_| format!("invalid width '{w}'"))?;
    let h = h.parse::<u32>().map_err(|_| format!("invalid height '{h}'"))?;
    if w == 0 || h == 0 {
        return Err("width and height must be greater than zero".into());
    }
    Ok((w, h))
}

async fn run_render_json(json_path: PathBuf, output_dir: PathBuf, threads: Option<usize>, frames: Option<Vec<u32>>, font_dirs: Vec<PathBuf>, target_resolution: Option<(u32, u32)>) {
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

    if !font_dirs.is_empty() {
        renderer::Resources::get().load_font_directories(&font_dirs);
    }

    render_anim_to_dir(anim, output_dir, threads, frames, target_resolution).await;
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