import pytest
from fairyflow import Group, Path, Rect, Scene, Text
from fairyflow.exprs import Call


@pytest.fixture
def sc():
    s = Scene(100, 100)
    with s:
        yield s


def test_at_no_args_is_center(sc):
    r = Rect().size(40, 20).xy(10, 10)
    p = r.at()
    expected_x = r._get_attr("x") + r._get_attr("width") * 0.5
    expected_y = r._get_attr("y") + r._get_attr("height") * 0.5
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)


def test_at_no_args_on_unsized_node_is_its_point(sc):
    p = Path()
    handle = p.move_to().xy(5, 7)
    pos = handle.at()
    assert pos.x == handle._get_attr("x")
    assert pos.y == handle._get_attr("y")


def test_at_named_anchor_top_left(sc):
    r = Rect().size(40, 20).xy(10, 10)
    p = r.at("top_left")
    assert p.x == r._get_attr("x")
    assert p.y == r._get_attr("y")


def test_at_named_anchor_bottom_right(sc):
    r = Rect().size(40, 20).xy(10, 10)
    p = r.at("bottom_right")
    expected_x = r._get_attr("x") + r._get_attr("width") * 1.0
    expected_y = r._get_attr("y") + r._get_attr("height") * 1.0
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)


def test_at_bare_fraction_raises(sc):
    r = Rect().size(40, 20)
    with pytest.raises(TypeError):
        r.at(0.5)


def test_at_named_anchor_with_y_raises(sc):
    r = Rect().size(40, 20)
    with pytest.raises(TypeError):
        r.at("center", 0.5)


def test_at_unknown_anchor_rejected(sc):
    r = Rect().size(40, 20)
    with pytest.raises(Exception):
        r.at("bogus")


def test_at_center_on_text_uses_measured_extent(sc):
    t = Text()
    t.span("hello")
    p = t.at()
    expected_x = t._get_attr("x") + Call.default_width(t) * 0.5
    expected_y = t._get_attr("y") + Call.default_height(t) * 0.5
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)


def test_at_top_left_on_text_is_unchanged(sc):
    t = Text()
    t.span("hello")
    p = t.at("top_left")
    assert p.x == t._get_attr("x")
    assert p.y == t._get_attr("y")


def test_at_center_on_span_uses_measured_extent(sc):
    span = Text().span("hello")
    p = span.at()
    expected_x = span._get_attr("x") + Call.default_width(span) * 0.5
    expected_y = span._get_attr("y") + Call.default_height(span) * 0.5
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)


def test_at_top_left_on_span_is_unchanged(sc):
    span = Text().span("hello")
    p = span.at("top_left")
    assert p.x == span._get_attr("x")
    assert p.y == span._get_attr("y")


def test_at_bare_fraction_raises_on_span(sc):
    span = Text().span("hello")
    with pytest.raises(TypeError):
        span.at(0.5)


def test_at_unknown_anchor_rejected_on_span(sc):
    span = Text().span("hello")
    with pytest.raises(Exception):
        span.at("bogus")


def test_at_center_on_group_uses_measured_extent(sc):
    group = Text().group()
    group.span("hello")
    p = group.at()
    expected_x = group._get_attr("x") + Call.default_width(group) * 0.5
    expected_y = group._get_attr("y") + Call.default_height(group) * 0.5
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)


def test_at_top_left_on_group_is_unchanged(sc):
    group = Text().group()
    group.span("hello")
    p = group.at("top_left")
    assert p.x == group._get_attr("x")
    assert p.y == group._get_attr("y")


def test_at_center_on_scene_uses_dimensions(sc):
    # Scene is SizeMixin, so _effective_width/_height read its real (always
    # explicitly seeded) width/height, not Call.default_width - unlike
    # TextSpan/TextGroup above, which have no SizeMixin of their own.
    p = sc.at()
    expected_x = sc._get_attr("x") + sc._get_attr("width") * 0.5
    expected_y = sc._get_attr("y") + sc._get_attr("height") * 0.5
    assert repr(p.x) == repr(expected_x)
    assert repr(p.y) == repr(expected_y)
    # Scene has no parent - at()'s Position must use the Scene itself as the
    # coordinate frame (map_x/map_y's -1 sentinel means exactly this), not
    # `None`, or it can't be embedded in another node's expression at all.
    assert p.node is sc


