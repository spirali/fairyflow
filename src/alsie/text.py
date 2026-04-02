from .ctxvars import get_frame
from .exprs import expr_default_x, expr_default_y
from .position import Position

from .nodes import (
    Node,
    NodeWithChildren,
    PositionMixin,
    StyleMixin,
    ZLevelMixin,
    make_node,
)


class TextStyleMixin(StyleMixin):

    def _init_text_style(self):
        self._init_style()
        self._add_attr("font", "sans-serif")
        self._add_attr("font_size", 16)
        self._add_attr("italic", False)

    def _init_text_style_from_parent(self):        
        self._init_style_from_parent()
        self._add_from_parent("font")
        self._add_from_parent("font_size")
        self._add_from_parent("italic")        

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
        self._init_text_style_from_parent()
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)

    def get_pos(self):
        return Position(self._parent, expr_default_x(self), expr_default_y(self))


class TextGroup(NodeWithChildren, TextStyleMixin):
    kind = "t_group"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_text_style_from_parent()

    def span(self, text):
        span = TextSpan(self, get_frame(), text)
        self._children.append(span)
        return span

    def group(self):
        group = TextGroup(self, self._frame)
        self._children.append(group)
        return group


class Text(NodeWithChildren, PositionMixin, TextStyleMixin, ZLevelMixin):
    kind = "text"

    def __init__(self, parent, frame, sh_language, sh_theme):
        super().__init__(parent, frame)
        self._init_position()
        self._init_text_style()
        self._init_z()
        self.sh_language = sh_language
        self.sh_theme = sh_theme

    def group(self):
        group = TextGroup(self, get_frame())
        self._children.append(group)
        return group

    def span(self, text):
        span = TextSpan(self, get_frame(), text)
        self._children.append(span)
        return span
    
    def serialize(self, serializer):
        result = super().serialize(serializer)
        if self.sh_language is not None:
            result["sh_language"] = self.sh_language
        if self.sh_theme is not None:
            result["sh_theme"] = self.sh_theme
    

def text():
    return make_node(Text)
