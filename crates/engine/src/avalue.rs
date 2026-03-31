use crate::basictypes::{AvId, FrameId, NodeId};
use crate::nodes::{Expr, Node, TopLevelExpr, Transition, Value};
use crate::eval::EvalCtx;
use anyhow::bail;
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
pub struct KeyFrame {
    pub value: TopLevelExpr,
    pub tr: Transition,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnimatedValue {
    pub id: AvId,
    #[serde(deserialize_with = "deserialize_keyframes")]
    values: BTreeMap<FrameId, FrameValue>,
}

#[derive(Debug, Clone)]
pub enum FrameValue {
    KeyFrame(KeyFrame),
    Hold,
}

fn deserialize_keyframes<'de, D>(d: D) -> Result<BTreeMap<FrameId, FrameValue>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    struct RawKeyFrame {
        frame: FrameId,
        #[serde(default)]
        value: Option<TopLevelExpr>,
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
                let expr = kf
                    .value
                    .unwrap_or(TopLevelExpr::new(Expr::Const(Value::None)));
                let tr = kf
                    .tr
                    .ok_or_else(|| D::Error::custom("keyframe missing `tr`"))?;
                FrameValue::KeyFrame(KeyFrame { value: expr, tr })
            };
            Ok((kf.frame, fv))
        })
        .collect::<Result<BTreeMap<_, _>, D::Error>>()
}

impl AnimatedValue {
    pub fn eval<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<Value> {
        let _span = tracing::trace_span!("av.eval", av_id = %self.id).entered();
        let frame = ctx.frame();
        let Some((left_f, left_fv)) = self.scan_left(frame) else {
            dbg!(&frame);
            dbg!(&self.values);
            panic!();
        };
        let left_v = left_fv.value.eval(ctx)?;
        if left_f == frame {
            tracing::trace!(av_id = %self.id, frame = frame.as_u32(), "exact keyframe");
            return Ok(left_v);
        };
        let Some((right_f, right_fv)) = self.scan_right(frame) else {
            tracing::trace!(av_id = %self.id, frame = frame.as_u32(), left_f = left_f.as_u32(), "holding (no right keyframe)");
            return Ok(left_v);
        };
        let right_v = match right_fv {
            FrameValue::Hold
            | FrameValue::KeyFrame(KeyFrame {
                tr: Transition::Step,
                ..
            }) => {
                tracing::trace!(av_id = %self.id, frame = frame.as_u32(), "sharp or hold, no interpolation");
                return Ok(left_v);
            }
            FrameValue::KeyFrame(KeyFrame { value, .. }) => value.eval(ctx)?,
        };
        let start_frame = left_f.as_u32();
        let end_frame = right_f.as_u32();
        let t = (frame.as_u32() - start_frame) as f64 / (end_frame - start_frame) as f64;
        tracing::trace!(av_id = %self.id, frame = frame.as_u32(), left_f = start_frame, right_f = end_frame, t, "interpolating");
        match (left_v, right_v) {
            (Value::Color(lc), Value::Color(rc)) => {
                Ok(Value::Color(crate::nodes::Color::interpolate(&lc, &rc, t)))
            }
            (Value::None, Value::Color(rc)) => {
                let mut lc = rc.clone();
                lc.set_alpha(0.0);
                Ok(Value::Color(crate::nodes::Color::interpolate(&lc, &rc, t)))
            }
            (Value::Color(lc), Value::None) => {
                let mut rc = lc.clone();
                rc.set_alpha(0.0);
                Ok(Value::Color(crate::nodes::Color::interpolate(&lc, &rc, t)))
            }
            (lv, rv) if lv.is_number() && rv.is_number() => {
                let lf = lv.as_f64()?;
                let rf = rv.as_f64()?;
                Ok(Value::Float(lf + t * (rf - lf)))
            }
            (lv, rc) => {
                bail!("Invalid interpolation of {lv:?} and {rc:?}");
            }
        }
    }

    pub fn collect_key_frames(&self, frames: &mut HashSet<FrameId>) {
        for frame in self.values.keys() {
            frames.insert(*frame);
        }
    }

    /// Look to smaller frames and find key frame (skips "hold")
    pub fn scan_left(&self, frame_id: FrameId) -> Option<(FrameId, &KeyFrame)> {
        let mut result_frame_id = None;
        for (frame_id, value) in self.values.range(..=frame_id).rev() {
            match value {
                FrameValue::KeyFrame(kf) => {
                    return Some((result_frame_id.unwrap_or(*frame_id), kf));
                }
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
        self.values
            .range(frame_id..)
            .map(|(frame_id, value)| (*frame_id, value))
            .next()
    }
}
