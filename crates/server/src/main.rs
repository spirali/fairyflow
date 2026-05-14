use crate::render::render_anim_to_dir;
use crate::service::start_service;
use clap::{Parser, Subcommand};
use engine::AnimationDef;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

mod config;
mod export;
mod lancher;
mod package;
mod pdf_export;
mod render;
mod service;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the HTTP server for a project directory
    Open {
        #[arg(short, long, default_value_t = 3000)]
        port: u16,

        /// Authentication token (default: randomly generated)
        #[arg(long)]
        token: Option<String>,

        /// Project directory to serve (must contain fairyflow.toml)
        directory: PathBuf,
    },
    /// Render all frames from a JSON animation file to PNG images
    RenderPng {
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

        /// Map a CSS generic family to a specific font (e.g. sans-serif=DejaVu Sans)
        #[arg(long = "font-alias", value_parser = parse_font_alias)]
        font_aliases: Vec<(String, String)>,

        /// Fit frames into this resolution, letterboxing with black (e.g. 1920x1080)
        #[arg(long = "target-resolution", value_parser = parse_resolution)]
        target_resolution: Option<(u32, u32)>,

        /// Also write frame{n}.json with the evaluated scene tree for each rendered frame
        #[arg(long)]
        write_tree: bool,
    },

    /// Render a JSON animation to a video file via ffmpeg
    RenderVideo {
        /// Path to the JSON animation file
        json_path: PathBuf,

        /// Output video file path (e.g. out.mp4)
        output_file: PathBuf,

        /// Number of threads to use for rendering (default: all available)
        #[arg(short, long)]
        threads: Option<usize>,

        /// Directories to load additional fonts from before rendering
        #[arg(long = "font-dir")]
        font_dirs: Vec<PathBuf>,

        /// Map a CSS generic family to a specific font (e.g. sans-serif=DejaVu Sans)
        #[arg(long = "font-alias", value_parser = parse_font_alias)]
        font_aliases: Vec<(String, String)>,

        /// Fit frames into this resolution, letterboxing with black (e.g. 1920x1080)
        #[arg(long = "target-resolution", value_parser = parse_resolution)]
        target_resolution: Option<(u32, u32)>,

        /// Frames per second
        #[arg(long, default_value_t = 24)]
        fps: u32,

        /// Video codec: h264, h265, or vp9
        #[arg(long, default_value = "h264")]
        codec: String,

        /// Constant rate factor for video quality (lower = better quality)
        #[arg(long, default_value_t = 23)]
        crf: u32,
    },

    /// Render all frames from a JSON animation file to a multi-page PDF
    RenderPdf {
        /// Path to the JSON animation file
        json_path: PathBuf,

        /// Output PDF file path (e.g. out.pdf)
        output_file: PathBuf,

        /// Comma-separated list of frame numbers to render (default: all frames)
        #[arg(long, value_delimiter = ',')]
        frames: Option<Vec<u32>>,

        /// Directories to load additional fonts from before rendering
        #[arg(long = "font-dir")]
        font_dirs: Vec<PathBuf>,

        /// Map a CSS generic family to a specific font (e.g. sans-serif=DejaVu Sans)
        #[arg(long = "font-alias", value_parser = parse_font_alias)]
        font_aliases: Vec<(String, String)>,
    },
    /// Initialize a new project directory
    Init {
        /// Directory to create the project in
        directory: PathBuf,
    },
    /// Open a .ffpkg package file in the native player window
    Play {
        /// Path to the .ffpkg package file
        package: PathBuf,

        /// Frames to pre-render ahead of the current playback position
        #[arg(long, default_value_t = 30)]
        lookahead: u32,

        /// Frames to keep rendered behind the current playback position (for fast reverse)
        #[arg(long, default_value_t = 30)]
        lookback: u32,
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

    renderer_skia::Resources::init();

    let args = Args::parse();

    match args.command {
        Cmd::Open {
            port,
            token,
            directory,
        } => run_serve(port, token, directory).await,
        Cmd::RenderPng {
            json_path,
            output_dir,
            threads,
            frames,
            font_dirs,
            font_aliases,
            target_resolution,
            write_tree,
        } => {
            run_render_png(
                json_path,
                output_dir,
                threads,
                frames,
                font_dirs,
                font_aliases,
                target_resolution,
                write_tree,
            )
            .await
        }
        Cmd::RenderVideo {
            json_path,
            output_file,
            threads,
            font_dirs,
            font_aliases,
            target_resolution,
            fps,
            codec,
            crf,
        } => {
            run_render_video(
                json_path,
                output_file,
                threads,
                font_dirs,
                font_aliases,
                VideoOptions {
                    target_resolution,
                    fps,
                    codec,
                    crf,
                },
            )
            .await
        }
        Cmd::RenderPdf {
            json_path,
            output_file,
            frames,
            font_dirs,
            font_aliases,
        } => run_render_pdf(json_path, output_file, frames, font_dirs, font_aliases).await,
        Cmd::Init { directory } => run_init(directory).await,
        Cmd::Play {
            package,
            lookahead,
            lookback,
        } => {
            if let Err(e) = player::open_player(&package, lookahead, lookback) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

async fn run_serve(port: u16, token: Option<String>, directory: PathBuf) {
    let toml_path = directory.join("fairyflow.toml");
    if !toml_path.exists() {
        eprintln!(
            "error: {} does not contain fairyflow.toml — run `fairyflow init {}` first",
            directory.display(),
            directory.display()
        );
        std::process::exit(1);
    }

    let config = match crate::config::ProjectConfig::load(&toml_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: failed to load fairyflow.toml: {e}");
            std::process::exit(1);
        }
    };
    renderer_skia::Resources::get().load_font_directories(&config.font_directories);
    renderer_skia::Resources::get().set_font_aliases(&config.font_aliases);

    if let Err(e) = std::env::set_current_dir(&directory) {
        eprintln!(
            "error: could not enter directory {}: {e}",
            directory.display()
        );
        std::process::exit(1);
    }

    info!(directory = %directory.display(), "serving project");

    let token = token.unwrap_or_else(|| {
        use rand::Rng;
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    });

    start_service(&directory, port, config, token).await;
}

fn parse_font_alias(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("expected GENERIC=FONTNAME, got '{s}'"))
}

fn parse_resolution(s: &str) -> Result<(u32, u32), String> {
    let (w, h) = s
        .split_once('x')
        .ok_or_else(|| format!("expected WxH format, got '{s}'"))?;
    let w = w
        .parse::<u32>()
        .map_err(|_| format!("invalid width '{w}'"))?;
    let h = h
        .parse::<u32>()
        .map_err(|_| format!("invalid height '{h}'"))?;
    if w == 0 || h == 0 {
        return Err("width and height must be greater than zero".into());
    }
    Ok((w, h))
}

async fn run_render_png(
    json_path: PathBuf,
    output_dir: PathBuf,
    threads: Option<usize>,
    frames: Option<Vec<u32>>,
    font_dirs: Vec<PathBuf>,
    font_aliases: Vec<(String, String)>,
    target_resolution: Option<(u32, u32)>,
    write_tree: bool,
) {
    let anim = load_anim(&json_path).await;
    if !font_dirs.is_empty() {
        renderer_skia::Resources::get().load_font_directories(&font_dirs);
    }
    if !font_aliases.is_empty() {
        renderer_skia::Resources::get()
            .set_font_aliases(&font_aliases.into_iter().collect::<HashMap<_, _>>());
    }
    render_anim_to_dir(
        anim,
        output_dir,
        threads,
        frames,
        target_resolution,
        write_tree,
    )
    .await;
}

struct VideoOptions {
    target_resolution: Option<(u32, u32)>,
    fps: u32,
    codec: String,
    crf: u32,
}

async fn run_render_video(
    json_path: PathBuf,
    output_file: PathBuf,
    threads: Option<usize>,
    font_dirs: Vec<PathBuf>,
    font_aliases: Vec<(String, String)>,
    opts: VideoOptions,
) {
    let anim = load_anim(&json_path).await;
    if !font_dirs.is_empty() {
        renderer_skia::Resources::get().load_font_directories(&font_dirs);
    }
    if !font_aliases.is_empty() {
        renderer_skia::Resources::get()
            .set_font_aliases(&font_aliases.into_iter().collect::<HashMap<_, _>>());
    }
    crate::render::render_anim_to_video(
        anim,
        output_file,
        threads,
        opts.target_resolution,
        opts.fps,
        opts.codec,
        opts.crf,
    )
    .await;
}

async fn run_render_pdf(
    json_path: PathBuf,
    output_file: PathBuf,
    frames: Option<Vec<u32>>,
    font_dirs: Vec<PathBuf>,
    font_aliases: Vec<(String, String)>,
) {
    let anim = load_anim(&json_path).await;
    if !font_dirs.is_empty() {
        renderer_skia::Resources::get().load_font_directories(&font_dirs);
    }
    if !font_aliases.is_empty() {
        renderer_skia::Resources::get()
            .set_font_aliases(&font_aliases.into_iter().collect::<HashMap<_, _>>());
    }
    crate::render::render_anim_to_pdf(anim, frames, output_file).await;
}

async fn load_anim(json_path: &PathBuf) -> AnimationDef {
    let json_str = match tokio::fs::read_to_string(json_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: failed to read {}: {e}", json_path.display());
            std::process::exit(1);
        }
    };
    match AnimationDef::from_json(&json_str) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: failed to parse animation JSON: {e}");
            std::process::exit(1);
        }
    }
}

