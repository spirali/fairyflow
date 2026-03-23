from fairyflow import Rect, Group, adv_frames, set_frame, linear


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
        adv_frames(1)
        r = Rect().xy(25, 5).size(10, 20).color("red")
        adv_frames(1)
        r.remove()


def test_simple_move(test_scene):
    with test_scene:
        r = Rect().size(10, 20).color("red").xy(5, 5)

        set_frame(3)
        r.hold()

        set_frame(6)
        linear()
        r.xy(15, 5).color("orange")

        set_frame(9)
        r.hold()

        set_frame(12)
        r.move(12, 4).color("blue")

        set_frame(15)
        r.hold()

        set_frame(18)
        r.move(5, 0)
