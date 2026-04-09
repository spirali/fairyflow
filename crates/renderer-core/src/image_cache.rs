use crate::resources::Resources;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tracing::debug;

/// Backend-independent decoded raster image (premultiplied RGBA8888).
pub struct RawPixmap {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct OraLayer {
    pub name: String,
    pub pixmap: RawPixmap,
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
        pixmap: RawPixmap,
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

/// Dispatch: load any supported image format (SVG, PNG, JPEG, ORA) from disk.
pub fn load_image(path: &str) -> Option<Arc<CachedImage>> {
    if let Some(cached) = cache_get(path) {
        return Some(cached);
    }
    let data = std::fs::read(path).ok()?;
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "svg" | "svgz" => load_svg_from_data(path, data),
        "png" | "jpg" | "jpeg" => load_raster_from_data(path, data),
        "ora" => load_ora_from_data(path, data),
        _ => None,
    }
}

pub fn load_svg_from_data(path: &str, data: Vec<u8>) -> Option<Arc<CachedImage>> {
    let opt = usvg::Options {
        fontdb: Resources::get().fontdb(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&data, &opt).ok()?;
    let svg_size = tree.size();
    let image_layers = Arc::new(svg_layer_labels(&data));
    let cached = CachedImage {
        kind: CachedImageKind::Svg { tree, raw_data: data },
        width: svg_size.width(),
        height: svg_size.height(),
        image_layers: Some(image_layers),
    };
    Some(cache_store(path.to_string(), cached))
}

fn load_raster_from_data(path: &str, data: Vec<u8>) -> Option<Arc<CachedImage>> {
    let img = image::load_from_memory(&data).ok()?;
    let width = img.width() as f32;
    let height = img.height() as f32;
    let pixmap = image_to_raw_pixmap(img)?;
    let cached = CachedImage {
        kind: CachedImageKind::Raster { pixmap },
        width,
        height,
        image_layers: None,
    };
    Some(cache_store(path.to_string(), cached))
}

fn load_ora_from_data(path: &str, data: Vec<u8>) -> Option<Arc<CachedImage>> {
    use std::io::Read;
    let cursor = std::io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;

    let stack_xml = {
        let mut file = archive.by_name("stack.xml").ok()?;
        let mut buf = String::new();
        file.read_to_string(&mut buf).ok()?;
        buf
    };
    let root = xmltree::Element::parse(std::io::Cursor::new(stack_xml.as_bytes())).ok()?;
    let width: f32 = root.attributes.get("w")?.parse().ok()?;
    let height: f32 = root.attributes.get("h")?.parse().ok()?;

    let stack_elem = root.children.iter().find_map(|child| {
        if let xmltree::XMLNode::Element(elem) = child {
            if elem.name == "stack" { Some(elem) } else { None }
        } else {
            None
        }
    })?;

    // Collect layer metadata in top-to-bottom stack.xml order.
    let mut layers_info: Vec<(String, String, i32, i32)> = Vec::new();
    for child in &stack_elem.children {
        let xmltree::XMLNode::Element(elem) = child else { continue };
        if elem.name != "layer" {
            continue;
        }
        let name = elem.attributes.get("name").cloned().unwrap_or_default();
        let src = elem.attributes.get("src").cloned().unwrap_or_default();
        let x: i32 = elem.attributes.get("x").and_then(|v| v.parse().ok()).unwrap_or(0);
        let y: i32 = elem.attributes.get("y").and_then(|v| v.parse().ok()).unwrap_or(0);
        layers_info.push((name, src, x, y));
    }

    // Store layer names bottom-to-top (consistent with SVG document order).
    let image_layers: Vec<String> = layers_info.iter().rev().map(|(n, ..)| n.clone()).collect();

    // Decode layer PNGs and store in bottom-to-top render order.
    let mut ora_layers: Vec<OraLayer> = Vec::new();
    for (name, src, x, y) in layers_info.iter().rev() {
        let png_data = {
            let Ok(mut file) = archive.by_name(src) else { continue };
            let mut buf = Vec::new();
            if file.read_to_end(&mut buf).is_err() {
                continue;
            }
            buf
        };
        let Ok(img) = image::load_from_memory(&png_data) else { continue };
        let Some(pixmap) = image_to_raw_pixmap(img) else { continue };
        ora_layers.push(OraLayer { name: name.clone(), pixmap, x: *x, y: *y });
    }

    let cached = CachedImage {
        kind: CachedImageKind::Ora { layers: ora_layers },
        width,
        height,
        image_layers: Some(Arc::new(image_layers)),
    };
    Some(cache_store(path.to_string(), cached))
}

/// Convert a decoded `DynamicImage` to a `RawPixmap` with premultiplied alpha.
pub fn image_to_raw_pixmap(img: image::DynamicImage) -> Option<RawPixmap> {
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let data: Vec<u8> = rgba
        .pixels()
        .flat_map(|p| {
            let [r, g, b, a] = p.0;
            let pm = |c: u8| (c as u32 * a as u32 / 255) as u8;
            [pm(r), pm(g), pm(b), a]
        })
        .collect();
    Some(RawPixmap { data, width, height })
}

/// Return the `inkscape:label` values of all direct-child `<g>` layer elements
/// in the SVG, in document order.  Elements without a label are skipped.
pub fn svg_layer_labels(data: &[u8]) -> Vec<String> {
    let Ok(root) = xmltree::Element::parse(std::io::Cursor::new(data)) else {
        return Vec::new();
    };
    root.children
        .iter()
        .filter_map(|child| {
            let xmltree::XMLNode::Element(elem) = child else { return None };
            if elem.name != "g" {
                return None;
            }
            inkscape_label(elem).map(str::to_owned)
        })
        .collect()
}

/// Return a modified copy of the SVG bytes where every direct-child `<g>`
/// element that carries an `inkscape:label` attribute whose value does **not**
/// equal `target_label` is hidden by setting `display:none` in its `style`.
pub fn svg_show_only_layer(data: &[u8], target_label: &str) -> Vec<u8> {
    let Ok(mut root) = xmltree::Element::parse(std::io::Cursor::new(data)) else {
        return data.to_vec();
    };

    for child in &mut root.children {
        let xmltree::XMLNode::Element(elem) = child else { continue };
        if elem.name != "g" {
            continue;
        }
        let Some(label) = inkscape_label(elem) else { continue };
        if label == target_label {
            continue;
        }
        let current = elem.attributes.get("style").cloned().unwrap_or_default();
        elem.attributes.insert("style".to_string(), css_display_none(&current));
    }

    let mut output = Vec::new();
    root.write(&mut output).ok();
    output
}

fn inkscape_label(elem: &xmltree::Element) -> Option<&str> {
    elem.attributes.get("label").map(String::as_str)
}

fn css_display_none(style: &str) -> String {
    let mut parts: Vec<&str> = style
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty() && !s.starts_with("display"))
        .collect();
    parts.push("display:none");
    parts.join(";")
}

/// Load (or create from cache) a single-layer view of the SVG at `path`.
pub fn load_svg_layer(path: &str, layer_label: &str) -> Option<Arc<CachedImage>> {
    let cache_key = format!("{}\0{}", path, layer_label);
    if let Some(cached) = cache_get(&cache_key) {
        return Some(cached);
    }

    let base = load_image(path)?;
    let raw_data = match &base.kind {
        CachedImageKind::Svg { raw_data, .. } => raw_data.clone(),
        _ => return None,
    };

    let modified = svg_show_only_layer(&raw_data, layer_label);

    let opt = usvg::Options {
        fontdb: Resources::get().fontdb(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(&modified, &opt).ok()?;
    let svg_size = tree.size();
    let cached = CachedImage {
        kind: CachedImageKind::Svg { tree, raw_data: modified },
        width: svg_size.width(),
        height: svg_size.height(),
        image_layers: None,
    };
    Some(cache_store(cache_key, cached))
}

/// Return the natural (intrinsic) pixel size of an image.
pub fn measure_image(path: &str) -> Option<(f32, f32)> {
    let cached = load_image(path)?;
    Some((cached.width, cached.height))
}

/// Return all layer names for the image at `path`, in document order.
pub fn svg_image_layers(path: &str) -> Arc<Vec<String>> {
    load_image(path).map(|c| c.image_layers.clone().unwrap_or_default()).unwrap_or_default()
}
