use std::fmt::Debug;
use std::sync::Arc;
use serde::de::DeserializeOwned;
use serde::{de, Deserialize, Deserializer, Serialize};
use crate::avalue::AnimatedValue;
use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use crate::FrameId;
use renderer::Color as RendererColor;

#[derive(Debug, Clone)]
pub struct Color(RendererColor);

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        RendererColor::from_html(&s)
            .map(Color)
            .ok_or_else(|| de::Error::custom(format!("invalid color '{}'", s)))
    }
}

impl Color {
    pub fn into_inner(self) -> RendererColor {
        self.0
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.0.set_alpha(alpha);
    }

    pub fn interpolate(a: &Color, b: &Color, t: f64) -> Color {
        Color(a.0.interpolate(&b.0, t))
    }
}

pub trait Eval<T> {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<T>;
}

pub trait Value: Sized {
    type Call: Eval<Self> + DeserializeOwned + Debug;
    fn recursive_value() -> Self;
    fn interpolate(&self, other: &Self, t: f64) -> Self;
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub enum Expr<T: Value + DeserializeOwned> {
    Const(T),
    Call(T::Call),
    AnimValue(AnimatedValue<T>),
    Inherited { expr: Box<Expr<T>> },
}

// const _ASSERT_SIZE: () = assert!(std::mem::size_of::<Expr<f64>>() == 24);

impl<T: Clone + Value + DeserializeOwned> Eval<T> for Expr<T> {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<T> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Call(call) => call.eval(ctx),
            Expr::Inherited { expr } => expr.eval(ctx),
            Expr::AnimValue(av) => av.eval(ctx),
        }
    }
}


impl Value for f64 {
    type Call = FloatCall;
    fn recursive_value() -> Self {
        0.0f64
    }
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self + t * (*other - *self)
    }
}

impl Expr<f64> {
    pub fn is_default_width_of(&self, node: NodeId) -> bool {
        match self {
            Expr::Call(FloatCall::DefaultWidth { node: n }) => {
                node == *n
            }
            _ => false
        }
    }
    pub fn is_default_height_of(&self, node: NodeId) -> bool {
        match self {
            Expr::Call(FloatCall::DefaultHeight { node: n }) => {
                node == *n
            }
            _ => false
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FloatParamsPair {
    pub a: Expr<f64>,
    pub b: Expr<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CallParamsNodeTransform {
    pub source: NodeId,
    pub target: NodeId,
    pub x: Expr<f64>,
    pub y: Expr<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CallParamsPathPoint {
    pub node: NodeId,
    pub t: Expr<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "fn", rename_all = "snake_case")]
pub enum FloatCall {
    NodeTransformX(Box<CallParamsNodeTransform>),
    NodeTransformY(Box<CallParamsNodeTransform>),
    #[serde(rename = "+")]
    Add(Box<FloatParamsPair>),
    #[serde(rename = "-")]
    Sub(Box<FloatParamsPair>),
    #[serde(rename = "*")]
    Mul(Box<FloatParamsPair>),
    #[serde(rename = "/")]
    Div(Box<FloatParamsPair>),
    Norm(Box<FloatParamsPair>),
    DefaultWidth {
        node: NodeId,
    },
    DefaultHeight {
        node: NodeId,
    },
    DefaultX {
        node: NodeId,
    },
    DefaultY {
        node: NodeId,
    },
    PathLength {
        node: NodeId,
    },
    PathX(Box<CallParamsPathPoint>),
    PathY(Box<CallParamsPathPoint>),
}

#[derive(Debug, Clone, Deserialize)]
pub enum NoCall {}

impl Eval<Option<Color>> for NoCall {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Option<Color>> {
        unreachable!()
    }
}

impl Eval<Arc<String>> for NoCall {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Arc<String>> {
        unreachable!()
    }
}

impl Eval<bool> for NoCall {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<bool> {
        unreachable!()
    }
}

impl Value for Option<Color> {
    type Call = NoCall;
    fn recursive_value() -> Self {
        None
    }
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        match (self, other) {
            (None, None) => None,
            (Some(a), Some(b)) => Some(Color::interpolate(a, b, t)),
            (None, Some(rc)) => {
                let mut lc = rc.clone();
                lc.set_alpha(0.0);
                Some(Color::interpolate(&lc, &rc, t))
            },
            (Some(lc), None) => {
                let mut rc = lc.clone();
                rc.set_alpha(0.0);
                Some(Color::interpolate(&lc, &rc, t))
            },
        }
    }
}

impl Value for Arc<String> {
    type Call = NoCall;
    fn recursive_value() -> Self {
        Arc::new(String::new())
    }

    fn interpolate(&self, other: &Self, t: f64) -> Self {
        self.clone()
    }
}

impl Value for bool {
    type Call = NoCall;
    fn recursive_value() -> Self {
        false
    }

    fn interpolate(&self, other: &Self, t: f64) -> Self {
        *self
    }
}
