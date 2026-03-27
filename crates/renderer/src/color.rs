#[derive(Debug, Clone, Default)]
pub struct Color(csscolorparser::Color);

impl serde::Serialize for Color {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0.to_css_hex())
    }
}

impl Color {
    pub fn from_html(color: &str) -> Option<Color> {
        csscolorparser::Color::from_html(color).ok().map(Color)
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.0.a = alpha;
    }

    pub fn interpolate(&self, other: &Color, t: f64) -> Color {
        let t = t as f32;
        let a = &self.0;
        let b = &other.0;
        Color(csscolorparser::Color {
            r: a.r + t * (b.r - a.r),
            g: a.g + t * (b.g - a.g),
            b: a.b + t * (b.b - a.b),
            a: a.a + t * (b.a - a.a),
        })
    }

    pub(crate) fn to_skia_color(&self) -> tiny_skia::Color {
        let c = &self.0;
        tiny_skia::Color::from_rgba(c.r as f32, c.g as f32, c.b as f32, c.a as f32)
            .unwrap_or(tiny_skia::Color::BLACK)
    }
}
