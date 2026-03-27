use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::{debug, trace};

const MAX_CACHE_SIZE: usize = 2048;

/// Backend-independent path verb for a glyph outline.
#[derive(Debug, Clone)]
pub enum PathVerb {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    QuadTo(f32, f32, f32, f32),
    CubicTo(f32, f32, f32, f32, f32, f32),
    Close,
}

/// Backend-independent glyph outline, positioned with y_cursor = 0.
#[derive(Debug, Clone)]
pub struct VectorPath {
    pub verbs: Vec<PathVerb>,
}

/// One glyph placed within a cached line.
#[derive(Debug)]
pub struct CachedGlyph {
    /// Index into the span slice used when the line was built.
    pub span_idx: usize,
    /// Left edge of this glyph within the line (for position queries).
    pub x: f32,
    /// Outline at y_cursor = 0 with (run_x + glyph.x, baseline - glyph.y) applied.
    pub path: VectorPath,
}

/// A fully laid-out line of text, ready to render or measure.
#[derive(Debug)]
pub struct CachedLine {
    pub width: f32,
    pub height: f32,
    pub glyphs: Vec<CachedGlyph>,
}

/// Normalized cache key for one span: text content + font properties only.
/// No node IDs or colors, so the same text from different nodes shares one entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpanKey {
    pub text: Arc<String>,
    pub font_family: Arc<String>,
    /// `f32::to_bits()` of the font size — gives `Hash`/`Eq` without float concerns.
    pub font_size_bits: u32,
    pub italic: bool,
}

/// Ordered slice of span keys describing a complete line.
pub type LineKey = Box<[SpanKey]>;

struct CacheEntry {
    last_used: u64,
    line: Arc<CachedLine>,
}

struct GlyphCache {
    entries: HashMap<LineKey, CacheEntry>,
    clock: u64,
}

static CACHE: OnceLock<Mutex<GlyphCache>> = OnceLock::new();

fn cache() -> &'static Mutex<GlyphCache> {
    CACHE.get_or_init(|| {
        Mutex::new(GlyphCache {
            entries: HashMap::new(),
            clock: 0,
        })
    })
}

/// Look up a cached line.  Bumps `last_used` on hit.
pub fn cache_get(key: &[SpanKey]) -> Option<Arc<CachedLine>> {
    let mut guard = cache().lock().unwrap();
    guard.clock += 1;
    let clock = guard.clock;
    let cache_size = guard.entries.len();
    if let Some(entry) = guard.entries.get_mut(key) {
        entry.last_used = clock;
        trace!(cache_size, "glyph cache hit");
        Some(Arc::clone(&entry.line))
    } else {
        trace!(cache_size, "glyph cache miss");
        None
    }
}

/// Store a newly built line and return an `Arc` to it.
pub fn cache_store(key: LineKey, line: CachedLine) -> Arc<CachedLine> {
    let line = Arc::new(line);
    let mut guard = cache().lock().unwrap();
    guard.clock += 1;
    let clock = guard.clock;
    guard.entries.insert(
        key,
        CacheEntry {
            last_used: clock,
            line: Arc::clone(&line),
        },
    );
    trace!(cache_size = guard.entries.len(), "glyph cache store");
    line
}

/// Drop the least-recently-used entries until the cache holds at most `MAX_CACHE_SIZE` lines.
/// Call this after each HTTP request completes, not during rendering.
pub fn prune_text_cache() {
    let max_size = MAX_CACHE_SIZE;
    let mut guard = cache().lock().unwrap();
    let before = guard.entries.len();
    if before <= max_size {
        return;
    }
    let to_remove = before - max_size;
    let mut ages: Vec<u64> = guard.entries.values().map(|e| e.last_used).collect();
    ages.sort_unstable();
    let cut_off_age = ages[to_remove - 1];
    guard.entries.retain(|_, e| e.last_used > cut_off_age);
    debug!(
        removed = before - guard.entries.len(),
        remaining = guard.entries.len(),
        "glyph cache pruned"
    );
}
