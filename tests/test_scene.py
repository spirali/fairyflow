from alsie import *


def test_simple_boxes(test_scene):
    with test_scene:
        rect().xy(5, 5).size(10, 20).color("orange")
        rect().xy(25, 5).size(10, 20).color("red")


def test_simple_move(test_scene):
    with test_scene:
        r = rect().size(10, 20).color("red").xy(5, 5)
        r.frame(3).hold()
        r.frame(6).linear().xy(15, 5).color("orange")
        r.frame(9).hold()
        r.frame(12).move(12, 4).color("blue")
        r.frame(15).hold()
        r.move(5, 0)