def test_at_bare_fraction_raises_on_scene(sc):
    with pytest.raises(TypeError):
        sc.at(0.5)


def test_scene_at_center_exports_without_circular_reference(sc):
    # Regression test for the gap PositionQueryMixin's frame fix and
    # serializer.py's Scene guard close together: embedding a Scene-derived
    # Position (here, via .pos()) used to reach Serializer.add_node(scene),
    # which re-serializes the Scene mid-walk and produces a circular
    # reference that json.dump rejects.
    from fairyflow.serializer import create_export
    import json

    r = Rect().size(10, 10)
    r.pos(sc.at("center"))
    exported = create_export(0, sc)
    json.dumps(exported)  # must not raise


def test_next_to_scene_exports_without_circular_reference(sc):
    # Regression test mirroring the at()-on-Scene case above: next_to()'s own
    # Position(node._parent, ...) construction needs the same self-or-parent
    # frame fix when `node` is the bare Scene (node._parent is None there).
    from fairyflow.serializer import create_export
    import json

    r = Rect().size(10, 10)
    r.next_to(sc, "above", gap=-20)
    exported = create_export(0, sc)
    json.dumps(exported)  # must not raise


# ── Visual placement (golden image) ─────────────────────────────────────────


def test_next_to_right(test_scene):
    with test_scene:
        with Group().size(60, 40):
            a = Rect().size(20, 20).color("steelblue").xy(10, 10)
            b = Rect().size(10, 10).color("tomato")
            b.next_to(a, "right", gap=5)


def test_next_to_below(test_scene):
    with test_scene:
        with Group().size(60, 40):
            a = Rect().size(20, 20).color("steelblue").xy(10, 10)
            b = Rect().size(10, 10).color("tomato")
            b.next_to(a, "below", gap=5, align=0)


def test_next_to_across_groups(test_scene):
    with test_scene:
        with Group().size(30, 30).xy(0, 0):
            a = Rect().size(20, 20).color("steelblue").xy(5, 5)
        with Group().size(30, 30).xy(30, 0):
            b = Rect().size(10, 10).color("tomato")
            b.next_to(a, "right", gap=5)


def test_next_to_top_level(test_scene):
    # Both a and b are direct Scene children, no Group at all - the Scene
    # has a reserved wire id (SCENE_NODE_ID, position.py) precisely so this
    # works instead of hitting a circular-reference error at export time.
    with test_scene:
        a = Rect().size(20, 20).color("steelblue").xy(10, 10)
        b = Rect().size(10, 10).color("tomato")
        b.next_to(a, "right", gap=5)


def test_next_to_top_level_mixed_with_group(test_scene):
    # a is a direct Scene child; b lives inside a Group - map_x/map_y must
    # walk b's ancestor chain up while treating a's side as the root frame.
    with test_scene:
        a = Rect().size(20, 20).color("steelblue").xy(10, 10)
        with Group().size(10, 10).xy(0, 25):
            b = Rect().size(10, 10).color("tomato")
            b.next_to(a, "right", gap=5)


def test_next_to_text(test_scene):
    # Text has no SizeMixin - next_to() must use the engine's measured extent
    # (auto_w/auto_h) instead of treating the label as zero-width, or the
    # rect would overlap the rendered text instead of clearing it.
    with test_scene:
        with Group().size(60, 40):
            t = Text().xy(2, 12)
            t.span("Hi").font_size(14)
            r = Rect().size(8, 8).color("tomato")
            r.next_to(t, "right", gap=3)


def test_next_to_span(test_scene):
    # next_to()'s node param now accepts PositionQueryMixin targets, not just
    # full PositionMixin ones - a TextSpan (query-only) must work directly,
    # not just its enclosing Text.
    with test_scene:
        with Group().size(60, 40):
            t = Text().xy(2, 12)
            span = t.span("Hi").font_size(14)
            r = Rect().size(8, 8).color("tomato")
            r.next_to(span, "right", gap=3)


def test_next_to_scene(test_scene):
    # node may be the bare Scene itself - "next to" the whole canvas is
    # naturally partly off-canvas for right/below, so use above/gap<0 to land
    # a visible rect near the top edge instead.
    with test_scene:
        r = Rect().size(8, 8).color("tomato")
        r.next_to(test_scene, "above", gap=-20)
