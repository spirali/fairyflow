import pytest

from fairyflow import Path, Group, Ellipse, Scene


def test_follow_path(test_scene):
    with test_scene.size(100, 100):
        with Group().size(80, 80):
            p = Path()
            p.stroke("black")
            p.move_to(0, 0)
            p.line_to(30, 10)
            p.cubic_to(0, 50, c1=(10, 0), c2=(15, 45))
            p.cubic_to(40, 20, c1=(-15, -45), c2=(40, 20))
            Ellipse().size(5, 5).fill("green").follow_path(p, dur=0.5)


def test_follow_path_backwards(test_scene):
    with test_scene.size(100, 100):
        with Group().size(80, 80):
            p = Path()
            p.stroke("black")
            p.move_to(0, 0)
            p.line_to(30, 10)
            p.cubic_to(0, 50, c1=(10, 0), c2=(15, 45))
            p.cubic_to(40, 20, c1=(-15, -45), c2=(40, 20))
            Ellipse().size(5, 5).fill("red").follow_path(p, dur=0.5, start=1, end=0)


def test_arrows(test_scene):
    test_scene.target_resolution = (320, 320)
    test_scene.pdf_tolerance = 60
    with test_scene.size(80, 80):
        p = Path()
        p.stroke("black")
        p.move_to(20, 10)
        p.line_to(50, 20)
        p.arrow("end")
        p.arrow("start").fill("green")

        p = Path()
        p.stroke("orange")
        p.move_to(20, 20)
        p.cubic_to(50, 60, c1=(-50, 25), c2=(35, 20))

        p.arrow("end")
        p.arrow("start").fill("green")


def test_line_to_accepts_position():
    """`line_to(a_position)` produces the same wire x/y as the equivalent
    explicit-coordinate call - both spellings must resolve identically."""
    s = Scene(100, 100)
    with s:
        with Group().size(100, 100):
            anchor = Ellipse().size(20, 20).xy(30, 40)
            p = Path().stroke("black")
            p.move_to(0, 0)
            expected = anchor.at("right").into_node(p)
            handle = p.line_to(anchor.at("right"))
    assert repr(handle._get_attr("x").get_first_value()) == repr(expected.x)
    assert repr(handle._get_attr("y").get_first_value()) == repr(expected.y)


def test_move_to_requires_y_when_not_position():
    """move_to(x) with a plain number and no y is an invalid call shape, not
    a bad value - TypeError, matching the font()/at() conflict-guard style."""
    with Scene(100, 100):
        p = Path()
        with pytest.raises(TypeError):
            p.move_to(5)


def test_line_to_requires_y_when_not_position():
    with Scene(100, 100):
        p = Path()
        p.move_to(0, 0)
        with pytest.raises(TypeError):
            p.line_to(5)


def test_cubic_to_sets_endpoint_and_control_points_in_one_call():
    with Scene(100, 100):
        p = Path()
        p.move_to(0, 0)
        c = p.cubic_to(40, 20, c1=(-15, -45), c2=(40, 20))
    assert c._attrs["x"].get_first_value() == 40
    assert c._attrs["y"].get_first_value() == 20
    assert c._attrs["c1_x"].get_first_value() == -15
    assert c._attrs["c1_y"].get_first_value() == -45
    assert c._attrs["c2_x"].get_first_value() == 40
    assert c._attrs["c2_y"].get_first_value() == 20


def test_c1_c2_set_both_axes_as_one_keyframe():
    """c1()/c2() must wrap their two _set_attr calls in Par() so an animated
    call lands both axes in the same keyframe - regression for a latent
    inconsistency in the old c1_xy()/c2_xy() (unlike every other compound
    setter in this file, they never wrapped in Par())."""
    with Scene(100, 100):
        p = Path()
        p.move_to(0, 0)
        c = p.cubic_to(40, 20)
        c.c1(10, -20, dur=0.5)
    frames_x = sorted(c._attrs["c1_x"].values)
    frames_y = sorted(c._attrs["c1_y"].values)
    assert frames_x == frames_y


def test_crop_unified_setter_sets_named_sides_only():
    with Scene(100, 100):
        p = Path()
        p.move_to(0, 0)
        p.line_to(10, 10)
        p.crop(end=0.5)
    assert p._attrs["crop_end"].get_first_value() == 0.5
    assert p._get_attr("crop_start").get_first_value() == 0.0


def test_crop_dur_and_ease_thread_through():
    with Scene(100, 100):
        p = Path()
        p.move_to(0, 0)
        p.line_to(10, 10)
        p.crop(start=0, end=1, dur=1)
    assert len(p._attrs["crop_start"].values) == 2
    assert len(p._attrs["crop_end"].values) == 2
