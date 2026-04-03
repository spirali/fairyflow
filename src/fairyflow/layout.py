from .exprs import Call
from .avalue import AnimatedValue


class LayoutBase:
    pass


class CenteringLayout(LayoutBase):
    def serialize(self, serializer):
        return {"kind": "center"}


# Singleton; centering layout is not parametrizable
CENTERING_LAYOUT = CenteringLayout()


class ColumnLayout(LayoutBase):
    def __init__(self, frame, gap, align):
        self.gap = AnimatedValue(gap, frame)
        self.align = AnimatedValue(align, frame)

    def serialize(self, serializer):
        from .serializer import serialize_expr

        return {
            "kind": "column",
            "gap": serialize_expr(self.gap),
            "align": serialize_expr(self.align),
        }


class RowLayout(LayoutBase):
    def __init__(self, frame, gap, align):
        self.gap = AnimatedValue(gap, frame)
        self.align = AnimatedValue(align, frame)

    def serialize(self, serializer):
        from .serializer import serialize_expr

        return {
            "kind": "row",
            "gap": serialize_expr(self.gap),
            "align": serialize_expr(self.align),
        }
