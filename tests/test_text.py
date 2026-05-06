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
