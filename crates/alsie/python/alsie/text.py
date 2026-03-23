
from .items import Node, NodeWithChildren, StyleMixin, make_node


class TextStyleMixin(StyleMixin):

    def _init_text_style(self):
        self._add_attr("font", "sans-serif")
        self._add_attr("stroke_color", None)
        self._add_attr("italic", False)
        self._init_style()

    def italic(self, value: bool):
        self._set_attr("italic", value)
        return self

    def font(self, value: str):
        self._set_attr("font", value)
        return self
    

class Span(Node, TextStyleMixin):

    kind = "span"

    def __init__(self, parent, frame, text):
        super().__init__(parent, frame)
        self._init_text_style()
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)


class Line(NodeWithChildren, TextStyleMixin):

    kind = "line"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_text_style()

    def span(self, text):
        span = Span(self, self._frame, text)
        self._children.append(span)
        return span


class Text(NodeWithChildren, TextStyleMixin):

    kind = "text"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_text_style()

    def line(self):
        line = Line(self, self._frame)
        self._children.append(line)
        return line


def text(*, frame=None):
    return make_node(Text, frame)