use crate::avalue::{AnimatedValue, KeyframeTuple};
use crate::basictypes::NodeId;
use crate::eval::EvalCtx;
use renderer_core::Color as RendererColor;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, de};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Color(RendererColor);

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Ok(Color(RendererColor::from_rgba8(0, 0, 0, 0)));
        }
        RendererColor::from_html(&s)
            .map(Color)
            .ok_or_else(|| de::Error::custom(format!("invalid color '{}'", s)))
    }
}

impl Color {
    pub fn into_inner(self) -> RendererColor {
        self.0
    }

    pub fn interpolate(a: &Color, b: &Color, t: f64) -> Color {
        Color(a.0.interpolate(&b.0, t))
    }
}

pub trait Eval<T> {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<T>;
}

/// Hand-rolled arity-checked positional-argument parsing for a `Value`'s call type.
/// Replaces v1's `#[serde(tag = "fn")]` internal tagging (which buffers the whole
/// object to peek the tag) with ordinary sequential `SeqAccess` reads.
pub trait CallParse: Sized {
    fn parse_seq<'de, A: de::SeqAccess<'de>>(op: &str, seq: A) -> Result<Self, A::Error>;
}

pub trait Value: Sized {
    type Call: Eval<Self> + CallParse + std::fmt::Debug;
    fn recursive_value() -> Self;
    fn interpolate(&self, other: &Self, t: f64) -> Self;
}

#[derive(Debug)]
pub enum Expr<T: Value + DeserializeOwned> {
    Const(T),
    Call(T::Call),
    AnimValue(AnimatedValue<T>),
}

impl<T: Clone + Value + DeserializeOwned> Eval<T> for Expr<T> {
    fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<T> {
        match self {
            Expr::Const(v) => Ok(v.clone()),
            Expr::Call(call) => call.eval(ctx),
            Expr::AnimValue(av) => av.eval(ctx),
        }
    }
}

/// The v2 value grammar (`api-v2-impl.md` §A.2-A.4): a constant, an `["op", ...]`
/// s-expr array, or a `{"k": [...]}` keyframes object. One hand-written visitor
/// dispatches on the JSON shape directly (`deserialize_any`) instead of the
/// buffered/retried `#[serde(untagged)]` trial-parsing v1 used.
impl<'de, T: Value + DeserializeOwned> Deserialize<'de> for Expr<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ExprVisitor<T>(PhantomData<T>);

        impl<'de, T: Value + DeserializeOwned> de::Visitor<'de> for ExprVisitor<T> {
            type Value = Expr<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    "a constant, an [\"op\", ...] expression, or a {{\"k\": [...]}} keyframes object"
                )
            }

            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                T::deserialize(de::value::BoolDeserializer::new(v)).map(Expr::Const)
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                T::deserialize(de::value::I64Deserializer::new(v)).map(Expr::Const)
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::deserialize(de::value::U64Deserializer::new(v)).map(Expr::Const)
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                T::deserialize(de::value::F64Deserializer::new(v)).map(Expr::Const)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                T::deserialize(de::value::StrDeserializer::new(v)).map(Expr::Const)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let op: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &"an [\"op\", ...] expression"))?;
                let call = T::Call::parse_seq(&op, seq)?;
                Ok(Expr::Call(call))
            }

            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let key: String = map
                    .next_key()?
                    .ok_or_else(|| de::Error::custom("expected a single \"k\" key"))?;
                if key != "k" {
                    return Err(de::Error::unknown_field(&key, &["k"]));
                }
                let tuples: Vec<KeyframeTuple<T>> = map.next_value()?;
                if map.next_key::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom(
                        "unexpected extra key in keyframes object",
                    ));
                }
                let mut values = BTreeMap::new();
                for t in tuples {
                    values.insert(t.frame, t.value);
                }
                Ok(Expr::AnimValue(AnimatedValue::from_map(values)))
            }
        }

        d.deserialize_any(ExprVisitor(PhantomData))
    }
}

impl Expr<Arc<String>> {
    pub fn collect_strings(&self, out: &mut HashSet<Arc<String>>) {
        match self {
            Expr::Const(s) => {
                out.insert(s.clone());
            }
            Expr::Call(c) => match *c {},
            Expr::AnimValue(av) => av.collect_strings(out),
        }
    }
}

