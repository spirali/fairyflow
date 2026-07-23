"""Tests for `Text.size()`/`expand()`/`keep_aspect()` — proposal §4.10's
sizing subsection: `Text` scales its laid-out block as a unit to fit an
explicit box, exactly like `Image`. `wrap()`/`text_align()` (§9.5) and
rotate/scale/pivot on `Text` are separate, unimplemented slices."""

from fairyflow import Image, Scene
from fairyflow.serializer import create_export
from fairyflow.text import Text
from pathlib import Path

ASSETS = Path(__file__).parent / "assets"


def _node(scene):
    return create_export(0, scene)["nodes"][0]


# ── Wire presence (non-golden) ──────────────────────────────────────────────


def test_size_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "w" not in node
    assert "h" not in node
    # keep_aspect defaults `True` but, like `Image`, is seeded eagerly at
    # construction (`_add_attr`) rather than lazily, so it's always present.
    assert node["keep_aspect"] is True


def test_size_both_axes_writes_w_and_h():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30)
    node = _node(s)
    assert node["w"] == 80
    assert node["h"] == 30


def test_size_single_axis_leaves_other_absent():
    s = Scene(100, 100)
    with s:
        Text("hi").size(w=80)
    node = _node(s)
    assert node["w"] == 80
    assert "h" not in node


def test_expand_is_rel_1_on_both_axes():
    s = Scene(100, 100)
    with s:
        t1 = Text("hi").expand()
    node = _node(s)
    assert node["w"] == node["h"]  # both are `mul(parent.dim, 1)` expressions
    assert t1._has_attr("width") and t1._has_attr("height")


def test_keep_aspect_writes_explicit_value():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30).keep_aspect(False)
    node = _node(s)
    assert node["keep_aspect"] is False


def test_keep_aspect_defaults_true_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30)
    node = _node(s)
    assert node["keep_aspect"] is True


def test_image_keep_aspect_setter_matches_ctor_kwarg():
    # Image's `keep_aspect` was constructor-only before this slice; confirm
    # the new shared setter can override it at runtime too.
    s = Scene(100, 100)
    with s:
        Image(ASSETS / "test.svg", keep_aspect=True).size(50, 40).keep_aspect(False)
    node = _node(s)
    assert node["keep_aspect"] is False


# ── Golden renders: width-only / height-only / both+keep_aspect / expand() ──


def test_text_size_width_only(test_scene):
    with test_scene:
        Text("Hi").font(size=16).size(w=50)


def test_text_size_height_only(test_scene):
    with test_scene:
        Text("Hi").font(size=16).size(h=30)


def test_text_size_both_keep_aspect_letterboxed(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).size(50, 30)


def test_text_size_both_keep_aspect_false_stretched(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).size(50, 30).keep_aspect(False)


def test_text_expand_poster_text(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).expand()
