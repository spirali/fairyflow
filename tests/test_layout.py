from fairyflow import Group, Rect, Ellipse


def test_centering_layout1(test_scene):
    with test_scene:
        with Group():
            Ellipse().size(20, 25).color("orange")
            with Group():
                Rect().size(15, 10).color("blue")


def test_centering_layout2(test_scene):
    with test_scene:
        with Group():
            Rect().size(10, 10).color("orange")
            Rect().size(3, 3).y(15).color("blue")


def test_column_layout(test_scene):
    with test_scene.size(50, 80):
        with Group().column(gap=5, align=1.0):
            Ellipse().size(25, 5).color("orange")
            with Group().column():
                Rect().size(15, 10).color("blue")
                Rect().size(10, 15).color("orange")
            Ellipse().size(10, 5).color("green")


def test_row_layout(test_scene):
    with test_scene.size(80, 50):
        with Group().row(gap=5, align=1.0):
            Ellipse().size(25, 5).color("orange")
            with Group().row():
                Rect().size(15, 10).color("blue")
                Rect().size(10, 15).color("orange")
            Ellipse().size(10, 5).color("green")
