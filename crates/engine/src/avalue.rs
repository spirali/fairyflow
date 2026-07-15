use crate::basictypes::FrameId;
use crate::eval::EvalCtx;
use crate::values::{Eval, Expr, Value};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, de};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// Rename of v1's `"S"`/`"L"` — only `Linear` is accepted for now (the fuller
/// easing-preset/sampled-curve grammar is a later step, not part of this rewrite).
#[derive(Debug, Clone, PartialEq)]
pub enum Transition {
    Step,
    Linear,
}

#[derive(Debug)]
pub struct KeyFrame<T: Value + DeserializeOwned> {
    pub value: Expr<T>,
    pub tr: Transition,
}

#[derive(Debug)]
pub enum FrameValue<T: Value + DeserializeOwned> {
    KeyFrame(KeyFrame<T>),
    Hold,
}

#[derive(Debug)]
pub struct AnimatedValue<T: Value + DeserializeOwned> {
    values: BTreeMap<FrameId, FrameValue<T>>,
}

impl<T: Value + DeserializeOwned> AnimatedValue<T> {
    pub(crate) fn from_map(values: BTreeMap<FrameId, FrameValue<T>>) -> Self {
        AnimatedValue { values }
    }
}

/// One entry of the `"k"` array: `[frame]` (hold), `[frame, value]` (step), or
/// `[frame, value, ease]` (interpolate). Replaces v1's `RawKeyFrame` all-`Option`
/// workaround (needed there because a keyframe could be either a `{frame,value,tr}`
/// or `{frame,op:"hold"}` object) — tuple length now disambiguates directly.
pub(crate) struct KeyframeTuple<T: Value + DeserializeOwned> {
    pub frame: FrameId,
    pub value: FrameValue<T>,
}

impl<'de, T: Value + DeserializeOwned> Deserialize<'de> for KeyframeTuple<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct TupleVisitor<T>(PhantomData<T>);

        impl<'de, T: Value + DeserializeOwned> de::Visitor<'de> for TupleVisitor<T> {
            type Value = KeyframeTuple<T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    "a [frame], [frame, value], or [frame, value, ease] keyframe tuple"
                )
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let frame: FrameId = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let Some(value) = seq.next_element::<Expr<T>>()? else {
                    return Ok(KeyframeTuple {
                        frame,
                        value: FrameValue::Hold,
                    });
                };
                let ease: Option<String> = seq.next_element()?;
                let tr = match ease.as_deref() {
                    None => Transition::Step,
                    Some("linear") => Transition::Linear,
                    Some(other) => {
                        return Err(de::Error::custom(format!(
                            "unsupported ease `{}` (only \"linear\" is supported)",
                            other
                        )));
                    }
                };
                if seq.next_element::<de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom("keyframe tuple has too many elements"));
                }
                Ok(KeyframeTuple {
                    frame,
                    value: FrameValue::KeyFrame(KeyFrame { value, tr }),
                })
            }
        }

        d.deserialize_seq(TupleVisitor(PhantomData))
    }
}

impl AnimatedValue<Arc<String>> {
    pub fn collect_strings(&self, out: &mut HashSet<Arc<String>>) {
        for fv in self.values.values() {
            if let FrameValue::KeyFrame(kf) = fv {
                kf.value.collect_strings(out);
            }
        }
    }
}

impl<T: Value + DeserializeOwned + Clone> AnimatedValue<T> {
    pub fn eval<'a>(&'a self, ctx: &'a EvalCtx<'a>) -> anyhow::Result<T> {
        let _span = tracing::trace_span!("av.eval").entered();
        let frame = ctx.frame();
        let Some((left_f, left_fv)) = self.scan_left(frame) else {
            let (_key, value) = self.values.first_key_value().unwrap();
            match value {
                FrameValue::KeyFrame(kf) => return kf.value.eval(ctx),
                FrameValue::Hold => unreachable!(),
            }
        };
        let left_v = left_fv.value.eval(ctx)?;
        if left_f == frame {
            tracing::trace!(frame = frame.as_u32(), "exact keyframe");
            return Ok(left_v);
        };
        let Some((right_f, right_fv)) = self.scan_right(frame) else {
            tracing::trace!(
                frame = frame.as_u32(),
                left_f = left_f.as_u32(),
                "holding (no right keyframe)"
            );
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
        tracing::trace!(
            frame = frame.as_u32(),
            left_f = start_frame,
            right_f = end_frame,
            t,
            "interpolating"
        );
        Ok(left_v.interpolate(&right_v, t))
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
