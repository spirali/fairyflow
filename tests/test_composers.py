import pytest
from fairyflow import Group, Par, Rect, Scene, Seq, anim, get_frame, time_to_frames


@pytest.fixture
def sc():
    s = Scene(100, 100)
    with s:
        yield s


def _transitions(node, attr):
    return node._attrs[attr].transitions


def test_anim_block_default_is_sequential_at_top_level(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(0.5):
        r.x(10)
        r.y(10)
    assert get_frame() == base + 2 * time_to_frames(0.5)


def test_anim_block_stays_parallel_inside_par(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with Par(), anim(0.5):
        r.x(10)
        r.y(10)
    # anim() is composition-transparent: Par still makes both calls parallel.
    assert get_frame() == base + time_to_frames(0.5)


def test_explicit_call_dur_overrides_anim_block(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(1.5):
        r.x(10, dur=0.2)
    assert get_frame() == base + time_to_frames(0.2)


def test_innermost_anim_block_wins_when_nested(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(1.5):
        with anim(0.3):
            r.x(10)
            r.y(10)
    # the inner anim(0.3) wins; the outer anim(1.5) never gets a chance to apply.
    assert get_frame() == base + 2 * time_to_frames(0.3)


def test_fade_in_picks_up_ambient_anim_block(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(0.3):
        r.fade_in()
    # fade_in() no longer hardcodes dur=1 - an enclosing anim() block now
    # supplies its duration, like any other setter.
    assert get_frame() == base + time_to_frames(0.3)


def test_hide_picks_up_ambient_anim_block(sc):
    with Group().size(40, 30) as g:
        Rect().size(40, 30)
    base = get_frame()
    with anim(0.3):
        g.hide("right")
    assert get_frame() == base + time_to_frames(0.3)


def test_fade_in_bare_is_instant(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.fade_in()
    # No explicit dur and no ambient anim() block: instant, like every
    # other setter - fade_in/fade_out/hide/reveal no longer default to 1s.
    assert get_frame() == base


def test_hide_bare_is_instant(sc):
    with Group().size(40, 30) as g:
        Rect().size(40, 30)
    base = get_frame()
    g.hide("right")
    assert get_frame() == base


def test_anim_proxy_is_parallel(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.anim(0.8).x(10).y(10)
    assert get_frame() == base + time_to_frames(0.8)


def test_anim_proxy_matches_equivalent_par(sc):
    a = Rect().size(10, 10)
    b = Rect().size(10, 10)
    base = get_frame()
    a.anim(0.8).x(10).y(10)
    proxy_end = get_frame()

    with Par():
        b.x(10, dur=0.8)
        b.y(10, dur=0.8)
    par_end = get_frame()

    assert proxy_end - base == par_end - proxy_end


def test_anim_proxy_is_not_sequential(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.anim(0.8).x(10).y(10)
    assert get_frame() < base + 2 * time_to_frames(0.8)


def test_anim_proxy_per_call_override(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.anim(0.8).x(10, dur=0.2).y(10)
    # y() still uses the proxy's 0.8; x()'s override doesn't shrink the
    # overall (parallel) duration below the longest branch.
    assert get_frame() == base + time_to_frames(0.8)


def test_anim_block_ease_only_with_call_site_dur(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(ease="in_out"):
        r.x(10, dur=0.5)
    end = base + time_to_frames(0.5)
    assert get_frame() == end
    assert _transitions(r, "x")[end] == "in_out"


def test_anim_block_ease_reaches_through_nested_dur_only_block(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(ease="in_out"), anim(0.3):
        r.x(10)
    end = base + time_to_frames(0.3)
    assert get_frame() == end
    assert _transitions(r, "x")[end] == "in_out"


def test_call_site_ease_overrides_anim_block_ease(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(ease="in_out"):
        r.x(10, dur=0.5, ease="out_back")
    end = base + time_to_frames(0.5)
    assert _transitions(r, "x")[end] == "out_back"


def test_anim_block_ease_without_any_dur_is_instant(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    with anim(ease="in_out"):
        r.x(10)
    assert get_frame() == base
    assert _transitions(r, "x")[base] == "S"


def test_anim_proxy_ease_only_with_call_site_dur(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.anim(ease="in_out").x(10, dur=0.5).y(10, dur=0.5)
    end = base + time_to_frames(0.5)
    assert get_frame() == end
    assert _transitions(r, "x")[end] == "in_out"
    assert _transitions(r, "y")[end] == "in_out"


def test_anim_proxy_call_site_ease_overrides(sc):
    r = Rect().size(10, 10)
    base = get_frame()
    r.anim(ease="in_out").x(10, dur=0.5, ease="out_back")
    end = base + time_to_frames(0.5)
    assert _transitions(r, "x")[end] == "out_back"
