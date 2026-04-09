use crate::config::PackageConfig;
use engine::{AnimationDef, FrameId, SceneSelection};
use std::collections::HashSet;
use std::io::Read;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use softbuffer::{Context, Surface};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
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

        animations.push(AnimationDef::from_str(&scene_json)?);
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
    // Loaded animation data (owns temp dir via _package)
    frame_map: Vec<(usize, u32)>,
    animations: Vec<AnimationDef>,
    cue_frames: HashSet<u32>,
    fps: u32,
    scene_width: u32,
    scene_height: u32,
    // Playback state
    current_frame: u32,
    paused: bool,
    last_frame_time: Instant,
    // Window/surface (created in `resumed`)
    window: Option<Arc<Window>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    // Keeps temp dir alive for the session
    _temp_dir: TempDirGuard,
}

impl PlayerApp {
    fn total_frames(&self) -> u32 {
        self.frame_map.len() as u32
    }

    fn advance_frame(&mut self) {
        let total = self.total_frames();
        if total == 0 || self.current_frame + 1 >= total {
            return;
        }
        self.current_frame += 1;
        if self.cue_frames.contains(&self.current_frame) {
            self.paused = true;
        }
    }

    fn render(&mut self) {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let (anim_idx, local_frame) = self.frame_map[self.current_frame as usize];
        let Ok(scene) =
            self.animations[anim_idx].build_scene(FrameId::new(local_frame), SceneSelection::All)
        else {
            return;
        };
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        renderer::render_scene_to_buffer(&scene, size.width, size.height, &mut buffer);
        let _ = buffer.present();
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
                KeyCode::ArrowRight if self.paused => {
                    self.paused = false;
                    self.advance_frame();
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

pub fn open_player(package_path: &Path) -> anyhow::Result<()> {
    renderer::Resources::init();

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

    let start_paused = true;

    // Destructure package so animations and temp_dir can be owned separately
    let LoadedPackage { animations, fps, scene_width, scene_height, _temp_dir } = package;

    let mut app = PlayerApp {
        frame_map,
        animations,
        cue_frames,
        fps,
        scene_width,
        scene_height,
        current_frame: 0,
        paused: start_paused,
        last_frame_time: Instant::now(),
        window: None,
        surface: None,
        _temp_dir,
    };

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
