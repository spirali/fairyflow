from fairyflow import Group, Rect, Scene

FRAMES = [0, 12, 24]


def test_reveal_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("down", dur=1)
    test_scene.select_frames = FRAMES


def test_reveal_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("up", dur=1)
    test_scene.select_frames = FRAMES


def test_reveal_right(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("right", dur=1)
    test_scene.select_frames = FRAMES


def test_reveal_left(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("left", dur=1)
    test_scene.select_frames = FRAMES


def test_hide_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("down", dur=1)
    test_scene.select_frames = FRAMES


def test_hide_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("up", dur=1)
    test_scene.select_frames = FRAMES


def test_hide_right(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("right", dur=1)
    test_scene.select_frames = FRAMES


def test_hide_left(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("left", dur=1)
    test_scene.select_frames = FRAMES


def test_clip_xywh(test_scene):
    """clip(x=, y=, w=, h=) sets multiple clip axes at once."""
    with test_scene:
        with Group().size(80, 60) as g:
            Rect().size(80, 60).color("coral")
        g.clip(w=0.5, h=0.5)


def test_clip_bare_clips_to_own_box(test_scene):
    """A bare `.clip()` (no args) enables clipping to the group's own box -
    regression test for the bare-call-is-a-no-op bug."""
    with test_scene:
        with Group().xy(5, 5).size(20, 15) as g:
            Rect().size(40, 30).color("coral")  # overflows g's box on both axes
        g.clip()


def test_clip_bare_call_writes_explicit_wire_defaults():
    """A bare `.clip()` must write clip_x/y/w/h explicitly (even though they
    equal the engine's own defaults) so the wire distinguishes "explicitly
    enabled" from "never called clip()" - this is the mechanism behind the
    fix for the bare-call-is-a-no-op bug, checked independently of rendering."""
    from fairyflow.serializer import create_export

    s1 = Scene(60, 40)
    with s1:
        g1 = Group().size(20, 15)
        g1.clip()
    node1 = create_export(0, s1)["nodes"][0]
    assert (node1["clip_x"], node1["clip_y"], node1["clip_w"], node1["clip_h"]) == (
        0,
        0,
        1,
        1,
    )

    s2 = Scene(60, 40)
    with s2:
        Group().size(20, 15)  # .clip() never called
    node2 = create_export(0, s2)["nodes"][0]
    for key in ("clip_x", "clip_y", "clip_w", "clip_h"):
        assert key not in node2
