from dataclasses import dataclass, field
from beartype import beartype
from typing import Union

from .types import StringLike, BoolLike, FloatLike
from .exprs import Call
from .position import Position

from .nodes import (
    Node,
    NodeWithChildren,
    PositionMixin,
    StyleMixin,
    ZLevelMixin,
)

@beartype
class TextStyleMixin(StyleMixin):

    def _init_text_style(self):
        self._init_style()
        self._add_attr("font", "sans-serif")
        self._add_attr("font_size", 16)
        self._add_attr("font_weight", 400)
        self._add_attr("italic", False)

    def _init_text_style_from_parent(self):        
        self._init_style_from_parent()
        self._add_from_parent("font")
        self._add_from_parent("font_size")
        self._add_from_parent("font_weight")
        self._add_from_parent("italic")        

    def italic(self, value: BoolLike):
        self._set_attr("italic", value)
        return self

    def font(self, value: StringLike):
        self._set_attr("font", value)
        return self

    def font_size(self, value: FloatLike, transition=None):
        self._set_attr("font_size", value, transition)
        return self
    
    def font_weight(self, value: FloatLike, transition=None):
        self._set_attr("font_weight", value, transition)
        return self    
    
    def bold(self):
        return self.font_weight(800)


@beartype
class TextSpan(Node, TextStyleMixin):
    kind = "t_span"

    def __init__(self, text: StringLike):
        super().__init__()
        self._init_text_style_from_parent()
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)

    def get_pos(self):
        return Position(self._parent, Call.default_x(self), Call.default_y(self))

@beartype
class TextGroup(NodeWithChildren, TextStyleMixin):
    kind = "t_group"

    def __init__(self):
        super().__init__()
        self._init_text_style_from_parent()

    def span(self, text: StringLike):
        span = TextSpan(self, text)
        self._children.append(span)
        return span

    def group(self):
        group = TextGroup(self)
        self._children.append(group)
        return group


class Text(NodeWithChildren, PositionMixin, TextStyleMixin, ZLevelMixin):
    kind = "text"

    def __init__(self):
        super().__init__()
        self._init_position()
        self._init_text_style()
        self._init_z()
        self.color("black")        
        self.sh_language = None
        self.sh_theme = None

    def sh(self, language, *, theme=None):
        """
        Enable Syntax highligting
        """
        self.sh_language = language
        self.sh_theme = theme
        return self

    def group(self) -> TextGroup:
        group = TextGroup(self)
        self._children.append(group)
        return group

    def span(self, text: StringLike) -> TextSpan:
        span = TextSpan(text)
        self._children.append(span)
        return span
    
    def serialize(self, serializer):
        result = super().serialize(serializer)
        if self.sh_language is not None:
            result["sh_language"] = self.sh_language
        if self.sh_theme is not None:
            result["sh_theme"] = self.sh_theme
        return result
    

@dataclass
class _TagNode:
    name: str
    children: list[Union[str, "_TagNode"]] = field(default_factory=list)


def _parse_content(src, pos, open_d, close_d, stop_tag=None):
    nodes = []
    n = len(src)
    buf = []

    while pos < n:
        idx = src.find(open_d, pos)
        if idx == -1:
            buf.append(src[pos:])
            pos = n
            break

        if idx > pos:
            buf.append(src[pos:idx])

        after_open = idx + len(open_d)
        if after_open < n and src[after_open] == '/':
            close_idx = src.find(close_d, after_open)
            if close_idx == -1:
                raise ValueError(f"Unclosed closing tag at position {idx}")
            tag_name = src[after_open + 1:close_idx]
            if stop_tag is not None and tag_name == stop_tag:
                if buf:
                    nodes.append(''.join(buf))
                return nodes, close_idx + len(close_d)
            raise ValueError(f"Unexpected closing tag '{tag_name}' at position {idx}")
        else:
            close_idx = src.find(close_d, after_open)
            if close_idx == -1:
                raise ValueError(f"Unclosed tag at position {idx}")
            tag_name = src[after_open:close_idx]
            if buf:
                nodes.append(''.join(buf))
                buf = []
            pos = close_idx + len(close_d)
            children, pos = _parse_content(src, pos, open_d, close_d, stop_tag=tag_name)
            nodes.append(_TagNode(tag_name, children))

    if buf:
        nodes.append(''.join(buf))

    if stop_tag is not None:
        raise ValueError(f"Missing closing tag '{stop_tag}'")

    return nodes, pos


def _plain_text(nodes):
    if any(isinstance(n, _TagNode) for n in nodes):
        return None
    return ''.join(nodes)


def _add_lines(parent, text_str, name=None):
    for line in text_str.split('\n'):
        if line or name is None:
            s = parent.span(line)
            if name is not None:
                s.name(name)


def _add_node_to(parent, node):
    if isinstance(node, str):
        _add_lines(parent, node)
    else:
        _add_tag_to(parent, node)


def _add_tag_to(parent, node):
    plain = _plain_text(node.children)
    if plain is not None:
        _add_lines(parent, plain, name=node.name)
    else:
        g = parent.group().name(node.name)
        for child in node.children:
            _add_node_to(g, child)


@beartype
def stext(input_text: str, *, strip: bool =True, delimiters: str = "<>"):
    """
    Parse input_text and create a text() node from it.

    Plain text is split on newlines into spans. Named tags become spans (leaf)
    or groups (nested), with the tag name assigned via .name(). Multiple
    top-level items are wrapped in an anonymous group.
    """
    if strip:
        input_text = input_text.strip()

    open_d = delimiters[0]
    close_d = delimiters[1]
    nodes, _ = _parse_content(input_text, 0, open_d, close_d)

    t = text()

    if not nodes:
        return t

    plain = _plain_text(nodes)
    if plain is not None:
        _add_lines(t, plain)
        return t

    if len(nodes) == 1:
        _add_tag_to(t, nodes[0])
    else:
        g = t.group()
        for node in nodes:
            _add_node_to(g, node)

    return t
