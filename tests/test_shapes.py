"""Tests for the `Line`/`Arrow` connector sugar and the `Polygon` family."""

import pytest

from fairyflow import Arrow, Line, Polygon, Rect, RegularPolygon, Scene, Star
from fairyflow.serializer import create_export


def _attr_value(node, name):
    """Read the raw value stored for `name` (bypassing the `AnimatedValue`
    wrapper `_get_attr` returns), same helper as `test_position.py`."""
    av = node._attrs[name]
    return av.values[av.init_frame]


# ── Line / Arrow wire-presence tests (no rendering) ─────────────────────────


def test_line_has_move_and_line_children():
    s = Scene(100, 100)
    with s:
        line = Line((0, 0), (50, 50))
    assert len(line._children) == 2
    node = create_export(0, s)["nodes"][0]
    assert node["children"] == [1, 2]


def test_line_start_end_are_the_move_and_line_handles():
    s = Scene(100, 100)
    with s:
        line = Line((0, 0), (50, 50))
    assert line.start is line._children[0]
    assert line.end is line._children[1]


def test_arrow_default_head_end_has_one_arrowhead():
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (50, 50))
    assert len(arrow.arrowheads) == 1


def test_arrow_head_both_has_two_arrowheads():
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (50, 50), head="both")
    assert len(arrow.arrowheads) == 2


def test_arrow_head_start_has_one_arrowhead():
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (50, 50), head="start")
    assert len(arrow.arrowheads) == 1


def test_arrow_shaft_has_only_move_and_line_children():
    """The arrowhead is a separate sibling node, same as the pre-existing
    `arrow()`/`triangle_arrow` behavior — the shaft itself stays 2 commands."""
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (50, 50), head="both")
    assert len(arrow._children) == 2


def test_arrow_alpha_propagates_to_arrowheads():
    """The arrowhead is a separate sibling node (see
    `test_arrow_shaft_has_only_move_and_line_children`), so `.alpha()` on the
    `Arrow` must be explicitly forwarded to it - same as `stroke_color`/
    `stroke_width` already are in `Path.arrow()`. The forwarded value is a
    live reference to the shaft's own `AnimatedValue`, not a snapshot, so a
    later `.alpha()` call on the shaft is picked up by the head too."""
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (50, 50), head="both").alpha(0.3)
    assert _attr_value(arrow, "alpha") == 0.3
    for head in arrow.arrowheads:
        assert _attr_value(head, "alpha") is arrow._attrs["alpha"]


def test_arrow_gap_shifts_end_toward_start():
    """Both endpoints are plain literals (no live `Position`), so the whole
    gap/direction expression is constant-foldable — the exported wire
    carries a single plain number, not a nested expression tree (see
    `serializer._try_fold`). Unit vector from (30,40) to (0,0) is
    (-0.6,-0.8): end shifted 5px toward start -> (30-3, 40-4) = (27, 36)."""
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (30, 40), gap=5)
    node = create_export(0, s)["nodes"][2]  # the "end" line command
    assert node["kind"] == "line"
    assert node["x"] == 27.0
    assert node["y"] == 36.0
    # tail end (no arrowhead) stays exactly at its given point
    assert _attr_value(arrow.start, "x").value == 0
    assert _attr_value(arrow.start, "y").value == 0


def test_arrow_gap_zero_touches_the_raw_point():
    s = Scene(100, 100)
    with s:
        arrow = Arrow((0, 0), (30, 40))
    assert _attr_value(arrow.end, "x").value == 30
    assert _attr_value(arrow.end, "y").value == 40


# ── Polygon family wire-presence tests ──────────────────────────────────────


def test_polygon_too_few_points_raises():
    s = Scene(100, 100)
    with s, pytest.raises(ValueError):
        Polygon([(0, 0)])


def test_polygon_builds_move_line_close_sequence():
    s = Scene(100, 100)
    with s:
        poly = Polygon([(0, 0), (80, 20), (40, 90)])
    kinds = [c.kind for c in poly._children]
    assert kinds == ["move", "line", "line", "close"]
    assert _attr_value(poly._children[0], "x") == 0
    assert _attr_value(poly._children[0], "y") == 0
    assert _attr_value(poly._children[1], "x") == 80
    assert _attr_value(poly._children[1], "y") == 20
    assert _attr_value(poly._children[2], "x") == 40
    assert _attr_value(poly._children[2], "y") == 90


