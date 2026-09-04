from .avalue import AnimatedValue


class LayoutBase:
    pass


class CenteringLayout(LayoutBase):
    def serialize(self, serializer):
        return {"kind": "center"}


# Singleton; centering layout is not parametrizable
CENTERING_LAYOUT = CenteringLayout()


class ColumnLayout(LayoutBase):
    def __init__(self, frame, gap, align, reserve, justify):
        self.gap = AnimatedValue(gap, frame)
        self.align = AnimatedValue(align, frame)
        self.justify = AnimatedValue(justify, frame)
        self.reserve = reserve

    def serialize(self, serializer):
        from .serializer import serialize_expr

        return {
            "kind": "column",
            "gap": serialize_expr(self.gap),
            "align": serialize_expr(self.align),
            "justify": serialize_expr(self.justify),
            "reserve": self.reserve,
        }


class RowLayout(LayoutBase):
    def __init__(self, frame, gap, align, reserve, justify):
        self.gap = AnimatedValue(gap, frame)
        self.align = AnimatedValue(align, frame)
        self.justify = AnimatedValue(justify, frame)
        self.reserve = reserve

    def serialize(self, serializer):
        from .serializer import serialize_expr

        return {
            "kind": "row",
            "gap": serialize_expr(self.gap),
            "align": serialize_expr(self.align),
            "justify": serialize_expr(self.justify),
            "reserve": self.reserve,
        }


class GridLayout(LayoutBase):
    def __init__(self, frame, cols, gap_x, gap_y, reserve):
        self.cols = cols
        self.gap_x = AnimatedValue(gap_x, frame)
        self.gap_y = AnimatedValue(gap_y, frame)
        self.reserve = reserve

    def serialize(self, serializer):
        from .serializer import serialize_expr

        return {
            "kind": "grid",
            "cols": self.cols,
            "gap_x": serialize_expr(self.gap_x),
            "gap_y": serialize_expr(self.gap_y),
            "reserve": self.reserve,
        }
