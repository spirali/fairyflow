import pytest

from fairyflow import Rect, Scene, Text, code, gradient
from fairyflow.config import (
    DEFAULT_FONT,
    DEFAULT_CODE,
    set_default_font,
    set_default_code,
)
from fairyflow.serializer import create_export


@pytest.fixture(autouse=True)
def _reset_default_font():
    """DEFAULT_FONT/DEFAULT_CODE are process-global mutable state; keep
    set_default_font()/set_default_code() calls in one test from leaking into
    the next."""
    font_snapshot = DEFAULT_FONT.copy()
    code_snapshot = DEFAULT_CODE.copy()
    yield
    DEFAULT_FONT.clear()
    DEFAULT_FONT.update(font_snapshot)
    DEFAULT_CODE.clear()
    DEFAULT_CODE.update(code_snapshot)


def _node(scene):
    return create_export(0, scene)["nodes"][0]


def _nodes_by_kind(scene, kind):
    return [n for n in create_export(0, scene)["nodes"] if n["kind"] == kind]


# ── set_default_font() composition ──────────────────────────────────────────


def test_set_default_font_composes_across_calls():
    set_default_font("Inter")
    set_default_font(size=28)
    assert DEFAULT_FONT["font"] == "Inter"
    assert DEFAULT_FONT["font_size"] == 28


def test_set_default_font_bold_and_weight_raises():
    with pytest.raises(TypeError):
        set_default_font(bold=True, weight=700)


def test_set_default_font_mono_and_family_raises():
    with pytest.raises(TypeError):
        set_default_font(mono=True, family="Foo")


def test_set_default_font_bold_sugar():
    set_default_font(bold=True)
    assert DEFAULT_FONT["font_weight"] == 800


def test_set_default_font_mono_sugar():
    set_default_font(mono=True)
    assert DEFAULT_FONT["font"] == "monospace"


def test_set_default_font_invalid_fill_raises_immediately():
    # Must raise inside the call itself (eager, like set_default_scene's
    # background=), not get deferred to the first Text() construction.
    with pytest.raises(ValueError):
        set_default_font(fill="not-a-real-color")
    assert "fill" not in DEFAULT_FONT


def test_set_default_font_gradient_fill():
    g = gradient("tomato", "gold")
    set_default_font(fill=g)
    assert DEFAULT_FONT["fill"] is g


# ── Materialization onto Text nodes ─────────────────────────────────────────


def test_configured_font_materializes_on_new_text():
    set_default_font("Inter", 28)
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert node["font"] == "Inter"
    assert node["font_size"] == 28


def test_unconfigured_font_fields_stay_absent():
    set_default_font(size=28)
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert node["font_size"] == 28
    assert "font" not in node
    assert "font_weight" not in node
    assert "italic" not in node


def test_no_default_font_configured_leaves_wire_sparse():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "font" not in node
    assert "font_size" not in node


def test_explicit_font_call_overrides_configured_default():
    set_default_font("Inter", 28)
    s = Scene(100, 100)
    with s:
        t = Text("hi")
        t.font("monospace", 12)
    node = _node(s)
    assert node["font"] == "monospace"
    assert node["font_size"] == 12


def test_configured_fill_materializes_on_new_text():
    set_default_font(fill="white")
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert node["fill"] == "white"


def test_configured_fill_does_not_reach_shapes():
    set_default_font(fill="white")
    s = Scene(100, 100)
    with s:
        Text("hi")
        Rect().size(10, 10)
    nodes = create_export(0, s)["nodes"]
    rect_node = next(n for n in nodes if n["kind"] == "rect")
    assert "fill" not in rect_node


def test_no_default_fill_configured_keeps_black():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert node["fill"] == "black"


def test_explicit_fill_call_overrides_configured_default():
    set_default_font(fill="white")
    s = Scene(100, 100)
    with s:
        t = Text("hi")
        t.fill("red")
    node = _node(s)
    assert node["fill"] == "red"


