use std::collections::{BTreeMap, HashMap, HashSet};
use serde::{Deserialize, Deserializer};
use crate::basictypes::{AvId, FrameId, NodeId};
use crate::eval::EvalCtx;
use crate::defs::{Expr, Node, Transition, Value};

#[derive(Debug, Clone, Deserialize)]
pub struct KeyFrame {
    pub value: Expr,
    pub tr: Transition,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimatedValue {
    pub id: AvId,
    #[serde(flatten)]
    pub kind: AnimatedValueKind,
}

#[derive(Debug, Clone)]
pub enum FrameValue {
    KeyFrame(KeyFrame),
    Hold,
}

#[derive(Debug, Clone)]
pub struct ValuesInFrames(BTreeMap<FrameId, FrameValue>);

impl ValuesInFrames {

    pub fn collect_key_frames(&self, frames: &mut HashSet<FrameId>) {
        for frame in self.0.keys() {
            frames.insert(*frame);
        }
    }

    /// Look to smaller frames and find key frame (skips "hold")
    pub fn scan_left(&self, frame_id: FrameId) -> Option<(FrameId, &KeyFrame)> {
        let mut result_frame_id = None;
        for (frame_id, value) in self.0.range(..=frame_id).rev() {
            match value {
                FrameValue::KeyFrame(kf) => {
                    return Some((result_frame_id.unwrap_or(*frame_id), kf))
                },
                FrameValue::Hold => {
                    if result_frame_id.is_none() {
                        result_frame_id = Some(*frame_id);
                    }
                }
            }
        }
        None
    }

    /// Look to higher frames and return FrameValue (do not skip "hold")
    pub fn scan_right(&self, frame_id: FrameId) -> Option<(FrameId, &FrameValue)> {
        self.0.range(frame_id..).map(|(frame_id, value)| (*frame_id, value)).next()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn get(&self, frame_id: &FrameId) -> Option<&FrameValue> {
        self.0.get(frame_id)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnimatedValueKind {
    Const { value: Expr },
    Animated {
        #[serde(deserialize_with = "deserialize_keyframes")]
        values: ValuesInFrames,
    },
}

fn deserialize_keyframes<'de, D>(d: D) -> Result<ValuesInFrames, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    struct RawKeyFrame {
        frame: FrameId,
        #[serde(default)]
        value: Option<Expr>,
        #[serde(default)]
        tr: Option<Transition>,
        #[serde(default)]
        op: Option<String>,
    }

    let raw: Vec<RawKeyFrame> = Vec::deserialize(d)?;
    raw.into_iter()
        .map(|kf| {
            let fv = if kf.op.as_deref() == Some("hold") {
                FrameValue::Hold
            } else {
                let expr = kf.value.unwrap_or(Expr::Const(Value::None));
                let tr = kf.tr.ok_or_else(|| D::Error::custom("keyframe missing `tr`"))?;
                FrameValue::KeyFrame(KeyFrame { value: expr, tr })
            };
            Ok((kf.frame, fv))
        })
        .collect::<Result<BTreeMap<_, _>, D::Error>>().map(ValuesInFrames)
}


impl AnimatedValue {

    pub fn collect_key_frames(&self, frames: &mut HashSet<FrameId>) {
        match &self.kind {
            AnimatedValueKind::Const { value } => {}
            AnimatedValueKind::Animated { values } => {
                values.collect_key_frames(frames);
            }
        }
    }

    pub fn check_exprs<F>(&self, f: &mut F) -> anyhow::Result<()> where F: FnMut(&Expr) -> anyhow::Result<()> {
        match &self.kind {
            AnimatedValueKind::Const { .. } => {}
            AnimatedValueKind::Animated { values } => {
                for kf in values.0.values() {
                    match kf {
                        FrameValue::KeyFrame(kf) => {
                            f(&kf.value)?;
                        }
                        FrameValue::Hold => {}
                    }
                }
            }
        }
        Ok(())
    }

    pub fn eval(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        let _span = tracing::trace_span!("av.eval", av_id = %self.id).entered();
        ctx.begin_eval()?;
        let result = self.eval_inner(ctx);
        ctx.end_eval();
        result
    }

    pub fn eval_at_frame(&self, ctx: &EvalCtx, frame_id: FrameId) -> anyhow::Result<Value> {
        let new_ctx = ctx.clone_at_frame(frame_id);
        self.eval(&new_ctx)
    }

    fn eval_inner(&self, ctx: &EvalCtx) -> anyhow::Result<Value> {
        match &self.kind {
            AnimatedValueKind::Const { value: expr } => {
                tracing::trace!(av_id = %self.id, "const");
                Ok(expr.eval(ctx)?)
            }
            AnimatedValueKind::Animated { values } => {
                let frame = ctx.frame();
                let (left_f, left_fv) = values.scan_left(frame).unwrap();
                let left_v = left_fv.value.eval(ctx)?;
                if left_f == frame {
                    tracing::trace!(av_id = %self.id, frame = frame.as_u32(), "exact keyframe");
                    return Ok(left_v)
                };
                let Some((right_f, right_fv)) = values.scan_right(frame) else {
                    tracing::trace!(av_id = %self.id, frame = frame.as_u32(), left_f = left_f.as_u32(), "holding (no right keyframe)");
                    return Ok(left_v);
                };
                let right_v = match right_fv {
                    FrameValue::Hold | FrameValue::KeyFrame(KeyFrame { tr: Transition::Sharp, .. }) => {
                        tracing::trace!(av_id = %self.id, frame = frame.as_u32(), "sharp or hold, no interpolation");
                        return Ok(left_v);
                    }
                    FrameValue::KeyFrame(KeyFrame { value, .. }) => {
                        value.eval(ctx)?
                    }
                };
                let start_frame = left_f.as_u32();
                let end_frame = right_f.as_u32();
                let t = (frame.as_u32() - start_frame) as f64 / (end_frame - start_frame) as f64;
                tracing::trace!(av_id = %self.id, frame = frame.as_u32(), left_f = start_frame, right_f = end_frame, t, "interpolating");
                match (left_v, right_v) {
                    (Value::Color(lc), Value::Color(rc)) => {
                        Ok(Value::Color(crate::defs::Color::interpolate(&lc, &rc, t)))
                    }
                    (lv, rv) => {
                        let lf = lv.as_f64()?;
                        let rf = rv.as_f64()?;
                        Ok(Value::Float(lf + t * (rf - lf)))
                    }
                }
            }
        }
    }
}
