from fairyflow import DEFAULT, Group, Rect, Ellipse, Scene, next_frame, rel
from fairyflow.serializer import create_export


def test_centering_layout1(test_scene):
    with test_scene:
        with Group():
            Ellipse().size(20, 25).fill("orange")
            with Group():
                Rect().size(15, 10).fill("blue")


def test_centering_layout2(test_scene):
    with test_scene:
        with Group():
            Rect().size(10, 10).fill("orange")
            Rect().size(3, 3).y(15).fill("blue")


def test_column_layout(test_scene):
    with test_scene.size(50, 80):
        with Group().column(gap=5, align=1.0):
            Ellipse().size(25, 5).fill("orange")
            with Group().column():
                Rect().size(15, 10).fill("blue")
                Rect().size(10, 15).fill("orange")
            Ellipse().size(10, 5).fill("green")


def test_row_layout(test_scene):
    with test_scene.size(80, 50):
        with Group().row(gap=5, align=1.0):
            Ellipse().size(25, 5).fill("orange")
            with Group().row():
                Rect().size(15, 10).fill("blue")
                Rect().size(10, 15).fill("orange")
            Ellipse().size(10, 5).fill("green")


def test_row_layout_with_rotated_rect(test_scene):
    """A rotated rect in a Row is laid out using its rotated (outer) bounding
    box, not its raw width - regression test for aabb_offset/get_outer_width
    generalizing from Group-only to any node with rotation/scale."""
    with test_scene.size(80, 50):
        with Group().row(gap=5, align=0.5):
            Rect().size(30, 10).fill("steelblue").rotate(90)
            Rect().size(10, 10).fill("coral")


def test_grid_layout(test_scene):
    """grid(cols=2) places children row-major; each column sized to its
    widest child, each row to its tallest."""
    with test_scene.size(90, 90):
        with Group().grid(cols=2, gap=5):
            Rect().size(40, 10).fill("orange")
            Rect().size(10, 30).fill("blue")
            Rect().size(20, 20).fill("green")


def test_grid_layout_gap_y(test_scene):
    """A separate gap_y from gap (the horizontal gap) is respected."""
    with test_scene.size(100, 100):
        with Group().grid(cols=2, gap=5, gap_y=20):
            Rect().size(30, 10).fill("orange")
            Rect().size(30, 10).fill("blue")
            Rect().size(30, 10).fill("green")
            Rect().size(30, 10).fill("coral")


def test_padding_on_column(test_scene):
    """padding() insets a Column layout's children from the group's own box."""
    with test_scene.size(60, 60):
        with Group().column(gap=10).padding(15):
            Rect().size(20, 10).fill("steelblue")
            Rect().size(20, 10).fill("coral")


def test_padding_most_specific_wins(test_scene):
    """A later padding(top=) call only overrides the side it names."""
    with test_scene.size(60, 60):
        with Group().padding(15) as g:
            g.padding(top=30)
            Rect().size(20, 20).fill("mediumpurple")


def test_align_respects_parent_padding():
    """align() must compose with padding() - it computes an explicit x/y
    expression independently of the engine's auto_x/auto_y (which is where
    padding-awareness was added), so it needs its own padding-aware formula
    or it silently ignores padding entirely. Regression for a bug caught
    after the fact: Table's left-aligned cell content used `.align(0, 0.5)`
    and sat flush against the cell border regardless of the table's
    padding, since align() never consulted it."""
    s = Scene(100, 100)
    with s:
        with Group().size(100, 100).padding(20):
            Rect().size(20, 20).fill("steelblue").align(0, 0)
    node = create_export(0, s)["nodes"][1]
    assert node["x"] == 20
    assert node["y"] == 20


def test_column_reserve_true(test_scene):
    """With reserve=True, the first item stays in place when the second appears."""
    with test_scene.size(60, 60):
        with Group().column(gap=10, reserve=True):
            Rect().size(40, 10).fill("steelblue")
            next_frame()
            Rect().size(40, 10).fill("coral")


def test_column_reserve_false(test_scene):
    """With reserve=False, the first item recentres when the second appears."""
    with test_scene.size(60, 60):
        with Group().column(gap=10, reserve=False):
            Rect().size(40, 10).fill("steelblue")
            next_frame()
            Rect().size(40, 10).fill("coral")


def test_relative_size_half_parent(test_scene):
    """size(rel(0.5), rel(0.5)) sizes a rect to half its parent's width and height."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().expand().fill("whitesmoke")
            Rect().size(rel(0.5), rel(0.5)).fill("steelblue")


def test_expand_fills_parent(test_scene):
    """expand() covers the entire parent group."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().expand().fill("coral")


def test_xy_default_resets_to_layout(test_scene):
    """xy(DEFAULT, DEFAULT) resets a moved node back to its centering-layout position."""
    with test_scene:
        with Group():
            r = Rect().size(20, 20).fill("steelblue")
            r.xy(10, 10)
            next_frame()
            r.xy(DEFAULT, DEFAULT)


def test_align_x_and_y(test_scene):
    """align(x=, y=) places a sized node proportionally within its parent, per axis."""
    with test_scene.size(60, 60):
        with Group().size(60, 60):
            Rect().size(20, 20).fill("tomato").align(0, 0)
            Rect().size(20, 20).fill("gold").align(1, 1)
            Rect().size(20, 20).fill("mediumseagreen").align(x=1, y=0)


def test_rel_in_position_slot(test_scene):
    """rel() also works in x()/y()/xy(), composed with arithmetic (rel(1) - 30)."""
    with test_scene.size(120, 80):
        with Group().size(120, 80):
            Rect().size(30, 30).fill("orchid").xy(x=rel(1) - 30, y=rel(0.5))


def test_rel_directly_under_scene(test_scene):
    """rel() resolves against the Scene itself when a node has no intermediate
    Group() parent — parent_group() returns the Scene in that case."""
    with test_scene.size(120, 80):
        Rect().size(rel(0.5), rel(0.5)).xy(x=rel(0.5)).fill("mediumpurple")
