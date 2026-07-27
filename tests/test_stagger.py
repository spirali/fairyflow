import pytest

from fairyflow import Par, Rect, Scene, Seq, anim, get_frame, time_to_frames


@pytest.fixture
def sc():
    s = Scene(100, 100)
    with s:
        yield s


def _attr_frames(node, attr):
    av = node._attrs[attr]
    return av.values, av.transitions


def test_stagger_offsets_bare_calls(sc):
    objs = [Rect().size(10, 10) for _ in range(3)]
    base = get_frame()
    with Par(stagger=0.1), anim(0.2):
        for r in objs:
            r.x(10)
    stagger_frames = time_to_frames(0.1)
    dur_frames = time_to_frames(0.2)

    for i, r in enumerate(objs):
        values, transitions = _attr_frames(r, "x")
        start = base + i * stagger_frames
        end = start + dur_frames
        assert end in values and values[end] == 10
        assert transitions[end] == "in_out"

    assert get_frame() == base + 2 * stagger_frames + dur_frames


def test_stagger_with_fade_in(sc):
    objs = [Rect().size(10, 10) for _ in range(3)]
    base = get_frame()
    with Par(stagger=0.1), anim(0.3):
        for r in objs:
            r.fade_in()
    stagger_frames = time_to_frames(0.1)
    dur_frames = time_to_frames(0.3)

    for i, r in enumerate(objs):
        values, transitions = _attr_frames(r, "alpha")
        start = base + i * stagger_frames
        end = start + dur_frames
        assert values[end] == 1
        assert transitions[end] == "in_out"

    assert get_frame() == base + 2 * stagger_frames + dur_frames


def test_seq_rejects_stagger():
    with pytest.raises(TypeError):
        Seq(stagger=0.1)


def test_par_without_stagger_is_unaffected(sc):
    a = Rect().size(10, 10)
    b = Rect().size(10, 10)
    base = get_frame()
    with Par():
        a.x(10, dur=0.5)
        b.y(10, dur=0.5)
    assert get_frame() == base + time_to_frames(0.5)


def test_nested_staggering_pars(sc):
    r00 = Rect().size(10, 10)
    r01 = Rect().size(10, 10)
    r10 = Rect().size(10, 10)
    r11 = Rect().size(10, 10)
    base = get_frame()
    outer_stagger = time_to_frames(0.2)
    inner_stagger = time_to_frames(0.05)
    dur = time_to_frames(0.1)

    with Par(stagger=0.2), anim(0.1):
        with Par(stagger=0.05):
            r00.x(10)
            r01.y(10)
        with Par(stagger=0.05):
            r10.x(10)
            r11.y(10)

    outer_unit0 = base
    outer_unit1 = base + outer_stagger

    values, _ = _attr_frames(r00, "x")
    assert values[outer_unit0 + dur] == 10
    values, _ = _attr_frames(r01, "y")
    assert values[outer_unit0 + inner_stagger + dur] == 10
    values, _ = _attr_frames(r10, "x")
    assert values[outer_unit1 + dur] == 10
    values, _ = _attr_frames(r11, "y")
    assert values[outer_unit1 + inner_stagger + dur] == 10

    assert get_frame() == outer_unit1 + inner_stagger + dur
