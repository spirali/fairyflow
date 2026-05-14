use crate::config::PackageConfig;
use engine::{AnimationDef, FrameId, SceneSelection};
use rayon::prelude::*;
use softbuffer::{Context, Surface};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, mpsc};
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window, WindowId};

// ── Temp dir guard ─────────────────────────────────────────────────────────

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

// ── Frame cache ────────────────────────────────────────────────────────────

struct FrameCache {
    frames: HashMap<u32, Vec<u32>>,
    width: u32,
    height: u32,
}

impl FrameCache {
    fn new() -> Self {
        Self { frames: HashMap::new(), width: 0, height: 0 }
    }
}

// ── Background cache thread ────────────────────────────────────────────────

struct CacheRequest {
    current_frame: u32,
    going_forward: bool,
    window_w: u32,
    window_h: u32,
}

fn run_cache_thread(
    rx: mpsc::Receiver<CacheRequest>,
    cache: Arc<RwLock<FrameCache>>,
    animations: Arc<Vec<AnimationDef>>,
    frame_map: Arc<Vec<(usize, u32)>>,
    lookahead: u32,
    lookback: u32,
) {
    loop {
        let mut req = match rx.recv() {
            Ok(r) => r,
            Err(_) => return,
        };
        // Drain stale requests, keep only the latest.
        while let Ok(newer) = rx.try_recv() {
            req = newer;
        }

        let CacheRequest { current_frame, going_forward, window_w, window_h } = req;
        let total = frame_map.len() as u32;
        if total == 0 || window_w == 0 || window_h == 0 {
            continue;
        }

        let ahead_end = (current_frame + lookahead).min(total.saturating_sub(1));
        let back_start = current_frame.saturating_sub(lookback);

        // Invalidate cache when the window size has changed.
        {
            let mut c = cache.write().unwrap();
            if c.width != window_w || c.height != window_h {
                c.frames.clear();
                c.width = window_w;
                c.height = window_h;
            }
        }

        // Collect frames that still need rendering, prioritizing the play direction.
        let to_render: Vec<u32> = {
            let c = cache.read().unwrap();
            let ahead = (current_frame + 1..=ahead_end).filter(|f| !c.frames.contains_key(f));
            let behind = (back_start..current_frame).filter(|f| !c.frames.contains_key(f));
            if going_forward {
                ahead.chain(behind).collect()
            } else {
                behind.rev().chain(ahead.rev()).collect()
            }
        };

        if to_render.is_empty() {
            continue;
        }

        // Render missing frames in parallel.
        let rendered: Vec<(u32, Vec<u32>)> = to_render
            .par_iter()
            .filter_map(|&n| {
                let (ai, lf) = frame_map[n as usize];
                let scene = animations[ai]
                    .build_scene(FrameId::new(lf), SceneSelection::All)
                    .ok()?;
                let mut pixels = vec![0u32; (window_w * window_h) as usize];
                renderer_skia::render_scene_to_buffer(&scene, window_w, window_h, &mut pixels);
                Some((n, pixels))
            })
            .collect();

        // Store results and evict frames now outside the window.
        {
            let mut c = cache.write().unwrap();
            for (n, px) in rendered {
                c.frames.insert(n, px);
            }
            c.frames.retain(|&k, _| k >= back_start && k <= ahead_end);
        }

        renderer_skia::prune_text_cache();
    }
}

// ── Package loading ────────────────────────────────────────────────────────

struct LoadedPackage {
    animations: Vec<AnimationDef>,
    fps: u32,
    scene_width: u32,
    scene_height: u32,
    _temp_dir: TempDirGuard,
}

