def test_follow_path(test_scene):
    with test_scene.size(100, 100):
        with group().size(80, 80):
            p = path()
            p.stroke_color("black")
            p.move_to().xy(0, 0)
            p.line_to().xy(30, 10)
            p.cubic_to().xy(0, 50).c1_xy(10, 0).c2_xy(15, 45)
            p.cubic_to().xy(40, 20).c1_xy(-15, -45).c2_xy(40, 20)
            ellipse().size(5, 5).color("green").follow_path(p, frames=16)