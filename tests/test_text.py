from pathlib import Path

import pytest

from fairyflow import gradient, next_frame
from fairyflow.nodes import Image, Rect, Scene
from fairyflow.sentinels import DEFAULT, rel
from fairyflow.serializer import create_export
from fairyflow.text import Text, TextGroup, TextSpan, code, stext

ASSETS = Path(__file__).parent / "assets"


def _span_text(span):
    av = span._attrs["text"]
    return av.values[av.init_frame]


def _nodes(scene):
    return create_export(0, scene)["nodes"]


def _node(scene):
    return _nodes(scene)[0]


@pytest.fixture
def sc():
    s = Scene(100, 100)
    with s:
        yield s


def test_plain_text(sc):
    t = stext("hello")
    assert isinstance(t, Text)
    assert len(t._children) == 1
    assert isinstance(t._children[0], TextSpan)
    assert _span_text(t._children[0]) == "hello"
    assert t._children[0]._name is None


def test_plain_multiline(sc):
    t = stext("line1\nline2")
    assert len(t._children) == 2
    assert _span_text(t._children[0]) == "line1"
    assert _span_text(t._children[1]) == "line2"


def test_strip(sc):
    t = stext("  hello  ")
    assert _span_text(t._children[0]) == "hello"


def test_no_strip(sc):
    t = stext("  hello  ", strip=False)
    assert _span_text(t._children[0]) == "  hello  "


def test_named_span(sc):
    t = stext("<abc>Hello</abc>")
    assert len(t._children) == 1
    span = t._children[0]
    assert isinstance(span, TextSpan)
    assert _span_text(span) == "Hello"
    assert span._name == "abc"


def test_named_span_with_newlines(sc):
    t = stext("<a>line1\nline2</a>")
    assert len(t._children) == 2
    assert isinstance(t._children[0], TextSpan)
    assert isinstance(t._children[1], TextSpan)
    assert _span_text(t._children[0]) == "line1"
    assert _span_text(t._children[1]) == "line2"
    assert t._children[0]._name == "a"
    assert t._children[1]._name == "a"


def test_mixed_newlines_create_top_level_lines(sc):
    # Tag + text with \n: B should share line 1 with <s>, C and D are separate lines
    t = stext("<s color='green'>A</s>B\nC\nD")
    assert len(t._children) == 3
    g = t._children[0]
    assert isinstance(g, TextGroup) and g._name is None
    assert len(g._children) == 2
    assert isinstance(g._children[0], TextSpan) and _span_text(g._children[0]) == "A"
    assert g._children[0]._name == "s"
    assert isinstance(g._children[1], TextSpan) and _span_text(g._children[1]) == "B"
    assert isinstance(t._children[1], TextSpan) and _span_text(t._children[1]) == "C"
    assert isinstance(t._children[2], TextSpan) and _span_text(t._children[2]) == "D"


def test_mixed_top_level(sc):
    t = stext("Text <abc>Hello</abc> <xyz>world!</xyz>")
    assert len(t._children) == 1
    g = t._children[0]
    assert isinstance(g, TextGroup)
    assert g._name is None
    children = g._children
    assert len(children) == 4
    assert isinstance(children[0], TextSpan) and _span_text(children[0]) == "Text "
    assert (
        isinstance(children[1], TextSpan)
        and _span_text(children[1]) == "Hello"
        and children[1]._name == "abc"
    )
    assert isinstance(children[2], TextSpan) and _span_text(children[2]) == " "
    assert (
        isinstance(children[3], TextSpan)
        and _span_text(children[3]) == "world!"
        and children[3]._name == "xyz"
    )


def test_nested_tag_creates_group(sc):
    t = stext("<a>one<b>two</b></a>")
    assert len(t._children) == 1
    g = t._children[0]
    assert isinstance(g, TextGroup)
    assert g._name == "a"
    assert len(g._children) == 2
    assert isinstance(g._children[0], TextSpan) and _span_text(g._children[0]) == "one"
    assert (
        isinstance(g._children[1], TextSpan)
        and _span_text(g._children[1]) == "two"
        and g._children[1]._name == "b"
    )


