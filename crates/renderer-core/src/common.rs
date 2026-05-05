#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dimensions {
    pub x: f64,
    pub y: f64,
}

impl Dimensions {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}
