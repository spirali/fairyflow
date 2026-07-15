import re
from dataclasses import dataclass, field
from beartype import beartype
from typing import Union

from .types import StringLike, BoolLike, FloatLike
from .exprs import Call
from .position import Position
from .animtime import Duration, Easing
from .aobject import INHERITED_VALUE

from .nodes import (
    Node,
    NodeWithChildren,
    PositionMixin,
    StyleMixin,
    InheritedStyleMixin,
    ZLevelMixin,
)


@beartype
class TextStyleMethods:
    """Public font/style setters, shared by `TextStyleMixin` (own defaults —
    `Text`) and `InheritedTextStyleMixin` (cascading defaults — `TextGroup`/
    `TextSpan`); the methods don't care which default strategy backs them."""

    def italic(self, value: BoolLike, *, dur: Duration = None, ease: Easing = None):
        self._set_attr("italic", value, dur, ease)
        return self

    def font(self, value: StringLike, *, dur: Duration = None, ease: Easing = None):
        self._set_attr("font", value, dur, ease)
        return self

    def font_size(self, value: FloatLike, *, dur: Duration = None, ease: Easing = None):
        self._set_attr("font_size", value, dur, ease)
        return self

    def font_weight(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ):
        self._set_attr("font_weight", value, dur, ease)
        return self

    def bold(self):
        return self.font_weight(800)


@beartype
class TextStyleMixin(StyleMixin, TextStyleMethods):
    """Own (literal) font defaults — used by the top-level `Text` block."""

    _ATTR_DEFAULTS = {
        "font": "sans-serif",
        "font_size": 16,
        "font_weight": 400,
        "italic": False,
    }


@beartype
class InheritedTextStyleMixin(InheritedStyleMixin, TextStyleMethods):
    """Cascading font defaults — used by text runs (`t_group`/`t_span`),
    which inherit from their ambient `Text`/`TextGroup` ancestor when unset.
    Always terminates at a real value: these are only ever constructed under
    a `Text` ancestor (`Text.group()`/`.span()`, or transitively via
    `TextGroup.group()`/`.span()`), which has real literal font defaults via
    `TextStyleMixin` — never a bare `Group`/`Scene`."""

    _ATTR_DEFAULTS = {
        "font": INHERITED_VALUE,
        "font_size": INHERITED_VALUE,
        "font_weight": INHERITED_VALUE,
        "italic": INHERITED_VALUE,
    }


@beartype
class TextSpan(Node, InheritedTextStyleMixin):
    kind = "t_span"

    def __init__(self, parent, text: StringLike):
        super().__init__(put_in_context=False, parent=parent)
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)

    def get_pos(self):
        return Position(self._parent, Call.default_x(self), Call.default_y(self))


@beartype
class TextGroup(NodeWithChildren, InheritedTextStyleMixin):
    kind = "t_group"

    def __init__(self, parent):
        super().__init__(put_in_context=False, parent=parent)

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
        self.color("black")
        self.sh_language = None
        self.sh_theme = None

    def sh(self, language, *, theme=None):
        """
        Enable Syntax highlighting
        """
        self.sh_language = language
        self.sh_theme = theme
        return self

    def group(self) -> TextGroup:
        group = TextGroup(self)
        self._children.append(group)
        return group

    def span(self, text: StringLike) -> TextSpan:
        span = TextSpan(self, text)
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
    attrs: dict
    children: list[Union[str, "_TagNode"]] = field(default_factory=list)


_ATTR_RE = re.compile(r"([\w-]+)(?:\s*=\s*(?:'([^']*)'|\"([^\"]*)\"))?")