def test_text_created_before_set_default_font_is_unaffected():
    s = Scene(100, 100)
    with s:
        t = Text("hi")
        set_default_font("Inter", 28)
    node = create_export(0, s)["nodes"][0]
    assert "font" not in node
    assert "font_size" not in node
    assert t is not None


# ── set_default_code() composition ──────────────────────────────────────────


def test_set_default_code_composes_across_calls():
    set_default_code("python")
    set_default_code(theme="base16-ocean.dark")
    set_default_code(size=18)
    assert DEFAULT_CODE["language"] == "python"
    assert DEFAULT_CODE["theme"] == "base16-ocean.dark"
    assert DEFAULT_CODE["size"] == 18


def test_default_code_family_is_monospace_by_default():
    assert DEFAULT_CODE["family"] == "monospace"


# ── code() ───────────────────────────────────────────────────────────────────


def test_code_dedents_and_strips_blank_lines():
    s = Scene(100, 100)
    with s:
        t = code(
            """
            def sieve(n):
                return [i for i in range(n)]
            """
        )
    assert [span._attrs["text"].values[0] for span in t._children] == [
        "def sieve(n):",
        "    return [i for i in range(n)]",
    ]


def test_code_does_not_parse_markup():
    s = Scene(100, 100)
    with s:
        t = code("a < b and Vec<T>")
    span = t._children[0]
    assert (
        span._attrs["text"].values[span._attrs["text"].init_frame] == "a < b and Vec<T>"
    )


def test_code_is_monospace_and_unhighlighted_by_default():
    s = Scene(100, 100)
    with s:
        code("x = 1")
    node = _node(s)
    assert node["font"] == "monospace"
    assert "sh_language" not in node


def test_code_picks_up_set_default_code():
    set_default_code("rust", theme="base16-ocean.dark", size=18)
    s = Scene(100, 100)
    with s:
        code("let x = 1;")
    node = _node(s)
    assert node["font"] == "monospace"
    assert node["font_size"] == 18
    assert node["sh_language"] == "rust"
    assert node["sh_theme"] == "base16-ocean.dark"


def test_code_call_args_override_set_default_code():
    set_default_code("rust", theme="dark")
    s = Scene(100, 100)
    with s:
        code("x = 1", "python", theme="light")
    node = _node(s)
    assert node["sh_language"] == "python"
    assert node["sh_theme"] == "light"


def test_code_dedent_false_keeps_raw_text():
    s = Scene(100, 100)
    with s:
        t = code("  raw", dedent=False)
    span = t._children[0]
    assert span._attrs["text"].values[span._attrs["text"].init_frame] == "  raw"


# ── .code() on Text/TextGroup/TextSpan ──────────────────────────────────────


def test_dot_code_on_text_enables_highlighting():
    s = Scene(100, 100)
    with s:
        Text("x = 1").code("python")
    node = _node(s)
    assert node["font"] == "monospace"
    assert node["sh_language"] == "python"


def test_dot_code_on_span_is_style_only():
    s = Scene(100, 100)
    with s:
        t = Text("a ")
        span = t.span("map()").code()
    tspan_nodes = _nodes_by_kind(s, "tspan")
    node = next(n for n in tspan_nodes if n["text"] == "map()")
    assert node["font"] == "monospace"
    assert "sh_language" not in node
    assert span is not None


def test_dot_code_on_span_with_language_raises():
    s = Scene(100, 100)
    with s:
        span = Text("x").span("y")
        with pytest.raises(TypeError):
            span.code("python")


# ── Golden image ─────────────────────────────────────────────────────────────


def test_default_font_renders(test_scene):
    test_scene.pdf_tolerance = 60  # bold monospace glyphs cross many AA edges
    set_default_font("monospace", 24, bold=True, fill="tomato")
    with test_scene.size(320, 60).background("#111"):
        Text("styled by default")


def test_code_renders(test_scene):
    test_scene.pdf_tolerance = 60
    set_default_code("python", theme="base16-ocean.dark")
    with test_scene.size(320, 90).background("#2b303b"):
        code(
            """
            x = "world"
            print(f"Hello {x}!")
            """
        )
