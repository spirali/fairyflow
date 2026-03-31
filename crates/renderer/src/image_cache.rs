use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::debug;

pub struct CachedImage {
    pub tree: usvg::Tree,
    pub width: f32,
    pub height: f32,
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
    let mut guard = cache().lock().unwrap();
    guard.entries.insert(path, Arc::clone(&image));
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
