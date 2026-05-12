from fairyflow import Group, Rect

FRAMES = [0, 12, 24]


def test_reveal_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal_down()
    test_scene.select_frames = FRAMES


def test_reveal_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.reveal_up()
    test_scene.select_frames = FRAMES


def test_hide_down(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide_down()
    test_scene.select_frames = FRAMES


def test_hide_up(test_scene):
    with test_scene:
        with Group().xy(10, 5).size(40, 30) as g:
            Rect().size(40, 30).color("steelblue")
        g.hide_up()
    test_scene.select_frames = FRAMES
