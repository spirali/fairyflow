import pytest

from fairyflow import (
    Frames,
    Group,
    Par,
    Rect,
    Scene,
    cue,
    get_frame,
    next_frame,
    note,
    wait,
)
from fairyflow.serializer import create_export


def test_zlevel_overlapping(test_scene):
    """Higher z-level rect is rendered on top regardless of creation order."""
    with test_scene:
        # red created first, but lower z — should end up below
        Rect().xy(5, 5).size(20, 20).fill("red").z_level(1)
        Rect().xy(10, 10).size(20, 20).fill("blue").z_level(2)


def test_zlevel_overlapping_creation_order(test_scene):
    """Z-level overrides creation order: blue created first but higher z stays on top."""
    with test_scene:
        Rect().xy(10, 10).size(20, 20).fill("blue").z_level(2)
        Rect().xy(5, 5).size(20, 20).fill("red").z_level(1)


def test_zlevel_group_inherit(test_scene):
    """Children inherit z-level from their group; group z determines order among scene siblings."""
    with test_scene:
        # group at z=1: its rect child inherits z=1 from the group
        with Group().z_level(1):
            Rect().xy(5, 5).size(20, 20).fill("red")
        # group at z=2: its rect child inherits z=2, rendered on top
        with Group().z_level(2):
            Rect().xy(10, 10).size(20, 20).fill("blue")


def test_zlevel_mixed_own_and_inherited(test_scene):
    """A rect with its own z-level overrides the inherited value from the group."""
    with test_scene:
        # group at z=2, but child has own z=1 — should be below the z=2 standalone rect
        with Group().z_level(2):
            Rect().xy(10, 10).size(20, 20).fill("blue").z_level(1)
        Rect().xy(5, 5).size(20, 20).fill("red").z_level(2)


def test_simple_boxes(test_scene):
    with test_scene:
        Rect().xy(5, 5).size(10, 20).fill("orange")
        next_frame()
        r = Rect().xy(25, 5).size(10, 20).fill("red")
        next_frame()
        r.remove()


def test_simple_move(test_scene):
    with test_scene:
        f3 = Frames(3)
        r = Rect().size(10, 20).fill("red").xy(5, 5)
        wait(f3)
        with Par():
            r.xy(15, 5, dur=f3).fill("orange", dur=f3)
        wait(Frames(3))
        with Par():
            r.move(12, 4, dur=f3).fill("blue", dur=f3)
        wait(Frames(3))
        r.move(5, 0, dur=f3)


def test_fill_color_in_next_frame(test_scene):
    with test_scene:
        r = Rect().size(20, 20)
        next_frame()
        r.fill("green")


# ── Scene.background() — replaces Scene.color()/Scene(color=) (item 11) ─────


def test_scene_background_ctor_kwarg():
    s = Scene(100, 100, background="tomato")
    with s:
        pass
    assert create_export(0, s)["background"] == "tomato"


def test_scene_background_runtime_setter():
    s = Scene(100, 100)
    with s:
        s.background("steelblue")
    assert create_export(0, s)["background"] == "steelblue"


def test_scene_has_no_color_method():
    s = Scene(100, 100)
    with s:
        pass
    assert not hasattr(s, "color")


# ── Lazy cue() advance (item 18) ────────────────────────────────────────────


def test_cue_advances_before_instant_attr_set():
    s = Scene(60, 40)
    with s:
        r = Rect().size(10, 10)
        cue()
        assert get_frame() == 0
        r.fill("red")
        assert get_frame() == 1


def test_cue_advances_before_node_creation():
    s = Scene(60, 40)
    with s:
        cue()
        assert get_frame() == 0
        Rect().size(10, 10)
        assert get_frame() == 1


def test_cue_advances_before_remove():
    s = Scene(60, 40)
    with s:
        r = Rect().size(10, 10)
        next_frame()
        cue()
        assert get_frame() == 1
        r.remove()
        assert get_frame() == 2


def test_cue_disarmed_by_explicit_wait():
    s = Scene(60, 40)
    with s:
        cue()
        wait(Frames(3))
        assert get_frame() == 3
        # wait() already disarmed the pending advance — no extra frame here
        Rect().size(10, 10)
        assert get_frame() == 3


def test_cue_disarmed_by_next_frame():
    s = Scene(60, 40)
    with s:
        cue()
        next_frame()
        assert get_frame() == 1
        Rect().size(10, 10)
        assert get_frame() == 1


def test_cue_idempotent():
    s = Scene(60, 40)
    with s:
        cue()
        cue()
        Rect().size(10, 10)
        assert get_frame() == 1


def test_trailing_cue_produces_no_extra_frame():
    s = Scene(60, 40)
    with s:
        Rect().size(10, 10)
        cue()
    assert s.max_frame == 0


# ── Scene.flow — replaces cue_at_start (item 18) ────────────────────────────


def test_scene_flow_default_absent_from_wire():
    s = Scene(100, 100)
    with s:
        pass
    assert "flow" not in create_export(0, s)


def test_scene_flow_true_serialized():
    s = Scene(100, 100, flow=True)
    with s:
        pass
    assert create_export(0, s)["flow"] is True


def test_scene_cue_at_start_removed():
    with pytest.raises(TypeError):
        Scene(100, 100, cue_at_start=True)


# ── Speaker notes: note() (item 19) ─────────────────────────────────────────


def test_note_absent_from_wire_when_never_called():
    s = Scene(60, 40)
    with s:
        Rect().size(10, 10)
    assert "notes" not in create_export(0, s)


def test_note_serializes_frame_and_text():
    s = Scene(60, 40)
    with s:
        note("Introduce the problem first.")
    assert create_export(0, s)["notes"] == [[0, "Introduce the problem first."]]


def test_note_does_not_move_the_clock():
    s = Scene(60, 40)
    with s:
        note("no clock movement")
        assert get_frame() == 0
        Rect().size(10, 10)
        assert get_frame() == 0


def test_note_after_cue_lands_on_cue_frame():
    # Matches the proposal's worked example: cue() then note() records the
    # note at the cue's frame — the lazy advance hasn't fired yet — so it
    # lands in the segment that starts at that cue, not the previous one.
    s = Scene(60, 40)
    with s:
        Rect().size(10, 10)
        next_frame()
        cue()
        note("Now the punchline.")
    result = create_export(0, s)
    assert result["cues"] == [1]
    assert result["notes"] == [[1, "Now the punchline."]]


def test_multiple_notes_in_one_segment_preserve_call_order():
    s = Scene(60, 40)
    with s:
        cue()
        note("first paragraph")
        note("second paragraph")
    assert create_export(0, s)["notes"] == [
        [0, "first paragraph"],
        [0, "second paragraph"],
    ]


def test_notes_sorted_by_frame_with_stable_same_frame_order():
    s = Scene(60, 40)
    with s:
        note("at frame 0")
        next_frame()
        note("first at frame 1")
        note("second at frame 1")
    result = create_export(0, s)["notes"]
    # sorted by frame (0 before 1); call order preserved within frame 1
    assert result == [
        [0, "at frame 0"],
        [1, "first at frame 1"],
        [1, "second at frame 1"],
    ]
