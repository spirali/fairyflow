import pytest
from fairyflow.nodes import Scene
from fairyflow.text import stext, Text, TextSpan, TextGroup


def _span_text(span):
    av = span._attrs["text"]
    return av.values[av.init_frame]


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
    """Two same-style spans in a t_group render exactly once each."""
    with test_scene.size(200, 40):
        stext("hello world").xy(4, 15).font_size(16)


def test_tgroup_inline_two_spans_colored(test_scene):
    """Two spans with different fill colors in a t_group render without duplication."""
    with test_scene.size(200, 40):
        stext("<a>hello</a> <b color='red'>world</b>").xy(4, 15).font_size(16)


def test_tgroup_plain_and_tagged(test_scene):
    """Plain text followed by a colored tag renders as one line, not overlapping."""
    with test_scene.size(240, 40):
        stext("cpus=2 <s color='green'>+ gpus=2</s>").xy(4, 15).font_size(16)


def test_tgroup_three_spans(test_scene):
    """Three inline spans with distinct colors each appear exactly once."""
    with test_scene.size(280, 40):
        stext(
            "<a color='red'>one</a> <b color='blue'>two</b> <c color='green'>three</c>"
        ).xy(4, 15).font_size(16)


def test_tgroup_nested(test_scene):
    """Nested t_group (outer + inner tag) does not duplicate glyphs."""
    with test_scene.size(200, 40):
        stext("<outer>foo <inner color='orange'>bar</inner></outer>").xy(
            4, 15
        ).font_size(16)
