from alsie import *

def test_centering_layout1(test_scene):
    with test_scene:
        with group():
            ellipse().size(20, 25).color("orange")
            with group():
                rect().size(15, 10).color("blue")                


def test_centering_layout2(test_scene):
    with test_scene:
        with group():
            rect().size(10, 10).color("orange")
            rect().size(3, 3).y(15).color("blue")