def test_deeply_nested(sc):
    t = stext("<a><b><c>text</c></b></a>")
    g_a = t._children[0]
    assert isinstance(g_a, TextGroup) and g_a._name == "a"
    g_b = g_a._children[0]
    assert isinstance(g_b, TextGroup) and g_b._name == "b"
    span = g_b._children[0]
    assert (
        isinstance(span, TextSpan) and _span_text(span) == "text" and span._name == "c"
    )


def test_custom_delimiters(sc):
    t = stext("[a]hello[/a]", delimiters="[]")
    assert len(t._children) == 1
    span = t._children[0]
    assert isinstance(span, TextSpan)
    assert _span_text(span) == "hello"
    assert span._name == "a"


def test_empty_input(sc):
    t = stext("")
    assert isinstance(t, Text)
    assert len(t._children) == 0


def test_unclosed_tag_raises(sc):
    with pytest.raises(ValueError):
        stext("<a>unclosed")


def test_unexpected_closing_tag_raises(sc):
    with pytest.raises(ValueError):
        stext("<a>text</b>")


# ── Tag attribute tests ───────────────────────────────────────────────────────


def _attr_value(node, attr):
    av = node._attrs[attr]
    return av.values[av.init_frame]


def test_tag_name_is_not_color(sc):
    # tag name alone does NOT set a color — use color='...' for that
    t = stext("<green>INFO</green>")
    span = t._children[0]
    assert isinstance(span, TextSpan)
    assert _span_text(span) == "INFO"
    assert span._name == "green"
    assert not span._has_attr("fill_color")


def test_attr_color_quoted(sc):
    t = stext("<span color='#ff0000'>text</span>")
    span = t._children[0]
    assert _attr_value(span, "fill_color").value == "#ff0000"


def test_attr_color_double_quoted(sc):
    t = stext('<span color="#abc">text</span>')
    span = t._children[0]
    assert _attr_value(span, "fill_color").value == "#abc"


def test_attr_font_size(sc):
    t = stext("<span font-size='24'>text</span>")
    span = t._children[0]
    assert _attr_value(span, "font_size") == 24.0


def test_attr_text_size_alias(sc):
    t = stext("<span text-size='18'>text</span>")
    span = t._children[0]
    assert _attr_value(span, "font_size") == 18.0


def test_attr_bold(sc):
    t = stext("<span bold>text</span>")
    span = t._children[0]
    assert _attr_value(span, "font_weight") == 800


def test_attr_italic(sc):
    t = stext("<span italic>text</span>")
    span = t._children[0]
    assert _attr_value(span, "italic") is True


def test_attr_font_name(sc):
    t = stext("<span font='monospace'>text</span>")
    span = t._children[0]
    assert _attr_value(span, "font") == "monospace"


def test_attr_multiple_on_same_tag(sc):
    t = stext("<span color='red' font-size='20' bold>hi</span>")
    span = t._children[0]
    assert _attr_value(span, "fill_color").value == "red"
    assert _attr_value(span, "font_size") == 20.0
    assert _attr_value(span, "font_weight") == 800


def test_attr_on_group(sc):
    # nested children force a TextGroup; attrs must be applied to the group
    t = stext("<outer color='blue'>a<inner>b</inner></outer>")
    g = t._children[0]
    assert isinstance(g, TextGroup)
    assert g._name == "outer"
    assert _attr_value(g, "fill_color").value == "blue"


def test_attr_tag_name_preserved(sc):
    t = stext("<warn color='orange'>!</warn>")
    span = t._children[0]
    assert span._name == "warn"
    assert _attr_value(span, "fill_color").value == "orange"


def test_literal_lt_no_closing_gt(sc):
    # '<' with no '>' must be treated as literal text, not raise
    t = stext("a < b")
    assert len(t._children) == 1
    assert _span_text(t._children[0]) == "a < b"


