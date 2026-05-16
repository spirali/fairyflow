from fairyflow import Path, Group, Ellipse


def test_follow_path(test_scene):
    with test_scene.size(100, 100):
        with Group().size(80, 80):
            p = Path()
            p.stroke_color("black")
            p.move_to().xy(0, 0)
            p.line_to().xy(30, 10)
            p.cubic_to().xy(0, 50).c1_xy(10, 0).c2_xy(15, 45)
            p.cubic_to().xy(40, 20).c1_xy(-15, -45).c2_xy(40, 20)
            Ellipse().size(5, 5).color("green").follow_path(p, tr=0.5)


def test_arrows(test_scene):
    with test_scene.size(80, 80):
        p = Path()
        p.stroke_color("black")
        p.move_to().xy(20, 10)
        p.line_to().xy(50, 20)
        p.triangle_arrow()
        p.triangle_arrow("start").color("green")

        p = Path()
        p.stroke_color("orange")
        p.move_to().xy(20, 20)
        p.cubic_to().xy(50, 60).c1_xy(-50, 25).c2_xy(35, 20)

        p.triangle_arrow()
        p.triangle_arrow("start").color("green")
