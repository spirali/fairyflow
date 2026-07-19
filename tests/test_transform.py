"""Tests for rotation/scale/pivot on drawable nodes.

Step 1 (wire fields + engine position math) is covered by the "*_serializes"
tests below (`sc` fixture: serialization only, no rendering). Step 2 (the
renderer actually painting rotation/scale/pivot) is covered by the golden-image
tests (`test_scene` fixture). See api-v2-impl.md item 2 for the full story.
"""

from pathlib import Path as FsPath

import pytest
from fairyflow import Ellipse, Image, Path as FFPath, Rect, Scene
from fairyflow.nodes import PathMove
from fairyflow.serializer import create_export
from fairyflow.text import TextSpan

ASSETS = FsPath(__file__).parent / "assets"


@pytest.fixture
def sc():
    s = Scene(100, 100)
    with s:
        yield s


def test_rect_rotate_serializes(sc):
    Rect().size(10, 10).rotate(45)
    node = create_export(0, sc)["nodes"][0]
    assert node["rotation"] == 45


def test_ellipse_scale_serializes(sc):
    Ellipse().size(10, 10).scale(2)
    node = create_export(0, sc)["nodes"][0]
    assert node["scale_x"] == 2
    assert node["scale_y"] == 2


def test_path_rotate_serializes(sc):
    # Path has no PositionMixin/SizeMixin - rotate() must still work via the
    # bare RotAndScaleMixin, with no x/y/w/h prerequisites.
    FFPath().rotate(30)
    node = create_export(0, sc)["nodes"][0]
    assert node["rotation"] == 30


def test_image_scale_x_serializes(sc):
    Image(ASSETS / "test.svg").size(50, 40).scale_x(1.5)
    node = create_export(0, sc)["nodes"][0]
    assert node["scale_x"] == 1.5


def test_image_layer_rotate_serializes(sc):
    img = Image(ASSETS / "test.svg").size(50, 40)
    layer = img.layer("Ball")
    layer.rotate(20)
    exported = create_export(0, sc)
    layer_node = next(n for n in exported["nodes"] if n.get("layer_name") == "Ball")
    assert layer_node["rotation"] == 20


def test_text_span_has_no_rotate(sc):
    from fairyflow import Text

    span = Text().span("hi")
    assert not hasattr(span, "rotate")
    assert isinstance(span, TextSpan)


def test_path_move_has_no_rotate(sc):
    p = FFPath()
    handle = p.move_to().xy(5, 7)
    assert not hasattr(handle, "rotate")
    assert isinstance(handle, PathMove)


def test_scene_has_no_rotate(sc):
    assert not hasattr(sc, "rotate")


def test_rect_rotate(test_scene):
    with test_scene:
        Rect().xy(10, 5).size(30, 20).color("steelblue").rotate(45)


def test_ellipse_scale(test_scene):
    with test_scene:
        Ellipse().xy(10, 5).size(30, 20).color("coral").scale_x(1.5).scale_y(0.5)


def test_rect_pivot(test_scene):
    """Rotate around a corner pivot (0, 0) instead of the default center.

    No `pivot()` setter exists yet in Python (a pre-existing gap, unrelated to
    this step - pivot is already settable via the wire format); pivot_x/
    pivot_y are set directly via the internal `_set_attr` to exercise it.
    """
    with test_scene:
        r = Rect().xy(20, 10).size(20, 20).color("mediumpurple")
        r._set_attr("pivot_x", 0.0)
        r._set_attr("pivot_y", 0.0)
        r.rotate(30)


def test_image_rotate(test_scene):
    with test_scene:
        Image(ASSETS / "test.svg").xy(5, 2).size(50, 33).rotate(15)


def test_image_layer_position(test_scene):
    """An ORA/SVG layer nudged from its natural position, independently of
    its parent Image.

    `ImageLayer.rotate()`/`.scale()` are deliberately NOT applied by the
    renderer yet (unlike Rect/Ellipse/Image/Path): a layer's `position` is a
    translate-only nudge on top of wherever its content already sits in the
    source composite, not a real local origin its content starts at - so
    rotating/scaling around a pivot computed from that nonexistent local box
    can swing the content arbitrarily far off screen. Deferred until layer
    content has a real bounding box (same category of gap as Path's inert
    pivot). This test locks in the pre-existing, still-supported position
    nudge; `.rotate()` is exercised only for a parse/no-crash check.
    """
    with test_scene:
        img = Image(ASSETS / "test.svg").xy(5, 2).size(50, 33)
        img.layer("Ball").xy(-8, 4)
        img.layer("Box").rotate(45)  # no-op today; must not crash or move it


def test_path_rotate(test_scene):
    """Path has no bounds computation, so rotation happens around the local
    origin (0, 0), not the shape's visual center - a documented, pre-existing
    limitation (pivot_x/pivot_y are inert for Path until bounds land). Points
    are drawn above/left of the origin (negative y) so that after a 90°
    rotation around (0, 0) the triangle lands back inside the scene.
    """
    with test_scene:
        p = FFPath()
        p.color("tomato")
        p.stroke_color("black")
        p.move_to().xy(5, -20)
        p.line_to().xy(20, -20)
        p.line_to().xy(20, -5)
        p.close()
        p.rotate(90)
