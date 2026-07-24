"""Tests for rotation/scale/pivot on drawable nodes."""

import json
from pathlib import Path as FsPath

import pytest
from fairyflow import Ellipse, Group, Image, Path as FFPath, Rect, Scene, rel
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


def test_path_move_has_no_rotate(sc):
    p = FFPath()
    handle = p.move_to(5, 7)
    assert not hasattr(handle, "rotate")
    assert isinstance(handle, PathMove)


def test_scene_has_no_rotate(sc):
    assert not hasattr(sc, "rotate")


def test_path_has_no_rotate_scale_pivot(sc):
    # Path lost RotAndScaleMixin entirely (no box to rotate/scale/pivot
    # around) - put a Path inside a Group and transform the Group instead.
    p = FFPath()
    assert not hasattr(p, "rotate")
    assert not hasattr(p, "scale")
    assert not hasattr(p, "scale_x")
    assert not hasattr(p, "scale_y")
    assert not hasattr(p, "pivot")


def test_rect_rotate(test_scene):
    with test_scene:
        Rect().xy(10, 5).size(30, 20).fill("steelblue").rotate(45)


def test_ellipse_scale(test_scene):
    with test_scene:
        Ellipse().xy(10, 5).size(30, 20).fill("coral").scale_x(1.5).scale_y(0.5)


def test_rect_pivot(test_scene):
    """Rotate around a corner pivot (top_left) instead of the default center,
    via the real `pivot()` setter."""
    with test_scene:
        r = Rect().xy(20, 10).size(20, 20).fill("mediumpurple")
        r.pivot("top_left")
        r.rotate(30)


def test_rect_pivot_px(test_scene):
    """`pivot(x=, y=)` is an absolute-pixel pivot from the node's own
    top-left, distinct from the anchor spelling above - here an
    out-of-box point below-right of the shape."""
    with test_scene:
        r = Rect().xy(15, 10).size(20, 20).fill("mediumpurple")
        r.pivot(x=30, y=30)
        r.rotate(30)


def test_rect_pivot_rel(test_scene):
    """`pivot(x=rel(fx), y=rel(fy))` is the own-box-relative pivot spelling -
    a genuinely new code path (rel() previously raised here), replacing the
    pre-existing `node.pivot(node.at(fx, fy))` workaround. 20x20 rect, so
    rel(0.25)/rel(0.75) pivots at (5, 15) from its own top-left."""
    with test_scene:
        r = Rect().xy(20, 10).size(20, 20).fill("mediumpurple")
        r.pivot(x=rel(0.25), y=rel(0.75))
        r.rotate(30)


def test_pivot_position_orbit(test_scene):
    with test_scene:
        sun = Ellipse().xy(30, 15).size(10, 10).fill("gold")
        planet = Rect().xy(45, 18).size(4, 4).fill("steelblue")
        planet.pivot(sun.at("center"))
        planet.rotate(90)


def test_pivot_group_position(test_scene):
    """A Group pivoting around another node's Position exercises the
    transform-frame branch of `pivot()`: `Position.into_node(group)` already
    yields group-local coordinates (Group is a transform frame in the
    engine), unlike the leaf case above where the node's own x/y must be
    subtracted."""
    with test_scene:
        anchor = Rect().xy(23, 23).size(4, 4).fill("gold")
        with Group().xy(5, 5).size(20, 20) as g:
            Rect().size(8, 8).fill("steelblue")
        g.pivot(anchor.at("center"))
        g.rotate(30)


def test_image_rotate(test_scene):
    with test_scene:
        Image(ASSETS / "test.svg").xy(5, 2).size(50, 33).rotate(15)


def test_image_layer_position(test_scene):
    """An ORA/SVG layer nudged from its natural position, independently of
    its parent Image.

    `ImageLayer.rotate()`/`.scale()` are deliberately NOT applied by the
    renderer yet (unlike Rect/Ellipse/Image): a layer's `position` is a
    translate-only nudge on top of wherever its content already sits in the
    source composite, not a real local origin its content starts at - so
    rotating/scaling around a pivot computed from that nonexistent local box
    can swing the content arbitrarily far off screen. Deferred until layer
    content has a real bounding box. This test locks in the pre-existing,
    still-supported position nudge; `.rotate()` is exercised only for a
    parse/no-crash check.
    """
    with test_scene:
        img = Image(ASSETS / "test.svg").xy(5, 2).size(50, 33)
        img.layer("Ball").xy(-8, 4)
        img.layer("Box").rotate(45)  # no-op today; must not crash or move it


# ── pivot() argument validation ─────────────────────────────────────────────


def test_pivot_point_and_xy_raises(sc):
    r = Rect().size(20, 20)
    with pytest.raises(TypeError):
        r.pivot("center", x=5)


def test_pivot_bare_number_raises(sc):
    r = Rect().size(20, 20)
    with pytest.raises(TypeError):
        r.pivot(0.5, 0.5)


def test_pivot_rel_x_y_equivalent_to_center_anchor(sc):
    # rel() in x=/y= is own-box-relative (unlike every other setter's rel(),
    # which is parent-relative) - rel(0.5) on both axes is the same point as
    # the "center" anchor, and both now compile to a Call("*", fraction,
    # effective_width/height) on the wire.
    r1 = Rect().size(20, 20)
    r1.pivot("center")
    r2 = Rect().size(20, 20)
    r2.pivot(x=rel(0.5), y=rel(0.5))
    exported = create_export(0, sc)["nodes"]
    assert exported[0]["pivot_x"] == exported[1]["pivot_x"]
    assert exported[0]["pivot_y"] == exported[1]["pivot_y"]


def test_pivot_px_is_literal_not_division(sc):
    # pivot(x=30) should write the literal 30 directly to pivot_x - no more
    # Call("/", 30, width) wrapping, now that the wire field is absolute px.
    r = Rect().size(20, 20)
    r.pivot(x=30, y=30)
    node = create_export(0, sc)["nodes"][0]
    assert node["pivot_x"] == 30
    assert node["pivot_y"] == 30


def test_pivot_no_args_is_noop(sc):
    r = Rect().size(20, 20)
    before_x = r._get_attr("pivot_x")
    before_y = r._get_attr("pivot_y")
    r.pivot()
    assert r._get_attr("pivot_x") == before_x
    assert r._get_attr("pivot_y") == before_y


def test_pivot_position_exports_without_circular_reference(sc):
    # Mirrors test_position.py's Scene-sentinel regression tests: a
    # Position-based pivot embeds a map_x/map_y Call the same way at()/pos()/
    # next_to() do, so it must survive the same export round trip.
    r1 = Rect().size(10, 10).xy(10, 10)
    r2 = Rect().size(10, 10).xy(40, 40)
    r2.pivot(r1.at("center"))
    r2.rotate(45)
    exported = create_export(0, sc)
    json.dumps(exported)  # must not raise
