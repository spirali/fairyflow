class InheritedValue:
    """`_ATTR_DEFAULTS` sentinel: this attribute defaults to the *parent's*
    resolved value when unset"""


INHERITED_VALUE = InheritedValue()


class DefaultMarker:
    """Sentinel accepted by structured setters (`xy()`, and later `size()`) to mean
    "reset this axis/field to its layout default" — as opposed to `None`, which means
    "leave it untouched" (api-v2-proposal.md §4.1)."""

    __slots__ = ()

    def __repr__(self):
        return "DEFAULT"


DEFAULT = DefaultMarker()
