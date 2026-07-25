from typing import Self

from tinycss2.color3 import parse_color

from .exprs import Expr


class Color:
    def __init__(self, value):
        if parse_color(value) is None:
            raise ValueError(f"Invalid CSS color: {value!r}")
        self.value = value

    @staticmethod
    def parse(value: str | None) -> Self | None:
        if value is None:
            return ""
        if isinstance(value, str):
            return Color(value)
        if isinstance(value, Expr):
            return value
        raise ValueError(f"Invalid color: {value!r}")

    def __repr__(self):
        return f"<Color {self.value}>"


class Gradient(Expr):
    """A linear gradient fill — see `gradient()`. Stop colors are animatable as a
    single whole-gradient value with `dur=`; the angle is static geometry."""

    def __init__(self, stops: list[tuple[float, Color]], angle: float):
        self.stops = stops
        self.angle = angle

    def serialize_expr(self):
        return [
            "gradient",
            [[offset, color.value] for offset, color in self.stops],
            self.angle,
        ]

    def __repr__(self):
        return f"<Gradient {self.stops} angle={self.angle}>"


def gradient(*stops, angle: float = 0) -> Gradient:
    """Build a linear gradient fill.

    Args:
        stops: Either plain colors (evenly spaced across the gradient) or
            `(offset, color)` pairs for explicit stop positions.
        angle: Gradient direction in degrees. `0` points from bottom to top,
            increasing clockwise (matches CSS `linear-gradient()`).

    Returns:
        A `Gradient`, passable to `.fill()`.
    """
    if not stops:
        raise ValueError("gradient() needs at least one stop")
    if isinstance(stops[0], tuple):
        parsed = [(float(offset), Color.parse(color)) for offset, color in stops]
    else:
        n = len(stops)
        parsed = [
            (i / (n - 1) if n > 1 else 0.0, Color.parse(color))
            for i, color in enumerate(stops)
        ]
    return Gradient(parsed, float(angle))