def _parse_tag_content(tag_content):
    """Parse 'name key=val bool-key ...' into (name, {key: value_or_True})."""
    parts = tag_content.strip().split(None, 1)
    name = parts[0] if parts else ""
    attrs = {}
    if len(parts) > 1:
        for m in _ATTR_RE.finditer(parts[1]):
            key = m.group(1)
            if m.group(2) is not None:
                attrs[key] = m.group(2)
            elif m.group(3) is not None:
                attrs[key] = m.group(3)
            else:
                attrs[key] = True
    return name, attrs


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
        if after_open < n and src[after_open] == "/":
            close_idx = src.find(close_d, after_open)
            if close_idx == -1:
                # No closing delimiter — treat '<' as literal text
                buf.append(src[idx : idx + len(open_d)])
                pos = idx + len(open_d)
                continue
            tag_name = src[after_open + 1 : close_idx].strip()
            if stop_tag is not None and tag_name == stop_tag:
                if buf:
                    nodes.append("".join(buf))
                return nodes, close_idx + len(close_d)
            raise ValueError(f"Unexpected closing tag '{tag_name}' at position {idx}")
        else:
            close_idx = src.find(close_d, after_open)
            if close_idx == -1:
                # No closing delimiter — treat '<' as literal text
                buf.append(src[idx : idx + len(open_d)])
                pos = idx + len(open_d)
                continue
            tag_content = src[after_open:close_idx]
            tag_name, tag_attrs = _parse_tag_content(tag_content)
            if buf:
                nodes.append("".join(buf))
                buf = []
            pos = close_idx + len(close_d)
            children, pos = _parse_content(src, pos, open_d, close_d, stop_tag=tag_name)
            nodes.append(_TagNode(tag_name, tag_attrs, children))

    if buf:
        nodes.append("".join(buf))

    if stop_tag is not None:
        raise ValueError(f"Missing closing tag '{stop_tag}'")

    return nodes, pos


def _plain_text(nodes):
    if any(isinstance(n, _TagNode) for n in nodes):
        return None
    return "".join(nodes)


def _apply_style(obj, attrs):
    for key, val in attrs.items():
        if key == "color":
            obj.color(val)
        elif key in ("text-size", "font-size"):
            obj.font_size(float(val))
        elif key == "bold":
            obj.bold()
        elif key == "italic":
            obj.italic(True)
        elif key == "font":
            obj.font(val)
        elif key == "font-weight":
            obj.font_weight(float(val))


def _add_lines(parent, text_str, name=None, attrs=None):
    for line in text_str.split("\n"):
        if line or name is None:
            s = parent.span(line)
            if name is not None:
                s.name(name)
            if attrs:
                _apply_style(s, attrs)


def _add_node_to(parent, node):
    if isinstance(node, str):
        _add_lines(parent, node)
    else:
        _add_tag_to(parent, node)


def _add_tag_to(parent, node):
    plain = _plain_text(node.children)
    if plain is not None:
        _add_lines(parent, plain, name=node.name, attrs=node.attrs)
    else:
        g = parent.group().name(node.name)
        _apply_style(g, node.attrs)
        for child in node.children:
            _add_node_to(g, child)


def _flush_line(parent, current_line):
    if not current_line:
        return
    if len(current_line) == 1:
        item = current_line[0]
        if isinstance(item, str):
            parent.span(item)
        else:
            _add_tag_to(parent, item)
    else:
        g = parent.group()
        for item in current_line:
            if isinstance(item, str):
                g.span(item)
            else:
                _add_tag_to(g, item)
    current_line.clear()


def _add_nodes_as_lines(parent, nodes):
    """Add mixed tag/text nodes to parent, splitting on \\n as line boundaries."""
    current_line = []
    for node in nodes:
        if isinstance(node, str):
            pieces = node.split("\n")
            for i, piece in enumerate(pieces):
                if i > 0:
                    _flush_line(parent, current_line)
                if piece:
                    current_line.append(piece)
        else:
            current_line.append(node)
    _flush_line(parent, current_line)


@beartype
def stext(input_text: str, *, strip: bool = True, delimiters: str = "<>"):
    """
    Parse input_text and create a text() node from it.

    Plain text is split on newlines into spans. Named tags become spans (leaf)
    or groups (nested), with the tag name assigned via .name(). Newlines act as
    line boundaries at the top level: items sharing a line are grouped inline,
    each newline starts a new top-level line.

    Tag attributes are applied as styles:
      color='...'            → .color(...)
      font-size='...'        → .font_size(...)  (also: text-size)
      font='...'             → .font(...)
      font-weight='...'      → .font_weight(...)
      bold                   → .bold()
      italic                 → .italic(True)

    Examples::

        stext("<green>INFO</green> message")
        stext("<span color='#f00' bold>warning</span>")
        stext("<abc font-size='12' color='blue'>text</abc>")
    """
    if strip:
        input_text = input_text.strip()

    open_d = delimiters[0]
    close_d = delimiters[1]
    nodes, _ = _parse_content(input_text, 0, open_d, close_d)

    t = Text()

    if not nodes:
        return t

    plain = _plain_text(nodes)
    if plain is not None:
        _add_lines(t, plain)
        return t

    if len(nodes) == 1:
        _add_tag_to(t, nodes[0])
    else:
        _add_nodes_as_lines(t, nodes)

    return t
