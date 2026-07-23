from .exprs import Call, Expr, to_expr


class InheritedValue:
    """`_ATTR_DEFAULTS` sentinel: this attribute defaults to the *parent's*
    resolved value when unset"""


INHERITED_VALUE = InheritedValue()


class DefaultMarker:
    """Sentinel accepted by structured setters (`xy()`, `size()`, ...) to mean
    "reset this axis/field to its layout default" — as opposed to `None`, which means
    "leave it untouched" (api-v2-proposal.md §4.1)."""

    __slots__ = ()

    def __repr__(self):
        return "DEFAULT"


DEFAULT = DefaultMarker()


class OmittedMarker:
    """Sentinel default for a structured setter's parameter whose *value type
    itself* already includes `None` as a real, meaningful setting (e.g.
    `ColorLike` includes `None` for "no color" - `color(None)` and
    `stroke(None)` disable a fill/stroke, matching `Color.parse(None) ->
    ""`). `None` can't double as "leave untouched" there the way it does for
    `xy()`/`size()`, since it's already spoken for - `OMITTED` is the
    "not passed" default instead, so an explicit `None` still reaches the
    setter and is honored."""

    __slots__ = ()

    def __repr__(self):
        return "OMITTED"


OMITTED = OmittedMarker()


class RelValue(Expr):
    """Value marker for `rel(f)`: `f` × the parent's corresponding dimension,
    resolved against the parent group when a setter consumes it. Composes with arithmetic like any `Expr`
    (`rel(1) - 20`) — those operators just wrap it in a `Call` node;
    `resolve_rel` walks the resulting tree to substitute it in place."""

    def __init__(self, factor):
        self.factor = to_expr(factor)

    def serialize_expr(self):
        raise RuntimeError(
            "rel() was never resolved against a parent — only "
            "width()/height()/size()/x()/y()/xy() accept it"
        )


def rel(factor):
    return RelValue(factor)


def resolve_rel(value, node, dim):
    """Replace any `RelValue` node in `value`'s expression tree with
    `Call.mul(parent._get_attr(dim), factor)`, where `parent` is `node`'s
    parent group. `dim` is `"width"` or `"height"`. Values with no `rel()`
    marker (the common case) pass through unchanged and `node.parent_group()`
    is never called — important because it's undefined (`None`) for a root
    `Scene`, and plain literal sizes/positions must keep working there."""
    if isinstance(value, RelValue):
        return Call.mul(node.parent_group()._get_attr(dim), value.factor)
    if isinstance(value, Call):
        return Call(value.op, *(resolve_rel(a, node, dim) for a in value.args))
    return value
