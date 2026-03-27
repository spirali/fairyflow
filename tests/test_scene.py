from alsie import *


def test_simple_boxes(test_scene):
    with test_scene:
        rect().xy(5, 5).size(10, 20).color("orange")
        frame(1)
        r = rect().xy(25, 5).size(10, 20).color("red")
        frame(2)
        r.remove()


def test_simple_move(test_scene):
    with test_scene:
        r = rect().size(10, 20).color("red").xy(5, 5)

        frame(3)
        r.hold()

        frame(6)
        linear()
        r.xy(15, 5).color("orange")

        frame(9)
        r.hold()

        frame(12)
        r.move(12, 4).color("blue")

        frame(15)
        r.hold()

        frame(18)
        r.move(5, 0)
