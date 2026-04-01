use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tiny_skia::Pixmap;
use tracing::debug;

pub struct OraLayer {
    pub name: String,
    pub pixmap: Pixmap,
    pub x: i32,
    pub y: i32,
}

pub enum CachedImageKind {
    Svg {
        tree: usvg::Tree,
        /// Raw SVG bytes, kept so individual layers can be extracted.
        raw_data: Vec<u8>,
    },
    /// Decoded raster image (PNG or JPEG).  No layer support.
    Raster {
        pixmap: Pixmap,
    },
    /// Open Raster (ORA) image.  Layers are stored in bottom-to-top render order.
    Ora {
        layers: Vec<OraLayer>,
    },
}

pub struct CachedImage {
    pub kind: CachedImageKind,
    pub width: f32,
    pub height: f32,
    /// Layer names in document order (bottom-to-top for SVG/ORA).  Empty for JPEG/PNG.
    pub image_layers: Option<Arc<Vec<String>>>,
}

struct ImageCache {
    entries: HashMap<String, Arc<CachedImage>>,
}

static CACHE: OnceLock<Mutex<ImageCache>> = OnceLock::new();

fn cache() -> &'static Mutex<ImageCache> {
    CACHE.get_or_init(|| {
        Mutex::new(ImageCache {
            entries: HashMap::new(),
        })
    })
}

/// Look up a cached image by path.
pub fn cache_get(path: &str) -> Option<Arc<CachedImage>> {
    let guard = cache().lock().unwrap();
    guard.entries.get(path).map(Arc::clone)
}

/// Store a parsed image and return an `Arc` to it.
pub fn cache_store(path: String, image: CachedImage) -> Arc<CachedImage> {
    let image = Arc::new(image);
    let image2 = image.clone();
    let mut guard = cache().lock().unwrap();
    guard.entries.insert(path, image2);
    image
}

/// Clear all cached images. Call this before each render request since image
/// files may have changed on disk between requests.
pub fn clear_image_cache() {
    let mut guard = cache().lock().unwrap();
    let count = guard.entries.len();
    guard.entries.clear();
    if count > 0 {
        debug!(removed = count, "image cache cleared");
    }
}
