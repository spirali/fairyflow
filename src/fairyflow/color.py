from typing import Self
from tinycss2.color3 import parse_color


class Color:
    def __init__(self, value):
        if parse_color(value) is None:
            raise ValueError(f"Invalid CSS color: {value!r}")
        self.value = value

    @staticmethod
    def parse(value: str | None) -> Self | None:
        if value is None:
            return value
        if isinstance(value, str):
            return Color(value)
        from .exprs import Expr

        if isinstance(value, Expr):
            return value
        raise ValueError(f"Invalid color: {value!r}")

    def __repr__(self):
        return f"<Color {self.value}>"