fn load_package(path: &Path) -> anyhow::Result<LoadedPackage> {
    let file = std::fs::File::open(path)
        .map_err(|e| anyhow::anyhow!("cannot open {}: {e}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)?;

    // Read package config
    let config: PackageConfig = {
        let mut f = archive.by_name("ffpackage.json")?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        serde_json::from_str(&s)?
    };

    // Create temp dir for extracted images
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let temp_dir = std::env::temp_dir().join(format!("fairyflow-player-{ts}"));
    std::fs::create_dir_all(&temp_dir)?;
    let temp_guard = TempDirGuard(temp_dir.clone());

    // Extract all images from the archive into the temp dir
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("images/") && !name.ends_with('/') {
            let dest = temp_dir.join(&name);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }

    // Build JSON path replacements: replace quoted original path with quoted temp path.
    // Using the quoted form prevents accidental partial-string matches.
    let path_replacements: Vec<(String, String)> = config
        .image_map
        .iter()
        .map(|(original, archive_path)| {
            let temp_path = temp_dir.join(archive_path);
            let temp_str = temp_path.to_string_lossy().replace('\\', "/");
            (format!("\"{}\"", original), format!("\"{}\"", temp_str))
        })
        .collect();

    // Load, patch, and parse each scene
    let mut animations: Vec<AnimationDef> = Vec::with_capacity(config.scenes.len());
    for scene_archive_path in &config.scenes {
        let mut f = archive.by_name(scene_archive_path)?;
        let mut scene_json = String::new();
        f.read_to_string(&mut scene_json)?;

        for (from, to) in &path_replacements {
            scene_json = scene_json.replace(from.as_str(), to.as_str());
        }

        animations.push(AnimationDef::from_json(&scene_json)?);
    }

    if animations.is_empty() {
        return Err(anyhow::anyhow!("package contains no scenes"));
    }

    // Determine scene dimensions from frame 0 of the first animation
    let (scene_width, scene_height) = {
        let scene = animations[0].build_scene(FrameId::new(0), SceneSelection::All)?;
        (scene.width.round() as u32, scene.height.round() as u32)
    };

    Ok(LoadedPackage {
        animations,
        fps: config.fps,
        scene_width,
        scene_height,
        _temp_dir: temp_guard,
    })
}

// ── Winit application ──────────────────────────────────────────────────────

struct PlayerApp {
    // Loaded animation data (owns temp dir via _temp_dir)
    frame_map: Arc<Vec<(usize, u32)>>,
    animations: Arc<Vec<AnimationDef>>,
    cue_frames: HashSet<u32>,
    fps: u32,
    scene_width: u32,
    scene_height: u32,
    // Playback state
    current_frame: u32,
    paused: bool,
    backward: bool,
    last_frame_time: Instant,
    // Window/surface (created in `resumed`)
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    // Pre-render cache
    frame_cache: Arc<RwLock<FrameCache>>,
    cache_tx: mpsc::Sender<CacheRequest>,
    // Keeps temp dir alive for the session
    _temp_dir: TempDirGuard,
}

impl PlayerApp {
    fn total_frames(&self) -> u32 {
        self.frame_map.len() as u32
    }

    fn advance_frame(&mut self) {
        if self.backward {
            if self.current_frame == 0 {
                self.paused = true;
                return;
            }
            self.current_frame -= 1;
            if self.current_frame == 0 || self.cue_frames.contains(&self.current_frame) {
                self.paused = true;
            }
        } else {
            let total = self.total_frames();
            if total == 0 || self.current_frame + 1 >= total {
                self.paused = true;
                return;
            }
            self.current_frame += 1;
            if self.cue_frames.contains(&self.current_frame) {
                self.paused = true;
            }
        }
    }

    /// Jump to the nearest cue frame strictly after `current_frame`, or to the
    /// last frame if none exists. Pauses and sets direction to forward.
    fn jump_to_next_cue(&mut self) {
        let last = self.total_frames().saturating_sub(1);
        let target = self
            .cue_frames
            .iter()
            .filter(|&&cf| cf > self.current_frame)
            .min()
            .copied()
            .unwrap_or(last);
        self.current_frame = target;
        self.backward = false;
        self.paused = true;
    }

    /// Jump to the nearest cue frame strictly before `current_frame`, or to
    /// frame 0 if none exists. Pauses and sets direction to backward.
    fn jump_to_prev_cue(&mut self) {
        let target = self
            .cue_frames
            .iter()
            .filter(|&&cf| cf < self.current_frame)
            .max()
            .copied()
            .unwrap_or(0);
        self.current_frame = target;
        self.backward = true;
        self.paused = true;
    }

    fn render(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }

        let (win_w, win_h) = (size.width, size.height);

        // Try to serve from the pre-render cache.
        let cached: Option<Vec<u32>> = {
            let c = self.frame_cache.read().unwrap();
            if c.width == win_w && c.height == win_h {
                c.frames.get(&self.current_frame).cloned()
            } else {
                None
            }
        };

        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };

        if let Some(pixels) = cached {
            buffer.copy_from_slice(&pixels);
        } else {
            // Fallback: render synchronously on this frame.
            let (anim_idx, local_frame) = self.frame_map[self.current_frame as usize];
            let Ok(scene) = self.animations[anim_idx]
                .build_scene(FrameId::new(local_frame), SceneSelection::All)
            else {
                return;
            };
            renderer_skia::render_scene_to_buffer(&scene, win_w, win_h, &mut buffer);
        }

        let _ = buffer.present();

        // Ask the background thread to pre-render surrounding frames.
        let _ = self.cache_tx.send(CacheRequest {
            current_frame: self.current_frame,
            going_forward: !self.backward,
            window_w: win_w,
            window_h: win_h,
        });
    }
}

