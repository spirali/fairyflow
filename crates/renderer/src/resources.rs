use std::sync::OnceLock;
use parley::FontContext;
use parley::fontique::{Collection, CollectionOptions, SourceCache, SourceCacheOptions};

static GLOBAL: OnceLock<Resources> = OnceLock::new();

pub struct Resources {
    font_cx: FontContext,
}

impl Resources {
    pub fn init() {
        GLOBAL.get_or_init(|| Resources {
            font_cx: FontContext {
                collection: Collection::new(CollectionOptions {
                    shared: true,
                    system_fonts: true,
                }),
                source_cache: SourceCache::new(SourceCacheOptions {
                    shared: true,
                }),
            },
        });
    }

    pub fn get() -> &'static Resources {
        GLOBAL.get().expect("renderer::Resources::init() must be called before rendering")
    }

    pub fn font_cx(&self) -> &FontContext {
        &self.font_cx
    }
}
