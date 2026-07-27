"""Tests for the unified `Text.font()` setter, which merges the former
`font_size()`/`font_weight()`/`bold()`/`italic()` methods into one call."""

import pytest

from fairyflow import Scene
from fairyflow.serializer import create_export
from fairyflow.text import Text, stext


def _node(scene):
    return create_export(0, scene)["nodes"][0]


# ── Regression: old wire behavior via the new unified call ─────────────────


def test_font_family_and_size_together():
    s = Scene(100, 100)
    with s:
        Text("hi").font("monospace", 24)
    node = _node(s)
    assert node["font"] == "monospace"
    assert node["font_size"] == 24


def test_font_size_alone():
    s = Scene(100, 100)
    with s:
        Text("hi").font(size=18)
    node = _node(s)
    assert node["font_size"] == 18
    assert "font" not in node


def test_font_weight_direct():
    s = Scene(100, 100)
    with s:
        Text("hi").font(weight=700)
    node = _node(s)
    assert node["font_weight"] == 700


def test_font_italic():
    s = Scene(100, 100)
    with s:
        Text("hi").font(italic=True)
    node = _node(s)
    assert node["italic"] is True


# ── bold= / mono= sugar flags ────────────────────────────────────────────────


def test_bold_true_sets_weight_800():
    s = Scene(100, 100)
    with s:
        Text("hi").font(bold=True)
    node = _node(s)
    assert node["font_weight"] == 800


def test_bold_false_sets_weight_400():
    s = Scene(100, 100)
    with s:
        Text("hi").font(bold=False)
    node = _node(s)
    assert node["font_weight"] == 400


def test_mono_true_sets_monospace_family():
    s = Scene(100, 100)
    with s:
        Text("hi").font(mono=True)
    node = _node(s)
    assert node["font"] == "monospace"


def test_mono_false_sets_sans_serif_family():
    s = Scene(100, 100)
    with s:
        Text("hi").font(mono=False)
    node = _node(s)
    assert node["font"] == "sans-serif"


# ── Conflict guards ──────────────────────────────────────────────────────────


def test_bold_and_weight_conflict_raises():
    s = Scene(100, 100)
    with s, pytest.raises(TypeError):
        Text("hi").font(weight=500, bold=True)


def test_mono_and_family_conflict_raises():
    s = Scene(100, 100)
    with s, pytest.raises(TypeError):
        Text("hi").font("Arial", mono=True)


# ── dur=/ease= keyframe threading ────────────────────────────────────────────


def test_font_dur_threads_multiple_attrs_together():
    s = Scene(100, 100)
    with s:
        text = Text("hi")
        span = text._children[0]
        span.font(size=10, bold=True)
        span.font(size=20, bold=False, dur=1)
    size_av = span._attrs["font_size"]
    weight_av = span._attrs["font_weight"]
    size_frames = sorted(size_av.values)
    weight_frames = sorted(weight_av.values)
    assert len(size_frames) == 2
    assert len(weight_frames) == 2
    assert size_av.values[size_frames[0]] == 10
    assert size_av.values[size_frames[1]] == 20
    assert weight_av.values[weight_frames[0]] == 800
    assert weight_av.values[weight_frames[1]] == 400
    # Par()-grouped: both attrs animate over the same frame span
    assert size_frames == weight_frames


# ── stext() markup regression ────────────────────────────────────────────────


def test_stext_markup_bold_italic_font_size():
    s = Scene(100, 100)
    with s:
        t = stext("<span bold italic font-size='20'>text</span>")
    span = t._children[0]
    av_weight = span._attrs["font_weight"]
    av_italic = span._attrs["italic"]
    av_size = span._attrs["font_size"]
    assert av_weight.values[av_weight.init_frame] == 800
    assert av_italic.values[av_italic.init_frame] is True
    assert av_size.values[av_size.init_frame] == 20.0
