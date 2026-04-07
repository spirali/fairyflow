use crate::basictypes::{AvId, FrameId, NodeId};
use crate::nodes::{Node, TopLevelExpr, Transition};
use crate::eval::EvalCtx;
use anyhow::bail;
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, HashMap, HashSet};
use serde::de::DeserializeOwned;
use crate::values::{Expr, Value, Eval};

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct KeyFrame<T: Value + DeserializeOwned> {
    pub value: Expr<T>,
    pub tr: Transition,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub struct AnimatedValue<T: Value + DeserializeOwned> {
    #[serde(deserialize_with = "deserialize_keyframes")]
    values: BTreeMap<FrameId, FrameValue<T>>,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: DeserializeOwned"))]
pub enum FrameValue<T: Value + DeserializeOwned> {
    KeyFrame(KeyFrame<T>),
    Hold,
}

fn deserialize_keyframes<'de, D, T: Value + DeserializeOwned>(d: D) -> Result<BTreeMap<FrameId, FrameValue<T>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(bound(deserialize = "T: DeserializeOwned"))]
    struct RawKeyFrame<T: Value + DeserializeOwned> {
        frame: FrameId,
        #[serde(default)]
        value: Option<Expr<T>>,
        #[serde(default)]
        tr: Option<Transition>,
        #[serde(default)]
        op: Option<String>,
    }

    let raw: Vec<RawKeyFrame<T>> = Vec::deserialize(d)?;
    raw.into_iter()
        .map(|kf| {
            let fv = if kf.op.as_deref() == Some("hold") {
                FrameValue::Hold
            } else {
                let expr = kf
                    .value
                    .ok_or_else(|| D::Error::custom("keyframe missing `value`"))?;
                let tr = kf
                    .tr
                    .ok_or_else(|| D::Error::custom("keyframe missing `tr`"))?;
                FrameValue::KeyFrame(KeyFrame { value: expr, tr })
            };
            Ok((kf.frame, fv))
        })
        .collect::<Result<BTreeMap<_, _>, D::Error>>()
}

impl<T: Value + DeserializeOwned + Clone> AnimatedValue<T> {
    pub fn eval<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<T> {
        let _span = tracing::trace_span!("av.eval").entered();
        let frame = ctx.frame();
        let Some((left_f, left_fv)) = self.scan_left(frame) else {
            dbg!(&frame);
            panic!();
        };
        let left_v = left_fv.value.eval(ctx)?;
        if left_f == frame {
            tracing::trace!(frame = frame.as_u32(), "exact keyframe");
            return Ok(left_v);
        };
        let Some((right_f, right_fv)) = self.scan_right(frame) else {
            tracing::trace!(frame = frame.as_u32(), left_f = left_f.as_u32(), "holding (no right keyframe)");
            return Ok(left_v);
        };
        let right_v = match right_fv {
            FrameValue::Hold
            | FrameValue::KeyFrame(KeyFrame {
                tr: Transition::Step,
                ..
            }) => {
                tracing::trace!(frame = frame.as_u32(), "sharp or hold, no interpolation");
                return Ok(left_v);
            }
            FrameValue::KeyFrame(KeyFrame { value, .. }) => value.eval(ctx)?,
        };
        let start_frame = left_f.as_u32();
        let end_frame = right_f.as_u32();
        let t = (frame.as_u32() - start_frame) as f64 / (end_frame - start_frame) as f64;
        tracing::trace!(frame = frame.as_u32(), left_f = start_frame, right_f = end_frame, t, "interpolating");
        Ok(left_v.interpolate(&right_v, t))
    }

    pub fn collect_key_frames(&self, frames: &mut HashSet<FrameId>) {
        for frame in self.values.keys() {
            frames.insert(*frame);
        }
    }

    /// Look to smaller frames and find key frame (skips "hold")
    pub fn scan_left(&self, frame_id: FrameId) -> Option<(FrameId, &KeyFrame<T>)> {
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
    pub fn scan_right(&self, frame_id: FrameId) -> Option<(FrameId, &FrameValue<T>)> {
        self.values
            .range(frame_id..)
            .map(|(frame_id, value)| (*frame_id, value))
            .next()
    }
}
