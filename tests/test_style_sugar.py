import pytest

from fairyflow import Ellipse, Path, Rect, Scene, gradient
from fairyflow.serializer import create_export
from fairyflow.text import Text

# ── Rect.radius() ────────────────────────────────────────────────────────────


def test_rect_radius_wire_presence():
    s = Scene(100, 100)
    with s:
        Rect().size(40, 40).radius(6)
    node = create_export(0, s)["nodes"][0]
    assert node["radius"] == 6


def test_rect_radius_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Rect().size(40, 40)
    node = create_export(0, s)["nodes"][0]
    assert "radius" not in node


def test_rect_radius_animatable():
    s = Scene(100, 100)
    with s:
        r = Rect().size(40, 40).radius(0)
        r.radius(10, dur=1)
    frames = sorted(r._attrs["radius"].values)
    assert len(frames) == 2
    assert r._attrs["radius"].values[frames[0]] == 0
    assert r._attrs["radius"].values[frames[1]] == 10


# ── stroke() — color/width regression + dash ────────────────────────────────


def test_stroke_writes_color_and_width():
    s = Scene(100, 100)
    with s:
        Rect().stroke("black", 3)
    node = create_export(0, s)["nodes"][0]
    assert node["stroke"] == "black"
    assert node["stroke_width"] == 3


def test_stroke_color_only():
    s = Scene(100, 100)
    with s:
        Rect().stroke("black")
    node = create_export(0, s)["nodes"][0]
    assert node["stroke"] == "black"
    assert "stroke_width" not in node


def test_stroke_none_disables_previously_set_color():
    """`stroke(None)` explicitly disables the stroke (matches `.fill(None)`
    for fill, and the old `stroke_color(None)` behavior it replaced) -
    distinct from a bare `stroke()` call, which leaves the color untouched.
    `ColorLike` already includes `None` as a real value, so the omitted
    default has to be a dedicated sentinel (`OMITTED`), not `None` itself."""
    s = Scene(100, 100)
    with s:
        r = Rect().stroke("black", 3)
        r.stroke(None)
    node = create_export(0, s)["nodes"][0]
    assert node["stroke"] == ""
    assert node["stroke_width"] == 3  # untouched by the color-only call


def test_stroke_omitted_leaves_color_untouched():
    s = Scene(100, 100)
    with s:
        r = Rect().stroke("black", 3)
        r.stroke(width=5)
    node = create_export(0, s)["nodes"][0]
    assert node["stroke"] == "black"
    assert node["stroke_width"] == 5


def test_stroke_dash_wire_presence():
    s = Scene(100, 100)
    with s:
        Rect().stroke(dash=(6, 4))
    node = create_export(0, s)["nodes"][0]
    assert node["dash_on"] == 6.0
    assert node["dash_off"] == 4.0


def test_stroke_dash_absent_when_never_set():
    s = Scene(100, 100)
    with s:
        Rect().stroke("black", 3)
    node = create_export(0, s)["nodes"][0]
    assert "dash_on" not in node
    assert "dash_off" not in node


@pytest.mark.parametrize("dash", [(0, 4), (6, 0), (-1, 4)])
def test_stroke_dash_non_positive_raises(dash):
    s = Scene(100, 100)
    with s, pytest.raises(ValueError):
        Rect().stroke(dash=dash)


def test_stroke_dash_offset_animatable():
    s = Scene(100, 100)
    with s:
        r = Rect().stroke(dash=(6, 4))
        r.stroke(offset=0)
        r.stroke(offset=20, dur=1)
    frames = sorted(r._attrs["dash_offset"].values)
    assert len(frames) == 2
    assert r._attrs["dash_offset"].values[frames[0]] == 0
    assert r._attrs["dash_offset"].values[frames[1]] == 20


def test_stroke_dash_on_ellipse_and_path_works():
    """Dash applies to all three `Style`-sharing shapes, not just `Rect`."""
    s = Scene(100, 100)
    with s:
        Ellipse().stroke(dash=(4, 2))
        Path().stroke(dash=(4, 2))
    nodes = create_export(0, s)["nodes"]
    assert nodes[0]["dash_on"] == 4.0
    assert nodes[1]["dash_on"] == 4.0


def test_stroke_dash_on_text_raises_typeerror():
    """`Text` doesn't share the engine's `Style` struct — dash there would be
    a silent no-op, so it's rejected explicitly instead (matches the
    project's established "no dead methods" precedent, e.g. path command
    handles excluding rotate/scale/alpha)."""
    s = Scene(100, 100)
    with s:
        t = Text("hello")
        with pytest.raises(TypeError):
            t.stroke(dash=(6, 4))


def test_stroke_offset_on_text_raises_typeerror():
    s = Scene(100, 100)
    with s:
        t = Text("hello")
        with pytest.raises(TypeError):
            t.stroke(offset=5)


