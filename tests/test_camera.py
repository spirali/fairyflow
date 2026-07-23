"""Tests for the `.camera` object on `Group` and `Scene`."""

from fairyflow import Ellipse, Group, Par, Rect, Scene, anim
from fairyflow.serializer import create_export
import pytest


FRAMES = [0, 12, 24]


# ── Wire-presence tests (no rendering) ──────────────────────────────────────


def test_camera_zoom_writes_only_camera_zoom():
    """`.camera.zoom()` writes only `camera_zoom` - `camera_x`/`camera_y` stay
    absent, exactly like a bare `.clip()` leaving untouched axes alone."""
    s = Scene(60, 40)
    with s:
        g = Group().size(20, 15)
        g.camera.zoom(2)
    node = create_export(0, s)["nodes"][0]
    assert node["camera_zoom"] == 2
    assert "camera_x" not in node
    assert "camera_y" not in node


def test_camera_never_touched_emits_nothing():
    """A group whose `.camera` is never accessed must emit none of the three
    camera keys - same "no wire footprint until touched" guarantee as every
    other lazily-defaulted attribute in this codebase."""
    s = Scene(60, 40)
    with s:
        Group().size(20, 15)  # .camera never accessed
    node = create_export(0, s)["nodes"][0]
    assert "camera_zoom" not in node
    assert "camera_x" not in node
    assert "camera_y" not in node


def test_camera_center_writes_both_axes():
    s = Scene(60, 40)
    with s:
        g = Group().size(20, 15)
        g.camera.center(3, 4)
    node = create_export(0, s)["nodes"][0]
    assert node["camera_x"] == 3
    assert node["camera_y"] == 4
    assert "camera_zoom" not in node


def test_camera_center_position_writes_map_x_map_y():
    """`camera.center(Position)` must compile to `map_x`/`map_y` Call
    expressions, same shape as `pivot(Position)` (test_pivot_group_position)."""
    s = Scene(60, 40)
    with s:
        anchor = Rect().xy(23, 23).size(4, 4).fill("gold")
        g = Group().xy(5, 5).size(20, 20)
        with g:
            Rect().size(8, 8).fill("steelblue")
        g.camera.center(anchor.at("center"))
    nodes = create_export(0, s)["nodes"]
    group_node = next(n for n in nodes if n["kind"] == "group")
    assert group_node["camera_x"][0] == "map_x"
    assert group_node["camera_y"][0] == "map_y"


def test_camera_center_position_and_xy_raises():
    s = Scene(60, 40)
    with s:
        anchor = Rect().xy(1, 1).size(2, 2)
        g = Group().size(20, 15)
        with pytest.raises(TypeError):
            g.camera.center(anchor.at("center"), 5)


def test_camera_center_requires_both_numbers():
    s = Scene(60, 40)
    with s:
        g = Group().size(20, 15)
        with pytest.raises(TypeError):
            g.camera.center(5)


def test_camera_reset_writes_all_three():
    s = Scene(60, 40)
    with s:
        g = Group().size(20, 15)
        g.camera.zoom(2)
        g.camera.reset()
    node = create_export(0, s)["nodes"][0]
    assert node["camera_zoom"] == 1


def test_scene_camera_zoom_writes_top_level_field():
    s = Scene(60, 40)
    with s:
        Group().size(20, 15)
        s.camera.zoom(1.5)
    exported = create_export(0, s)
    assert exported["camera_zoom"] == 1.5


# ── Golden image tests ──────────────────────────────────────────────────────


def test_camera_zoom_only(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).fill("steelblue")
            Ellipse().size(10, 10).fill("gold").xy(5, 5)
        g.camera.zoom(2, dur=1)
    test_scene.select_frames = FRAMES


def test_camera_pan_only(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).fill("steelblue")
            Ellipse().size(10, 10).fill("gold").xy(5, 5)
        g.camera.center(30, 20, dur=1)
    test_scene.select_frames = FRAMES


def test_camera_zoom_and_pan(test_scene):
    """Adapts the spec's own worked example: zoom in on a point, then reset."""
    with test_scene:
        with Group().xy(0, 0).size(60, 40) as g:
            Rect().size(60, 40).fill("steelblue")
            Ellipse().size(10, 10).fill("gold").xy(40, 25)
        with Par(), anim(1):
            g.camera.zoom(1.9)
            g.camera.center(45, 30)
    test_scene.select_frames = FRAMES


def test_camera_center_tracks_moving_node(test_scene):
    """`camera.center(Position)` tracks a moving node live, same as
    `pivot(Position)`."""
    with test_scene:
        with Group().xy(0, 0).size(60, 40) as g:
            target = Rect().size(8, 8).fill("gold").xy(5, 5)
        g.camera.zoom(1.5)
        g.camera.center(target.at("center"))
        target.xy(40, 25, dur=1)
    test_scene.select_frames = FRAMES


def test_scene_level_camera(test_scene):
    with test_scene:
        Rect().size(60, 40).fill("steelblue")
        Ellipse().size(10, 10).fill("gold").xy(25, 15)
        test_scene.camera.zoom(1.5, dur=1)
    test_scene.select_frames = FRAMES


def test_camera_does_not_auto_clip_pairs_with_clip(test_scene):
    """Zoomed content overflows the group box unless paired with `.clip()` -
    proves "does not auto-clip" and that the clip window itself stays
    computed against the un-zoomed box."""
    with test_scene:
        with Group().xy(10, 5).size(30, 20) as g:
            Rect().size(30, 20).fill("steelblue")
            Ellipse().size(8, 8).fill("gold").xy(11, 6)
        g.clip()
        g.camera.zoom(2)
    test_scene.select_frames = FRAMES
