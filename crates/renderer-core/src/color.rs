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

    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color(csscolorparser::Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        })
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

    /// Return the raw RGBA components in [0.0, 1.0] range.
    pub fn to_rgba_f32(&self) -> (f32, f32, f32, f32) {
        let c = &self.0;
        (c.r, c.g, c.b, c.a)
    }

    pub fn is_transparent(&self) -> bool {
        self.0.a == 0.0
    }
}
