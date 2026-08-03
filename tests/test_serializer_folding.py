"""Tests for constant folding at serialization time (`serializer._try_fold`)."""

from fairyflow import Arrow, Rect, Scene
from fairyflow.exprs import Call
from fairyflow.serializer import _try_fold, create_export, serialize_expr


def test_add_folds():
    assert _try_fold(Call.add(2, 3)) == 5


def test_mul_folds():
    assert _try_fold(Call.mul(2.0, 3)) == 6.0


def test_nested_folds_recursively():
    assert _try_fold(Call.mul(Call.add(1, 2), 4)) == 12


def test_norm_folds():
    assert _try_fold(Call.norm(3, 4)) == 0.6


def test_neg_folds():
    assert _try_fold(Call.neg(5)) == -5


def test_neg_operator_folds():
    """The `__neg__` overload (`-expr`) must produce the same result as the
    `Call.neg` static factory, not just work when called directly."""
    assert _try_fold(-Call.add(2, 3)) == -5


def test_rsub_operator_folds():
    assert _try_fold(260 - Call.add(2, 3)) == 255


def test_radd_operator_folds():
    assert _try_fold(5 + Call.add(2, 3)) == 10


def test_rmul_operator_folds():
    assert _try_fold(2 * Call.add(2, 3)) == 10


def test_rtruediv_operator_folds():
    assert _try_fold(10 / Call.add(3, 2)) == 2


def test_neg_of_unfoldable_stays_structural():
    s = Scene(100, 100)
    with s:
        r = Rect().size(50, 50)
        r._add_attr("test_neg", Call.neg(Call.auto_x(r)))
    exported = create_export(0, s)
    node = next(n for n in exported["nodes"] if "test_neg" in n)
    assert node["test_neg"][0] == "neg"
    assert node["test_neg"][1][0] == "auto_x"


def test_norm_zero_vector_uses_zero_guard():
    """Matches Rust's `d < 0.0001 -> 0.0` guard (`eval.rs::FloatCall::Norm`)
    exactly - not an approximation."""
    assert _try_fold(Call.norm(0, 0)) == 0.0


def test_div_by_zero_uses_zero_guard():
    """Matches Rust's `vb.abs() < 0.000001 -> 0.0` guard exactly."""
    assert _try_fold(Call.div(1, 0)) == 0.0


def test_unanimated_attribute_folds_through_a_call():
    s = Scene(100, 100)
    with s:
        r = Rect().xy(10, 20)
    av = r._get_attr("x")
    assert _try_fold(Call.add(av, 5)) == 15


def test_animated_attribute_does_not_fold():
    """The user's stated nuance: an attribute reference only becomes
    foldable when it was never animated. A `dur=`-animated attribute must
    keep serializing structurally (the keyframe table), not collapse to
    whatever its first value happened to be."""
    s = Scene(100, 100)
    with s:
        r = Rect().xy(10, 20)
        r.x(15, dur=1)
    av = r._get_attr("x")
    assert _try_fold(Call.add(av, 5)) is None


def test_mixed_foldable_and_nonfoldable_args_partially_folds():
    """A `Call` mixing a foldable arg and a live one (`auto_x`, which
    depends on layout and can never be folded in Python) does not fold as
    a whole, but the foldable side still serializes as a plain literal via
    ordinary recursion, not the root-or-nothing shortcut."""
    s = Scene(100, 100)
    with s:
        r = Rect().size(50, 50)
        r._add_attr("test_mixed", Call.add(5, Call.auto_x(r)))
    exported = create_export(0, s)
    node = next(n for n in exported["nodes"] if "test_mixed" in n)
    assert node["test_mixed"][0] == "+"
    assert node["test_mixed"][1] == 5
    assert node["test_mixed"][2][0] == "auto_x"


def test_arrow_gap_with_literal_endpoints_folds_to_plain_numbers():
    s = Scene(100, 100)
    with s:
        Arrow((0, 0), (30, 40), gap=5)
    node = create_export(0, s)["nodes"][2]
    assert node["x"] == 27.0
    assert node["y"] == 36.0


def test_arrow_gap_with_live_position_endpoints_does_not_fold():
    """Folding must stop exactly at the live-tracking boundary - a
    `Position`-based endpoint compiles to `map_x`/`map_y`, which depends on
    the referenced node's live transform and can never be resolved in
    Python."""
    s = Scene(100, 100)
    with s:
        a = Rect().xy(10, 10).size(40, 40)
        b = Rect().xy(200, 100).size(40, 40)
        Arrow(a.at("right"), b.at("left"), gap=5)
    node = create_export(0, s)["nodes"][4]  # the "end" line command
    assert node["kind"] == "line"
    # unfoldable: stays a structural expression, not a plain number
    assert isinstance(node["x"], list)
    assert isinstance(node["y"], list)
    assert "map_x" in str(node["x"])
    assert "map_y" in str(node["y"])


def test_serialize_expr_zero_is_not_mistaken_for_unfoldable():
    """`_try_fold` returning a legitimate `0` must still short-circuit
    `serialize_expr` - a falsy-but-valid fold result is not `None`."""
    assert serialize_expr(Call.sub(5, 5)) == 0