# ── Path.draw() ──────────────────────────────────────────────────────────────


def test_path_draw_snaps_crop_end_then_animates():
    s = Scene(100, 100)
    with s:
        p = Path()
        p.move_to(0, 0)
        p.line_to(10, 10)
        p.draw(dur=1)
    frames = sorted(p._attrs["crop_end"].values)
    assert len(frames) == 2
    assert p._attrs["crop_end"].values[frames[0]] == 0
    assert p._attrs["crop_end"].values[frames[1]] == 1


def test_path_draw_instant_without_dur():
    s = Scene(100, 100)
    with s:
        p = Path()
        p.move_to(0, 0)
        p.line_to(10, 10)
        p.draw()
    av = p._attrs["crop_end"]
    assert av.values[av.init_frame] == 1


# ── Golden image tests ───────────────────────────────────────────────────────


def test_rounded_rect(test_scene):
    with test_scene.size(200, 100):
        Rect().xy(20, 20).size(60, 60).radius(12).fill("steelblue")
        Rect().xy(110, 20).size(60, 60).radius(30).fill("mediumseagreen")  # pill


def test_dashed_stroke(test_scene):
    test_scene.pdf_tolerance = 60  # dashed segments cross many raster/vector AA edges
    with test_scene.size(220, 100):
        Rect().xy(10, 10).size(60, 60).stroke("black", 3, dash=(8, 4))
        Ellipse().xy(90, 10).size(60, 60).stroke("black", 3, dash=(8, 4))
        p = Path().stroke("black", 3, dash=(6, 3))
        p.move_to(170, 10)
        p.line_to(200, 70)


def test_path_draw_animation(test_scene):
    with test_scene.size(120, 60):
        p = Path().stroke("darkorange", 3)
        p.move_to(10, 30)
        p.line_to(110, 30)
        p.draw(dur=1)


# ── fill() — replaces color() ──────────────────────


def test_fill_writes_fill_color():
    s = Scene(100, 100)
    with s:
        Rect().fill("tomato")
        Text("hi").fill("steelblue")
    nodes = create_export(0, s)["nodes"]
    assert nodes[0]["fill"] == "tomato"
    assert nodes[1]["fill"] == "steelblue"


def test_shapes_and_text_have_no_color_method():
    s = Scene(100, 100)
    with s:
        r = Rect()
        t = Text("hi")
    assert not hasattr(r, "color")
    assert not hasattr(t, "color")


# ── gradient() — linear gradients ─────────────────


def test_gradient_bare_colors_evenly_spaced():
    s = Scene(100, 100)
    with s:
        Rect().fill(gradient("tomato", "gold", angle=90))
    node = create_export(0, s)["nodes"][0]
    assert node["fill"] == ["gradient", [[0.0, "tomato"], [1.0, "gold"]], 90]


def test_gradient_three_bare_colors_evenly_spaced():
    s = Scene(100, 100)
    with s:
        Rect().fill(gradient("red", "green", "blue"))
    node = create_export(0, s)["nodes"][0]
    assert node["fill"] == [
        "gradient",
        [[0.0, "red"], [0.5, "green"], [1.0, "blue"]],
        0,
    ]


def test_gradient_explicit_offset_tuples_preserved():
    s = Scene(100, 100)
    with s:
        Rect().fill(gradient((0.0, "black"), (0.3, "black"), (1.0, "white")))
    node = create_export(0, s)["nodes"][0]
    assert node["fill"] == [
        "gradient",
        [[0.0, "black"], [0.3, "black"], [1.0, "white"]],
        0,
    ]


def test_gradient_default_angle_is_zero():
    s = Scene(100, 100)
    with s:
        Rect().fill(gradient("red", "blue"))
    node = create_export(0, s)["nodes"][0]
    assert node["fill"][2] == 0


def test_gradient_needs_at_least_one_stop():
    with pytest.raises(ValueError):
        gradient()


def test_stroke_rejects_gradient():
    s = Scene(100, 100)
    with s:
        r = Rect()
        with pytest.raises(TypeError):
            r.stroke(gradient("tomato", "gold"))


def test_background_rejects_gradient():
    s = Scene(100, 100)
    with s, pytest.raises(TypeError):
        s.background(gradient("tomato", "gold"))


def test_text_fill_accepts_gradient_wire_presence():
    # Uniform `.fill()` API — Text can be given a gradient at the Python/wire
    # level (StyleMethods is shared with shapes); actual gradient rendering
    # on text glyphs isn't implemented, it degrades to the first stop.
    s = Scene(100, 100)
    with s:
        Text("hi").fill(gradient("red", "blue"))
    node = create_export(0, s)["nodes"][0]
    assert node["fill"] == ["gradient", [[0.0, "red"], [1.0, "blue"]], 0]
