from fairyflow import DEFAULT, Group, Rect, Ellipse, next_frame, rel


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


def test_column_reserve_true(test_scene):
    """With reserve=True, the first item stays in place when the second appears."""
    with test_scene.size(60, 60):
        with Group().column(gap=10, reserve=True):
            Rect().size(40, 10).color("steelblue")
            next_frame()
            Rect().size(40, 10).color("coral")


def test_column_reserve_false(test_scene):
    """With reserve=False, the first item recentres when the second appears."""
    with test_scene.size(60, 60):
        with Group().column(gap=10, reserve=False):
            Rect().size(40, 10).color("steelblue")
            next_frame()
            Rect().size(40, 10).color("coral")


def test_relative_size_half_parent(test_scene):
    """size(rel(0.5), rel(0.5)) sizes a rect to half its parent's width and height."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().expand().color("whitesmoke")
            Rect().size(rel(0.5), rel(0.5)).color("steelblue")


def test_expand_fills_parent(test_scene):
    """expand() covers the entire parent group."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().expand().color("coral")


def test_xy_default_resets_to_layout(test_scene):
    """xy(DEFAULT, DEFAULT) resets a moved node back to its centering-layout position."""
    with test_scene:
        with Group():
            r = Rect().size(20, 20).color("steelblue")
            r.xy(10, 10)
            next_frame()
            r.xy(DEFAULT, DEFAULT)


def test_align_x_and_y(test_scene):
    """align(x=, y=) places a sized node proportionally within its parent, per axis."""
    with test_scene.size(60, 60):
        with Group().size(60, 60):
            Rect().size(20, 20).color("tomato").align(0, 0)
            Rect().size(20, 20).color("gold").align(1, 1)
            Rect().size(20, 20).color("mediumseagreen").align(x=1, y=0)


def test_rel_in_position_slot(test_scene):
    """rel() also works in x()/y()/xy(), composed with arithmetic (rel(1) - 30)."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().size(30, 30).color("orchid").xy(x=rel(1) - 30, y=rel(0.5))


def test_rel_directly_under_scene(test_scene):
    """rel() resolves against the Scene itself when a node has no intermediate
    Group() parent — parent_group() returns the Scene in that case."""
    with test_scene.size(120, 80):
        Rect().size(rel(0.5), rel(0.5)).xy(x=rel(0.5)).color("mediumpurple")