def test_literal_lt_slash_no_closing_gt(sc):
    # '</' with no '>' (e.g. ASCII art) must be treated as literal text
    t = stext("| |  < / foo |")
    assert len(t._children) == 1
    assert _span_text(t._children[0]) == "| |  < / foo |"


def test_tgroup_inline_two_spans(test_scene):
    """Two same-style spans in a tline render exactly once each."""
    with test_scene.size(200, 40):
        stext("hello world").xy(4, 15).font(size=16)


def test_tgroup_inline_two_spans_colored(test_scene):
    """Two spans with different fill colors in a tline render without duplication."""
    with test_scene.size(200, 40):
        stext("<a>hello</a> <b color='red'>world</b>").xy(4, 15).font(size=16)


def test_tgroup_plain_and_tagged(test_scene):
    """Plain text followed by a colored tag renders as one line, not overlapping."""
    with test_scene.size(240, 40):
        stext("cpus=2 <s color='green'>+ gpus=2</s>").xy(4, 15).font(size=16)


def test_tgroup_three_spans(test_scene):
    """Three inline spans with distinct colors each appear exactly once."""
    with test_scene.size(280, 40):
        stext(
            "<a color='red'>one</a> <b color='blue'>two</b> <c color='green'>three</c>"
        ).xy(4, 15).font(size=16)


def test_tgroup_nested(test_scene):
    """Nested tline (outer + inner tag) does not duplicate glyphs."""
    with test_scene.size(200, 40):
        stext("<outer>foo <inner color='orange'>bar</inner></outer>").xy(4, 15).font(
            size=16
        )


# ── Text ctor / .line() tests ────────────────────────────────────────────────


def test_ctor_single_line_is_bare_span(sc):
    t = Text("hello")
    assert len(t._children) == 1
    assert isinstance(t._children[0], TextSpan)
    assert _span_text(t._children[0]) == "hello"


def test_ctor_multiline_splits_into_top_level_spans(sc):
    t = Text("a\nb")
    assert len(t._children) == 2
    assert isinstance(t._children[0], TextSpan)
    assert isinstance(t._children[1], TextSpan)
    assert _span_text(t._children[0]) == "a"
    assert _span_text(t._children[1]) == "b"


def test_ctor_no_text_has_no_children(sc):
    t = Text()
    assert len(t._children) == 0


def test_span_promotes_bare_line_to_group(sc):
    t = Text("INFO ")
    span = t.span("server started")
    assert len(t._children) == 1
    g = t._children[0]
    assert isinstance(g, TextGroup)
    assert len(g._children) == 2
    assert _span_text(g._children[0]) == "INFO "
    assert _span_text(g._children[1]) == "server started"
    assert span is g._children[1]


def test_line_starts_fresh_top_level_line(sc):
    t = Text("INFO ")
    t.span("server started")
    t.line("second line")
    assert len(t._children) == 2
    assert isinstance(t._children[1], TextSpan)
    assert _span_text(t._children[1]) == "second line"


def test_span_without_ctor_text_starts_bare(sc):
    t = Text()
    span = t.span("a")
    assert len(t._children) == 1
    assert t._children[0] is span
    assert isinstance(span, TextSpan)


def test_span_span_promotes_second_call(sc):
    t = Text()
    t.span("a")
    t.span("b")
    assert len(t._children) == 1
    g = t._children[0]
    assert isinstance(g, TextGroup)
    assert _span_text(g._children[0]) == "a"
    assert _span_text(g._children[1]) == "b"


def test_group_on_fresh_text_is_single_level(sc):
    t = Text()
    group = t.group()
    assert t._children == [group]
    assert isinstance(group, TextGroup)
    group.span("hello")
    assert len(group._children) == 1
    assert isinstance(group._children[0], TextSpan)