impl ApplicationHandler for PlayerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_size = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .map(|m| {
                let s = m.size();
                PhysicalSize::new(s.width / 2, s.height / 2)
            })
            .unwrap_or_else(|| PhysicalSize::new(self.scene_width, self.scene_height));

        let attrs = Window::default_attributes()
            .with_title("FairyFlow Player")
            .with_inner_size(window_size);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("error: failed to create window: {e}");
                event_loop.exit();
                return;
            }
        };
        let context = match Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: failed to create graphics context: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: failed to create surface: {e}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match code {
                KeyCode::Escape => {
                    if let Some(w) = &self.window {
                        if w.fullscreen().is_some() {
                            w.set_fullscreen(None);
                        } else {
                            event_loop.exit();
                        }
                    } else {
                        event_loop.exit();
                    }
                }
                KeyCode::F5 => {
                    if let Some(w) = &self.window {
                        w.set_fullscreen(Some(Fullscreen::Borderless(None)));
                    }
                }
                KeyCode::ArrowRight => {
                    if self.paused {
                        // Resume forward playback
                        self.backward = false;
                        self.paused = false;
                        self.advance_frame();
                    } else if !self.backward {
                        // Playing forward → jump to next cue and pause
                        self.jump_to_next_cue();
                    } else {
                        // Playing backward → reverse direction to forward
                        self.backward = false;
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                KeyCode::ArrowLeft => {
                    if self.paused {
                        // Resume backward playback
                        self.backward = true;
                        self.paused = false;
                        self.advance_frame();
                    } else if self.backward {
                        // Playing backward → jump to previous cue and pause
                        self.jump_to_prev_cue();
                    } else {
                        // Playing forward → reverse direction to backward
                        self.backward = true;
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                KeyCode::Home => {
                    self.current_frame = 0;
                    self.paused = true;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                KeyCode::End => {
                    self.current_frame = self.total_frames().saturating_sub(1);
                    self.paused = true;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.paused || self.total_frames() == 0 {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let frame_duration = Duration::from_secs_f64(1.0 / self.fps.max(1) as f64);
        let now = Instant::now();
        if now.duration_since(self.last_frame_time) >= frame_duration {
            self.last_frame_time = now;
            self.advance_frame();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            self.last_frame_time + frame_duration,
        ));
    }
}

// ── Public entry point ─────────────────────────────────────────────────────

pub fn open_player(package_path: &Path, lookahead: u32, lookback: u32) -> anyhow::Result<()> {
    renderer_skia::Resources::init();

    let package = load_package(package_path)?;

    // Build global frame map: (animation_index, local_frame) per global frame
    let mut frame_map: Vec<(usize, u32)> = Vec::new();
    let mut cue_frames: HashSet<u32> = HashSet::new();

    for (anim_idx, anim) in package.animations.iter().enumerate() {
        let base_offset = frame_map.len() as u32;
        for local_frame in 0..anim.frame_count(SceneSelection::All) {
            frame_map.push((anim_idx, local_frame));
        }
        // Collect cue frames with global offsets
        let mut within_anim = 0u32;
        for si in anim.scene_infos() {
            for &cf in &si.cue_frames {
                cue_frames.insert(base_offset + within_anim + cf);
            }
            within_anim += si.frame_count;
        }
    }

    if frame_map.is_empty() {
        return Err(anyhow::anyhow!("package has no frames to play"));
    }

    let LoadedPackage { animations, fps, scene_width, scene_height, _temp_dir } = package;

    let animations = Arc::new(animations);
    let frame_map = Arc::new(frame_map);

    let frame_cache = Arc::new(RwLock::new(FrameCache::new()));
    let (cache_tx, cache_rx) = mpsc::channel::<CacheRequest>();

    // Spawn background pre-rendering thread.
    {
        let cache = Arc::clone(&frame_cache);
        let anims = Arc::clone(&animations);
        let fmap = Arc::clone(&frame_map);
        std::thread::spawn(move || {
            run_cache_thread(cache_rx, cache, anims, fmap, lookahead, lookback);
        });
    }

    let mut app = PlayerApp {
        frame_map,
        animations,
        cue_frames,
        fps,
        scene_width,
        scene_height,
        current_frame: 0,
        paused: true,
        backward: false,
        last_frame_time: Instant::now(),
        window: None,
        surface: None,
        frame_cache,
        cache_tx,
        _temp_dir,
    };

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
