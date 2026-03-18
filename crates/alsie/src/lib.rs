use pyo3::prelude::*;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::types::PyType;

#[pyclass(module = "alsie._alsie")]
#[derive(Clone)]
pub struct Color {
    inner: csscolorparser::Color,
}

#[pymethods]
impl Color {
    #[new]
    #[pyo3(signature = (r, g, b, a = 1.0))]
    fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { inner: csscolorparser::Color { r, g, b, a } }
    }

    #[getter]
    fn r(&self) -> f32 { self.inner.r }
    #[getter]
    fn g(&self) -> f32 { self.inner.g }
    #[getter]
    fn b(&self) -> f32 { self.inner.b }
    #[getter]
    fn a(&self) -> f32 { self.inner.a }

    /// Parse a CSS color string or return the value unchanged if already a Color.
    #[classmethod]
    fn parse(_cls: &Bound<'_, PyType>, value: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(c) = value.extract::<Color>() {
            return Ok(c);
        }
        let s: String = value.extract()
            .map_err(|_| PyTypeError::new_err("Color.parse() expects a str or Color"))?;
        csscolorparser::Color::from_html(&s)
            .map(|inner| Color { inner })
            .map_err(|_| PyValueError::new_err(format!("Cannot parse color: {s:?}")))
    }

    /// Linearly interpolate toward `other` by factor `t` (0.0 = self, 1.0 = other).
    fn interpolate_rgb(&self, other: &Color, t: f32) -> Color {
        Color { inner: self.inner.interpolate_rgb(&other.inner, t) }
    }

    fn __str__(&self) -> String {
        self.inner.to_css_hex()
    }

    fn __eq__(&self, other: &Bound<'_, PyAny>) -> bool {
        other.extract::<Color>()
            .map(|c| self.inner.r == c.inner.r && self.inner.g == c.inner.g
                   && self.inner.b == c.inner.b && self.inner.a == c.inner.a)
            .unwrap_or(false)
    }

    fn __repr__(&self) -> String {
        format!("<Color {}>", self.inner.to_css_hex())
    }
}

#[pymodule]
fn _alsie(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Color>()?;
    Ok(())
}