def test_text_ctor_multiline(test_scene):
    """A two-line Text built via the ctor renders correctly end-to-end."""
    test_scene.pdf_tolerance = 40  # two lines of vector-drawn glyphs vs. raster AA
    with test_scene.size(200, 60):
        Text("line one\nline two").xy(4, 15).font(size=16)


# ── Sizing: .size()/.expand()/.keep_aspect() ─────────────────────────────────


def test_size_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "w" not in node
    assert "h" not in node
    # keep_aspect defaults `True` but, like `Image`, is seeded eagerly at
    # construction (`_add_attr`) rather than lazily, so it's always present.
    assert node["keep_aspect"] is True


def test_size_both_axes_writes_w_and_h():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30)
    node = _node(s)
    assert node["w"] == 80
    assert node["h"] == 30


def test_size_single_axis_leaves_other_absent():
    s = Scene(100, 100)
    with s:
        Text("hi").size(w=80)
    node = _node(s)
    assert node["w"] == 80
    assert "h" not in node


def test_expand_is_rel_1_on_both_axes():
    s = Scene(100, 100)
    with s:
        t1 = Text("hi").expand()
    node = _node(s)
    assert node["w"] == node["h"]  # both are `mul(parent.dim, 1)` expressions
    assert t1._has_attr("width") and t1._has_attr("height")


def test_keep_aspect_writes_explicit_value():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30).keep_aspect(False)
    node = _node(s)
    assert node["keep_aspect"] is False


def test_keep_aspect_defaults_true_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi").size(80, 30)
    node = _node(s)
    assert node["keep_aspect"] is True


def test_image_keep_aspect_setter_matches_ctor_kwarg():
    # Image's `keep_aspect` was constructor-only before this slice; confirm
    # the new shared setter can override it at runtime too.
    s = Scene(100, 100)
    with s:
        Image(ASSETS / "test.svg", keep_aspect=True).size(50, 40).keep_aspect(False)
    node = _node(s)
    assert node["keep_aspect"] is False


def test_text_size_width_only(test_scene):
    with test_scene:
        Text("Hi").font(size=16).size(w=50)


def test_text_size_height_only(test_scene):
    with test_scene:
        Text("Hi").font(size=16).size(h=30)


def test_text_size_both_keep_aspect_letterboxed(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).size(50, 30)


def test_text_size_both_keep_aspect_false_stretched(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).size(50, 30).keep_aspect(False)


