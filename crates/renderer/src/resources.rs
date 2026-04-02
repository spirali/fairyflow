use fontdb::Database;
use parley::FontContext;
use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache, SourceCacheOptions};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

static GLOBAL: OnceLock<Resources> = OnceLock::new();

pub struct Resources {
    font_cx: Mutex<FontContext>,
    fontdb: Mutex<FontDbState>,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

/// Tracks the accumulated set of font directories so the database can be
/// rebuilt from scratch whenever new directories are added (fontdb::Database
/// does not implement Clone, so we rebuild rather than patch in place).
struct FontDbState {
    dirs: Vec<PathBuf>,
    db: Arc<Database>,
}

impl FontDbState {
    fn build(dirs: &[PathBuf]) -> Arc<Database> {
        let mut db = Database::new();
        db.load_system_fonts();
        for dir in dirs {
            db.load_fonts_dir(dir);
        }
        Arc::new(db)
    }
}

impl Resources {
    pub fn init() {
        GLOBAL.get_or_init(|| {
            let fontdb_state = FontDbState {
                dirs: Vec::new(),
                db: FontDbState::build(&[]),
            };
            Resources {
                font_cx: Mutex::new(FontContext {
                    collection: Collection::new(CollectionOptions {
                        shared: true,
                        system_fonts: true,
                    }),
                    source_cache: SourceCache::new(SourceCacheOptions { shared: true }),
                }),
                fontdb: Mutex::new(fontdb_state),
                syntax_set: SyntaxSet::load_defaults_newlines(),
                theme_set: ThemeSet::load_defaults(),
            }
        });
    }

    pub fn get() -> &'static Resources {
        GLOBAL
            .get()
            .expect("renderer::Resources::init() must be called before rendering")
    }

    pub fn font_cx(&self) -> FontContext {
        self.font_cx.lock().unwrap().clone()
    }

    /// Returns a reference-counted handle to the current font database.
    /// Cheap to call — just clones an `Arc`.
    pub fn fontdb(&self) -> Arc<Database> {
        Arc::clone(&self.fontdb.lock().unwrap().db)
    }

    /// Load all font files found (recursively) in the given directories into
    /// both the parley `FontContext` and the usvg `fontdb::Database`.
    pub fn load_font_directories(&self, dirs: &[impl AsRef<std::path::Path>]) {
        const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc", "woff", "woff2"];

        let mut font_cx_guard = self.font_cx.lock().unwrap();
        let mut fontdb_guard = self.fontdb.lock().unwrap();

        let mut added_any = false;
        for dir in dirs {
            let dir = dir.as_ref().to_path_buf();
            if fontdb_guard.dirs.contains(&dir) {
                continue;
            }
            fontdb_guard.dirs.push(dir.clone());
            added_any = true;

            for path in walkdir(&dir) {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase());
                if ext
                    .as_deref()
                    .map_or(false, |e| FONT_EXTENSIONS.contains(&e))
                {
                    match std::fs::read(&path) {
                        Ok(data) => {
                            font_cx_guard
                                .collection
                                .register_fonts(Blob::from(data.clone()), None);
                            tracing::debug!("loaded font {}", path.display());
                        }
                        Err(e) => tracing::warn!("failed to load font {}: {e}", path.display()),
                    }
                }
            }
        }

        if added_any {
            fontdb_guard.db = FontDbState::build(&fontdb_guard.dirs);
        }
    }
}

/// Yields all file paths under `root` recursively (best-effort; skips unreadable dirs).
fn walkdir(root: &std::path::Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
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
