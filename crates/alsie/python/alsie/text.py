
from .exprs import expr_default_x, expr_default_y
from .position import Position

from .items import Node, NodeWithChildren, PositionMixin, StyleMixin, make_node


class TextStyleMixin(StyleMixin):

    def _init_text_style(self):
        self._add_attr("font", "sans-serif")
        self._add_attr("font_size", 16)
        self._add_attr("stroke_color", None)
        self._add_attr("italic", False)
        self._init_style()

    def italic(self, value: bool):
        self._set_attr("italic", value)
        return self

    def font(self, value: str):
        self._set_attr("font", value)
        return self

    def font_size(self, value: float):
        self._set_attr("font_size", value)
        return self
    

class TextSpan(Node, TextStyleMixin):

    kind = "t_span"

    def __init__(self, parent, frame, text):
        super().__init__(parent, frame)
        self._init_text_style()
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)

    def get_pos(self):
        return Position(self._parent, expr_default_x(self), expr_default_y(self))


class TextGroup(NodeWithChildren, TextStyleMixin):

    kind = "t_group"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_text_style()

    def span(self, text):
        span = TextSpan(self, self._frame, text)
        self._children.append(span)
        return span
    
    def group(self):
        group = TextGroup(self, self._frame)
        self._children.append(group)
        return group    


class Text(NodeWithChildren, PositionMixin, TextStyleMixin):

    kind = "text"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_position(0, 0)
        self._init_text_style()


    def group(self):
        group = TextGroup(self, self._frame)
        self._children.append(group)
        return group
    

    def span(self, text):
        span = TextSpan(self, self._frame, text)
        self._children.append(span)
        return span


def text(*, frame=None):
    return make_node(Text, frame)