def test_text_expand_poster_text(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene:
        Text("Hi").font(size=16).expand()


# ── Wrapping and alignment: .wrap()/.text_align() ────────────────────────────


def test_wrap_default_removes_the_attribute():
    s = Scene(100, 100)
    with s:
        t = Text("hi").wrap(200)
        t.wrap(DEFAULT)
    node = _node(s)
    assert "wrap" not in node


def test_wrap_accepts_rel():
    s = Scene(100, 100)
    with s:
        Text("hi").wrap(rel(0.5))
    node = _node(s)
    # `rel(0.5)` resolves against the parent Scene's width (100) at
    # serialization time via `resolve_rel`, same as `width()`/`x()`.
    assert node["wrap"] == 50.0


def test_text_align_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "text_align" not in node


def test_text_align_writes_explicit_mode():
    s = Scene(100, 100)
    with s:
        Text("hi").text_align("center")
    node = _node(s)
    assert node["text_align"] == "center"


def test_tgroup_text_align_absent_by_default():
    s = Scene(100, 100)
    with s:
        t = Text()
        line = t.line()
        line.span("hi")
    nodes = _nodes(s)
    node = _node(s)
    line_node = nodes[node["children"][0]]
    assert "text_align" not in line_node


def test_tgroup_text_align_overrides_block_default():
    s = Scene(100, 100)
    with s:
        t = Text().text_align("left")
        line = t.line()
        line.span("hi")
        line.text_align("right")
    nodes = _nodes(s)
    node = _node(s)
    assert node["text_align"] == "left"
    line_node = nodes[node["children"][0]]
    assert line_node["text_align"] == "right"


def test_text_wrap_breaks_multi_word_line(test_scene):
    test_scene.pdf_tolerance = 60
    with test_scene.size(120, 100):
        Text("the quick brown fox jumps").font(size=16).wrap(100).xy(4, 4)


def test_text_align_center(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene.size(160, 100):
        Text("hi\nlong line here").font(size=16).wrap(140).text_align("center").xy(4, 4)


def test_text_align_right(test_scene):
    test_scene.pdf_tolerance = 40
    with test_scene.size(160, 100):
        Text("hi\nlong line here").font(size=16).wrap(140).text_align("right").xy(4, 4)


def test_text_align_justify(test_scene):
    test_scene.pdf_tolerance = 90
    with test_scene.size(160, 100):
        Text("the quick brown fox jumps over the lazy dog").font(size=16).wrap(
            140
        ).text_align("justify")


def test_tgroup_text_align_overrides_block_default_visually(test_scene):
    test_scene.pdf_tolerance = 70
    with test_scene.size(160, 100):
        t = Text().font(size=16).wrap(140).text_align("left").xy(4, 4)
        t.line("left aligned")
        right_line = t.line()
        right_line.span("right aligned")
        right_line.text_align("right")


def test_wrap_and_size_fit_the_wrapped_extent_not_the_wrap_width(test_scene):
    test_scene.pdf_tolerance = 150
    with test_scene.size(200, 100):
        Text("the quick brown fox").font(size=16).wrap(90).size(w=180).xy(4, 4)


# ── Transforms: Text.rotate()/.scale()/.pivot() ──────────────────────────────


def test_text_rotate_scale_pivot_serializes():
    s = Scene(100, 100)
    with s:
        Text("hi").rotate(45).scale(2).pivot(x=0, y=0)
    node = _node(s)
    assert node["rotation"] == 45
    assert node["scale_x"] == 2
    assert node["scale_y"] == 2
    assert node["pivot_x"] == 0
    assert node["pivot_y"] == 0


def test_text_rotate(test_scene):
    with test_scene.size(150, 150):
        Text("Rotated").font(size=20).xy(20, 60).fill("darkred").rotate(20)


def test_text_scale_and_pivot(test_scene):
    with test_scene.size(150, 150):
        t = Text("Grow").font(size=16).xy(20, 60).fill("darkslateblue")
        t.pivot("top_left")
        t.scale(1.8)


# ── Placeable runs: TextGroup/TextSpan .xy()/.move()/.next_to() ─────────────


def test_span_xy_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text().span("hi")
    # A bare `.span()` with no prior line is Text's direct child (no `tline`
    # wrapper), so `children[0]` is the span itself.
    node = _nodes(s)[_node(s)["children"][0]]
    assert "x" not in node
    assert "y" not in node


def test_span_xy_writes_x_y():
    s = Scene(100, 100)
    with s:
        span = Text().span("hi")
        span.xy(120, 45)
    node = _nodes(s)[_node(s)["children"][0]]
    assert node["x"] == 120
    assert node["y"] == 45


def test_span_xy_default_resets():
    # Unlike `wrap(DEFAULT)` (true absence, no auto-op to fall back to),
    # `xy(DEFAULT)` follows the standard `PositionMixin` reset: it writes an
    # explicit `auto_x`/`auto_y`-referencing expression rather than removing
    # the attribute, matching `x(DEFAULT)` on every other placeable node.
    s = Scene(100, 100)
    with s:
        span = Text().span("hi")
        span.xy(120, 45)
        span.xy(DEFAULT, DEFAULT)
    node = _nodes(s)[_node(s)["children"][0]]
    assert node["x"][0] == "auto_x"
    assert node["y"][0] == "auto_y"


def test_tgroup_xy_writes_x_y():
    s = Scene(100, 100)
    with s:
        t = Text()
        line = t.line()
        line.span("hi")
        line.xy(80, 30)
    node = _nodes(s)[_node(s)["children"][0]]
    assert node["x"] == 80
    assert node["y"] == 30


def test_span_move_writes_relative_x_y():
    s = Scene(100, 100)
    with s:
        span = Text().span("hi")
        span.move(5, -3)
    node = _nodes(s)[_node(s)["children"][0]]
    assert "x" in node and "y" in node


def test_word_flies_out_of_sentence(test_scene):
    """A single overridden span draws at its explicit position while its
    siblings keep their natural flowed positions (no reflow) — the gap where
    the overridden word used to be stays empty."""
    # Matches item 15/item 14's precedent: PDF (vector) vs PNG (raster) text
    # AA differs slightly more than the default tolerance allows once
    # multiple runs at different positions are involved.
    test_scene.pdf_tolerance = 60
    with test_scene.size(200, 100):
        t = Text().font(size=16).xy(4, 20).fill("black")
        t.span("The quick brown ")
        fox = t.span("fox")
        fox.fill("darkred")
        fox.xy(120, 60)
        t.span(" jumps")


def test_tgroup_move_displaces_whole_line(test_scene):
    """A `.move()`'d TextGroup line displaces every descendant span that
    doesn't have its own closer override, as a rigid unit."""
    test_scene.pdf_tolerance = 60
    with test_scene.size(200, 150):
        t = Text().font(size=16).xy(4, 20).fill("black")
        t.line("Untouched line")
        line2 = t.line()
        line2.span("Moved ").fill("darkblue")
        line2.span("line")
        line2.xy(60, 100)


def test_span_own_override_wins_over_ancestor_group(test_scene):
    """A span's own override takes precedence over its parent group's,
    matching the "nearest self-or-ancestor wins" cascade rule."""
    with test_scene.size(200, 150):
        t = Text().font(size=16).xy(4, 20).fill("black")
        line = t.line()
        line.span("Second ").fill("darkblue")
        special = line.span("line")
        special.fill("darkred")
        line.xy(100, 100)
        special.xy(10, 60)


def test_next_to_targeting_a_placeable_span(test_scene):
    """`next_to()` still works when its target is a genuinely movable
    (PositionMixin, not just query-only) TextSpan."""
    with test_scene.size(150, 80):
        t = Text().font(size=14).xy(4, 15).fill("black")
        span = t.span("target")
        r = Rect().size(8, 8).fill("tomato")
        r.next_to(span, "right", gap=3)


# ── Per-run transforms: TextGroup/TextSpan .rotate()/.scale()/.pivot() ──────


def test_span_rotate_scale_pivot_serializes():
    s = Scene(100, 100)
    with s:
        span = Text().span("hi")
        span.rotate(45).scale(2).pivot(x=0, y=0)
    node = _nodes(s)[_node(s)["children"][0]]
    assert node["rotation"] == 45
    assert node["scale_x"] == 2
    assert node["scale_y"] == 2
    assert node["pivot_x"] == 0
    assert node["pivot_y"] == 0


def test_span_rotate_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text().span("hi")
    node = _nodes(s)[_node(s)["children"][0]]
    assert "rotation" not in node
    assert "scale_x" not in node
    assert "pivot_x" not in node


def test_tgroup_scale_serializes():
    s = Scene(100, 100)
    with s:
        t = Text()
        line = t.line()
        line.span("hi")
        line.scale(1.5)
    node = _nodes(s)[_node(s)["children"][0]]
    assert node["scale_x"] == 1.5
    assert node["scale_y"] == 1.5


def test_span_has_no_size_or_z():
    # RotAndScaleMixin doesn't drag in SizeMixin/ZLevelMixin — a run's extent
    # is always its measured glyphs, and paragraph order is draw order.
    s = Scene(100, 100)
    with s:
        span = Text().span("hi")
    assert not hasattr(span, "size")
    assert not hasattr(span, "z")


def test_word_spins_in_place(test_scene):
    with test_scene.size(150, 150):
        t = Text().font(size=20).xy(20, 60).fill("black")
        t.span("spin ")
        word = t.span("me")
        word.fill("darkred")
        word.rotate(30)


def test_word_grows_from_corner_pivot(test_scene):
    with test_scene.size(150, 150):
        t = Text().font(size=16).xy(20, 60).fill("black")
        t.span("grow ")
        word = t.span("me")
        word.fill("darkblue")
        word.pivot("top_left")
        word.scale(2.0)


def test_word_spins_and_moves(test_scene):
    """`.rotate()` and `.xy()` on the same span compose: spin in place
    (around the span's own natural position/pivot), then translate by the
    override delta — the two cascades resolve independently but stack."""
    test_scene.pdf_tolerance = 60
    with test_scene.size(300, 200):
        t = Text().font(size=20).xy(20, 40).fill("black")
        t.span("spin and move ")
        word = t.span("me")
        word.fill("darkred")
        word.rotate(45)
        word.xy(180, 30)


def test_tgroup_rotates_as_a_rigid_unit(test_scene):
    """A rotated `TextGroup` line spins every descendant span that doesn't
    have its own closer transform, as one rigid unit."""
    test_scene.pdf_tolerance = 60
    with test_scene.size(200, 150):
        t = Text().font(size=16).xy(60, 20).fill("black")
        t.line("Untouched line")
        line2 = t.line()
        line2.span("Rotated ").fill("darkblue")
        line2.span("line")
        line2.rotate(15)


def test_span_own_transform_wins_over_ancestor_group(test_scene):
    """A span's own rotate/scale takes precedence over its parent group's,
    matching the "nearest self-or-ancestor wins" cascade rule used for
    position override."""
    test_scene.pdf_tolerance = 60
    with test_scene.size(200, 150):
        t = Text().font(size=16).xy(20, 20).fill("black")
        line = t.line()
        line.span("Second ").fill("darkblue")
        special = line.span("line")
        special.fill("darkred")
        line.rotate(20)
        special.scale(1.8)


# ── Typewriter reveal: type_on() (item 21, proposal §9.6) ───────────────────


def test_reveal_absent_when_type_on_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "reveal" not in node


def test_type_on_produces_snap_to_zero_then_animate_to_one():
    s = Scene(100, 100)
    with s:
        Text("hi").type_on(dur=1)
    node = _node(s)
    assert node["reveal"] == {"k": [[0, 0], [24, 1, "in_out"]]}


def test_type_on_respects_custom_ease():
    s = Scene(100, 100)
    with s:
        Text("hi").type_on(dur=1, ease="out")
    node = _node(s)
    assert node["reveal"]["k"][1] == [24, 1, "out"]


def test_type_on_starts_from_the_current_frame():
    s = Scene(100, 100)
    with s:
        next_frame()
        next_frame()
        Text("hi").type_on(dur=0.5)
    node = _node(s)
    assert node["reveal"]["k"][0] == [2, 0]
    assert node["reveal"]["k"][1] == [14, 1, "in_out"]


# ── Text decorations: underline()/strike() (proposal §10.2, item C2) ────────


def test_underline_absent_when_never_called():
    s = Scene(100, 100)
    with s:
        Text("hi")
    node = _node(s)
    assert "underline" not in node
    assert "strike" not in node


def test_underline_default_call_writes_progress_one():
    s = Scene(100, 100)
    with s:
        Text("hi").underline()
    node = _node(s)
    assert node["underline"] == 1.0


def test_underline_zero_is_explicit_not_absent():
    """.underline(0) must round-trip as a present, explicit 0 -- this is how
    a run opts out of an inherited decoration, so it must be distinguishable
    from "never called" (which stays fully absent, see the test above)."""
    s = Scene(100, 100)
    with s:
        t = Text("hi")
        span = t.span("bye")
        span.underline(0)
    nodes = _nodes(s)
    node = next(n for n in nodes if n["kind"] == "tspan" and n["text"] == "bye")
    assert node["underline"] == 0.0


def test_strike_and_underline_are_independent():
    s = Scene(100, 100)
    with s:
        Text("hi").underline().strike(0.5)
    node = _node(s)
    assert node["underline"] == 1.0
    assert node["strike"] == 0.5


def test_underline_bool_coerces_to_float():
    s = Scene(100, 100)
    with s:
        Text("a").underline(True)
        Text("b").strike(False)
    nodes = _nodes(s)
    assert nodes[0]["underline"] == 1.0
    assert isinstance(nodes[0]["underline"], float)
    assert nodes[2]["strike"] == 0.0


def test_underline_styling_knobs_each_independently_sparse():
    s = Scene(100, 100)
    with s:
        Text("hi").underline(width=3)
    node = _node(s)
    assert node["underline_width"] == 3
    assert "underline_color" not in node
    assert "underline_offset" not in node


def test_underline_color_and_offset_round_trip():
    s = Scene(100, 100)
    with s:
        Text("hi").underline(color="red", offset=2)
    node = _node(s)
    assert node["underline_color"] == "red"
    assert node["underline_offset"] == 2


def test_underline_color_accepts_gradient():
    s = Scene(100, 100)
    with s:
        Text("hi").underline(color=gradient("tomato", "gold"))
    node = _node(s)
    assert node["underline_color"][0] == "gradient"


def test_underline_on_span_absent_falls_back_to_block_on_wire():
    """The span itself carries no `underline` key when it never called
    `.underline()` -- inheritance is resolved by the engine (verified by a
    Rust unit test, `animdef.rs`), not observable from the Python-serialized
    wire shape alone."""
    s = Scene(100, 100)
    with s:
        t = Text("hi").underline()
        t.span("bye")
    nodes = _nodes(s)
    span = next(n for n in nodes if n["kind"] == "tspan" and n["text"] == "bye")
    assert "underline" not in span


def test_underline_dur_animates():
    s = Scene(100, 100)
    with s:
        Text("hi").underline(dur=1)
    node = _node(s)
    assert node["underline"] == {"k": [[0, 0], [24, 1.0, "in_out"]]}


def test_strike_over_tspan():
    s = Scene(100, 100)
    with s:
        t = Text("a ")
        span = t.span("word")
        span.strike(color="darkred", width=2)
    nodes = _nodes(s)
    span_node = next(n for n in nodes if n["kind"] == "tspan" and n["text"] == "word")
    assert span_node["strike"] == 1.0
    assert span_node["strike_color"] == "darkred"
    assert span_node["strike_width"] == 2


# ── Text decorations: golden images ─────────────────────────────────────────


def test_underline_progress_zero_half_and_full(test_scene):
    test_scene.pdf_tolerance = 60  # thin decoration rects cross many AA edges
    with test_scene.size(320, 60):
        t = Text().font(size=24).fill("black")
        t.span("zero").underline(0)
        t.span(" ")
        t.span("half").underline(0.5, color="darkorange")
        t.span(" ")
        t.span("full").underline(color="darkred")


def test_strike_default_color_matches_own_fill(test_scene):
    test_scene.pdf_tolerance = 60
    with test_scene.size(200, 60):
        t = Text("crossed out").font(size=24).fill("darkblue")
        t.strike()


def test_underline_wraps_continuously_across_rows(test_scene):
    # Three lines of text -> proportionally more AA edges.
    test_scene.pdf_tolerance = 200
    with test_scene.size(220, 150):
        t = Text().font(size=20).wrap(180).fill("black")
        t.span("one two three four five six seven eight nine").underline()


def test_strike_over_syntax_highlighted_line(test_scene):
    """The decoration must take the run's own fill, not each token's
    syntax-highlight color -- a strike line with per-token colors would
    indicate `decoration_rects` picked up the wrong color source."""
    with test_scene.size(260, 60).background("#2b303b"):
        t = code('x = "hello world"', "python", theme="base16-ocean.dark")
        t.strike()