def test_regular_polygon_too_few_sides_raises():
    s = Scene(100, 100)
    with s, pytest.raises(ValueError):
        RegularPolygon(2, radius=10)


def test_regular_polygon_vertex_count():
    s = Scene(100, 100)
    with s:
        hexagon = RegularPolygon(6, radius=50)
    # 6 move/line-ish vertices + 1 move + 5 line + 1 close = 7 children
    assert len(hexagon._children) == 7


def test_regular_polygon_first_vertex_points_up():
    s = Scene(100, 100)
    with s:
        rp = RegularPolygon(4, radius=10)
    first = rp._children[0]
    assert _attr_value(first, "x") == pytest.approx(0, abs=1e-9)
    assert _attr_value(first, "y") == pytest.approx(-10)


def test_star_too_few_points_raises():
    s = Scene(100, 100)
    with s, pytest.raises(ValueError):
        Star(points=1, outer=50, inner=20)


def test_star_vertex_count():
    s = Scene(100, 100)
    with s:
        star = Star(points=5, outer=50, inner=20)
    # 10 vertices (5 outer + 5 inner) + move + 9 line + close = 11 children
    assert len(star._children) == 11


def test_star_first_vertex_is_outer_and_points_up():
    s = Scene(100, 100)
    with s:
        star = Star(points=5, outer=50, inner=20)
    first = star._children[0]
    assert _attr_value(first, "x") == pytest.approx(0, abs=1e-9)
    assert _attr_value(first, "y") == pytest.approx(-50)


def test_regular_polygon_center_offsets_vertices():
    s = Scene(100, 100)
    with s:
        rp = RegularPolygon(4, radius=10, center=(20, 30))
    first = rp._children[0]
    assert _attr_value(first, "x") == pytest.approx(20, abs=1e-9)
    assert _attr_value(first, "y") == pytest.approx(20)  # 30 - 10


# ── Golden image tests ───────────────────────────────────────────────────────


def test_line_and_arrow_styles(test_scene):
    """One row per arrowhead style, plus a bare Line for comparison."""
    test_scene.pdf_tolerance = 120  # six stroked/filled shapes in one scene
    with test_scene.size(320, 220):
        Line((20, 20), (300, 20)).stroke("black", 3)
        for i, style in enumerate(["triangle", "open", "stealth", "bar", "dot"]):
            y = 60 + i * 30
            Arrow((20, y), (300, y), style=style).stroke("steelblue", 3)


def test_arrow_head_both(test_scene):
    with test_scene.size(200, 60):
        Arrow((20, 30), (180, 30), head="both").stroke("darkorange", 3)


def test_arrow_alpha_dims_arrowhead(test_scene):
    """Both the shaft and the arrowhead must dim together under `.alpha()` -
    regression test for the arrowhead staying fully opaque while the shaft
    faded (see `test_arrow_alpha_propagates_to_arrowheads` for the unit-level
    check)."""
    with test_scene.size(200, 60):
        Arrow((20, 30), (180, 30)).stroke("darkorange", 3).alpha(0.3)


def test_arrow_gap_leaves_space_before_target(test_scene):
    with test_scene.size(200, 100):
        target = Rect().xy(150, 30).size(40, 40).fill("steelblue")
        Arrow((10, 50), target.at("left"), gap=6).stroke("black", 3)


def test_polygon_irregular_triangle(test_scene):
    with test_scene.size(160, 120):
        Polygon([(20, 100), (140, 60), (60, 10)]).fill("steelblue")


def test_regular_polygon_hexagon(test_scene):
    with test_scene.size(160, 160):
        RegularPolygon(6, radius=60, center=(80, 80)).fill("mediumseagreen")


def test_star_five_point(test_scene):
    with test_scene.size(160, 160):
        Star(points=5, outer=70, inner=28, center=(80, 80)).fill("gold")
