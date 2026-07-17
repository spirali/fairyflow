from fairyflow import Group, Rect

FRAMES = [0, 12, 24]


def test_reveal_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("down")
    test_scene.select_frames = FRAMES


def test_reveal_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("up")
    test_scene.select_frames = FRAMES


def test_reveal_right(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("right")
    test_scene.select_frames = FRAMES


def test_reveal_left(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal("left")
    test_scene.select_frames = FRAMES


def test_hide_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("down")
    test_scene.select_frames = FRAMES


def test_hide_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("up")
    test_scene.select_frames = FRAMES


def test_hide_right(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("right")
    test_scene.select_frames = FRAMES


def test_hide_left(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide("left")
    test_scene.select_frames = FRAMES


def test_clip_xywh(test_scene):
    """clip(x=, y=, w=, h=) sets multiple clip axes at once."""
    with test_scene:
        with Group().size(80, 60) as g:
            Rect().size(80, 60).color("coral")
        g.clip(w=0.5, h=0.5)
