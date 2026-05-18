from fairyflow import Rect, Group, next_frame, wait, Frames, Par


def test_zlevel_overlapping(test_scene):
    """Higher z-level rect is rendered on top regardless of creation order."""
    with test_scene:
        # red created first, but lower z — should end up below
        Rect().xy(5, 5).size(20, 20).color("red").z_level(1)
        Rect().xy(10, 10).size(20, 20).color("blue").z_level(2)


def test_zlevel_overlapping_creation_order(test_scene):
    """Z-level overrides creation order: blue created first but higher z stays on top."""
    with test_scene:
        Rect().xy(10, 10).size(20, 20).color("blue").z_level(2)
        Rect().xy(5, 5).size(20, 20).color("red").z_level(1)


def test_zlevel_group_inherit(test_scene):
    """Children inherit z-level from their group; group z determines order among scene siblings."""
    with test_scene:
        # group at z=1: its rect child inherits z=1 from the group
        with Group().z_level(1):
            Rect().xy(5, 5).size(20, 20).color("red")
        # group at z=2: its rect child inherits z=2, rendered on top
        with Group().z_level(2):
            Rect().xy(10, 10).size(20, 20).color("blue")


def test_zlevel_mixed_own_and_inherited(test_scene):
    """A rect with its own z-level overrides the inherited value from the group."""
    with test_scene:
        # group at z=2, but child has own z=1 — should be below the z=2 standalone rect
        with Group().z_level(2):
            Rect().xy(10, 10).size(20, 20).color("blue").z_level(1)
        Rect().xy(5, 5).size(20, 20).color("red").z_level(2)


def test_simple_boxes(test_scene):
    with test_scene:
        Rect().xy(5, 5).size(10, 20).color("orange")
        next_frame()
        r = Rect().xy(25, 5).size(10, 20).color("red")
        next_frame()
        r.remove()


def test_simple_move(test_scene):
    with test_scene:
        f3 = Frames(3)
        r = Rect().size(10, 20).color("red").xy(5, 5)
        wait(f3)
        with Par():
            r.xy(15, 5, f3).color("orange", f3)
        wait(Frames(3))
        with Par():
            r.move(12, 4, f3).color("blue", f3)
        wait(Frames(3))
        r.move(5, 0, f3)


def test_fill_color_in_next_frame(test_scene):
    with test_scene:
        r = Rect().size(20, 20)
        next_frame()
        r.color("green")