impl Expr<f64> {
    pub fn is_auto_width_of(&self, node: NodeId) -> bool {
        match self {
            Expr::Call(FloatCall::AutoWidth { node: n }) => node == *n,
            _ => false,
        }
    }
    pub fn is_auto_height_of(&self, node: NodeId) -> bool {
        match self {
            Expr::Call(FloatCall::AutoHeight { node: n }) => node == *n,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct FloatParamsPair {
    pub a: Expr<f64>,
    pub b: Expr<f64>,
}

#[derive(Debug)]
pub struct CallParamsMap {
    pub source: NodeId,
    pub target: NodeId,
    pub x: Expr<f64>,
    pub y: Expr<f64>,
}

#[derive(Debug)]
pub struct CallParamsPathPoint {
    pub node: NodeId,
    pub t: Expr<f64>,
}

#[derive(Debug)]
pub enum FloatCall {
    MapX(Box<CallParamsMap>),
    MapY(Box<CallParamsMap>),
    Add(Box<FloatParamsPair>),
    Sub(Box<FloatParamsPair>),
    Mul(Box<FloatParamsPair>),
    Div(Box<FloatParamsPair>),
    Norm(Box<FloatParamsPair>),
    Max(Box<FloatParamsPair>),
    AutoWidth { node: NodeId },
    AutoHeight { node: NodeId },
    AutoX { node: NodeId },
    AutoY { node: NodeId },
    PathLength { node: NodeId },
    PathX(Box<CallParamsPathPoint>),
    PathY(Box<CallParamsPathPoint>),
}

impl CallParse for FloatCall {
    fn parse_seq<'de, A: de::SeqAccess<'de>>(op: &str, mut seq: A) -> Result<Self, A::Error> {
        macro_rules! next {
            () => {
                seq.next_element()?.ok_or_else(|| {
                    de::Error::custom(format!("not enough arguments for op `{}`", op))
                })?
            };
        }
        let result = match op {
            "+" => FloatCall::Add(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "-" => FloatCall::Sub(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "*" => FloatCall::Mul(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "/" => FloatCall::Div(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "norm" => FloatCall::Norm(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "max" => FloatCall::Max(Box::new(FloatParamsPair {
                a: next!(),
                b: next!(),
            })),
            "map_x" => FloatCall::MapX(Box::new(CallParamsMap {
                source: next!(),
                target: next!(),
                x: next!(),
                y: next!(),
            })),
            "map_y" => FloatCall::MapY(Box::new(CallParamsMap {
                source: next!(),
                target: next!(),
                x: next!(),
                y: next!(),
            })),
            "auto_x" => FloatCall::AutoX { node: next!() },
            "auto_y" => FloatCall::AutoY { node: next!() },
            "auto_w" => FloatCall::AutoWidth { node: next!() },
            "auto_h" => FloatCall::AutoHeight { node: next!() },
            "path_len" => FloatCall::PathLength { node: next!() },
            "path_x" => FloatCall::PathX(Box::new(CallParamsPathPoint {
                node: next!(),
                t: next!(),
            })),
            "path_y" => FloatCall::PathY(Box::new(CallParamsPathPoint {
                node: next!(),
                t: next!(),
            })),
            other => return Err(de::Error::custom(format!("unknown op `{}`", other))),
        };
        if seq.next_element::<de::IgnoredAny>()?.is_some() {
            return Err(de::Error::custom(format!(
                "too many arguments for op `{}`",
                op
            )));
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub enum NoCall {}

impl CallParse for NoCall {
    fn parse_seq<'de, A: de::SeqAccess<'de>>(op: &str, _seq: A) -> Result<Self, A::Error> {
        Err(de::Error::custom(format!(
            "op `{}` is not valid for this attribute type",
            op
        )))
    }
}

impl Eval<Color> for NoCall {
    fn eval(&self, _ctx: &EvalCtx) -> anyhow::Result<Color> {
        unreachable!()
    }
}

impl Eval<Arc<String>> for NoCall {
    fn eval(&self, _ctx: &EvalCtx) -> anyhow::Result<Arc<String>> {
        unreachable!()
    }
}

impl Eval<bool> for NoCall {
    fn eval(&self, _ctx: &EvalCtx) -> anyhow::Result<bool> {
        unreachable!()
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

impl Value for Color {
    type Call = NoCall;
    fn recursive_value() -> Self {
        Color(RendererColor::from_rgba8(0, 0, 0, 0))
    }
    fn interpolate(&self, other: &Self, t: f64) -> Self {
        Color::interpolate(self, other, t)
    }
}

impl Value for Arc<String> {
    type Call = NoCall;
    fn recursive_value() -> Self {
        Arc::new(String::new())
    }

    fn interpolate(&self, _other: &Self, _t: f64) -> Self {
        self.clone()
    }
}

impl Value for bool {
    type Call = NoCall;
    fn recursive_value() -> Self {
        false
    }

    fn interpolate(&self, _other: &Self, _t: f64) -> Self {
        *self
    }
}
