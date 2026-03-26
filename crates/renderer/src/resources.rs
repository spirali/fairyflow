use std::sync::{Mutex, OnceLock};
use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache, SourceCacheOptions};

static GLOBAL: OnceLock<Resources> = OnceLock::new();

pub struct Resources {
    font_cx: Mutex<FontContext>,
}

impl Resources {
    pub fn init() {
        GLOBAL.get_or_init(|| Resources {
            font_cx: Mutex::new(FontContext {
                collection: Collection::new(CollectionOptions {
                    shared: true,
                    system_fonts: true,
                }),
                source_cache: SourceCache::new(SourceCacheOptions {
                    shared: true,
                }),
            }),
        });
    }

    pub fn get() -> &'static Resources {
        GLOBAL.get().expect("renderer::Resources::init() must be called before rendering")
    }

    pub fn font_cx(&self) -> FontContext {
        self.font_cx.lock().unwrap().clone()
    }

    /// Load all font files found (recursively) in the given directories.
    pub fn load_font_directories(&self, dirs: &[impl AsRef<std::path::Path>]) {
        const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2"];
        let mut guard = self.font_cx.lock().unwrap();
        for dir in dirs {
            let dir = dir.as_ref();
            let walker = walkdir(dir);
            for entry in walker {
                let path = entry.as_path();
                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase());
                if ext.as_deref().map_or(false, |e| FONT_EXTENSIONS.contains(&e)) {
                    match std::fs::read(path) {
                        Ok(data) => {
                            guard.collection.register_fonts(Blob::from(data), None);
                            tracing::debug!("loaded font {}", path.display());
                        }
                        Err(e) => tracing::warn!("failed to load font {}: {e}", path.display()),
                    }
                }
            }
        }
    }
}

/// Yields all file paths under `root` recursively (best-effort; skips unreadable dirs).
fn walkdir(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                result.push(path);
            }
        }
    }
    result
}
