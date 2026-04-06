from tinycss2.color3 import parse_color


class Color:
    def __init__(self, value):
        if parse_color(value) is None:
            raise ValueError(f"Invalid CSS color: {value!r}")
        self.value = value

    @staticmethod
    def parse(value):        
        from .exprs import Expr
        if isinstance(value, Expr):
            return value
        return Color(value)

    def __repr__(self):
        return f"<Color {self.value}>"