async fn run_init(directory: PathBuf) {
    let r = "\x1b[0m"; // reset
    let b = "\x1b[1m"; // bold
    let pink = "\x1b[95m"; // bright magenta
    let blue = "\x1b[94m"; // bright blue
    let _gray = "\x1b[90m"; // dark gray
    let grn = "\x1b[92m"; // bright green
    let yel = "\x1b[93m"; // bright yellow
    let red = "\x1b[91m"; // bright red

    println!();
    println!("  {b}{pink}Fairy{r}");
    println!("  {b}{blue}   Flow{r}");
    println!();

    if let Err(e) = tokio::fs::create_dir_all(&directory).await {
        eprintln!(
            "  {red}{b}error:{r} failed to create directory {}: {e}",
            directory.display()
        );
        std::process::exit(1);
    }

    for subdir in &["scenes", "sequences"] {
        let path = directory.join(subdir);
        if let Err(e) = tokio::fs::create_dir_all(&path).await {
            eprintln!(
                "  {red}{b}error:{r} failed to create directory {}: {e}",
                path.display()
            );
            std::process::exit(1);
        }
    }

    let files: &[(&str, &str)] = &[
        (
            "fairyflow.toml",
            "fps = 24\n\n# Prologue is automatically included into any scene file\nprologue = \"prologue.py\"\n",
        ),
        (
            "prologue.py",
            "from fairyflow import *\n\nset_default_scene(width=300, height=200, color=\"white\", cue_at_start=True)\n",
        ),
        (
            "scenes/scene1.ffpy",
            "with Scene():\n    stext(\"Hello world!\").fade_out()\n",
        ),
        (
            "sequences/sequence1.ffsq",
            "{\n  \"scene_files\": [\n    \"scenes/scene1.ffpy\"\n  ]\n}\n",
        ),
    ];

    for (name, content) in files {
        let path = directory.join(name);
        if path.exists() {
            eprintln!(
                "  {yel}{b}warning:{r} {} already exists, skipping",
                path.display()
            );
            continue;
        }
        if let Err(e) = tokio::fs::write(&path, content).await {
            eprintln!(
                "  {red}{b}error:{r} failed to create {}: {e}",
                path.display()
            );
            std::process::exit(1);
        }
        println!("  {grn}created{r} {}", path.display());
    }

    println!(
        "\n  {b}{grn}initialized{r} project in {b}{}{r}",
        directory.display()
    );
    println!(
        "\n  open your project with:\n\n     {b}fairyflow open {}{r}",
        directory.display()
    );
    println!();
}
