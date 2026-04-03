from fairyflow import *


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


def test_column_layout(test_scene):
    with test_scene.size(50, 80):
        with group().column(gap=5, align=1.0):
            ellipse().size(25, 5).color("orange")
            with group().column():
                rect().size(15, 10).color("blue")
                rect().size(10, 15).color("orange")
            ellipse().size(10, 5).color("green")


def test_row_layout(test_scene):
    with test_scene.size(80, 50):
        with group().row(gap=5, align=1.0):
            ellipse().size(25, 5).color("orange")
            with group().row():
                rect().size(15, 10).color("blue")
                rect().size(10, 15).color("orange")
            ellipse().size(10, 5).color("green")
