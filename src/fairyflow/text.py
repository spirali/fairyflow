import re
from dataclasses import dataclass, field
from beartype import beartype
from typing import Union, Literal, Self

from .types import StringLike, BoolLike, FloatLike
from .animtime import Duration, Easing
from .avalue import AnimatedValue
from .sentinels import INHERITED_VALUE, DEFAULT, DefaultMarker, RelValue, resolve_rel
from .ctxvars import Par

from .nodes import (
    Node,
    NodeWithChildren,
    PositionMixin,
    PositionQueryMixin,
    SizeMixin,
    KeepAspectMixin,
    StyleMixin,
    InheritedStyleMixin,
    ZLevelMixin,
)


TextAlignMode = Literal["left", "center", "right", "justify"]


@beartype
class TextStyleMethods:
    """Public font/style setters, shared by `TextStyleMixin` (own defaults —
    `Text`) and `InheritedTextStyleMixin` (cascading defaults — `TextGroup`/
    `TextSpan`); the methods don't care which default strategy backs them."""

    def font(
        self,
        family: StringLike | None = None,
        size: FloatLike | None = None,
        *,
        weight: FloatLike | None = None,
        italic: BoolLike | None = None,
        bold: bool | None = None,
        mono: bool | None = None,
        dur: Duration = None,
        ease: Easing = None,
    ):
        if bold is not None:
            if weight is not None:
                raise TypeError("font(): pass either weight= or bold=, not both")
            weight = 800 if bold else 400
        if mono is not None:
            if family is not None:
                raise TypeError(
                    "font(): pass either family (positional) or mono=, not both"
                )
            family = "monospace" if mono else "sans-serif"
        with Par():
            if family is not None:
                self._set_attr("font", family, dur, ease)
            if size is not None:
                self._set_attr("font_size", size, dur, ease)
            if weight is not None:
                self._set_attr("font_weight", weight, dur, ease)
            if italic is not None:
                self._set_attr("italic", italic, dur, ease)
        return self


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
    """Cascading font defaults — used by text runs (`tline`/`tspan`),
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
class TextSpan(Node, InheritedTextStyleMixin, PositionQueryMixin):
    kind = "tspan"

    def __init__(self, parent, text: StringLike):
        super().__init__(put_in_context=False, parent=parent)
        self._add_attr("text", text)

    def text(self, value: str):
        self._set_attr("text", value)


@beartype
class TextGroup(NodeWithChildren, InheritedTextStyleMixin, PositionQueryMixin):
    kind = "tline"

    def __init__(self, parent):
        super().__init__(put_in_context=False, parent=parent)
        self._text_align = None

    def span(self, text: StringLike):
        span = TextSpan(self, text)
        self._children.append(span)
        return span

    def group(self):
        group = TextGroup(self)
        self._children.append(group)
        return group

    def text_align(self, mode: TextAlignMode) -> Self:
        """Override this line's alignment, independent of the block default.

        Args:
            mode: ``"left"``, ``"center"``, ``"right"``, or ``"justify"``.

        Returns:
            self, for method chaining.
        """
        self._text_align = mode
        return self

    def serialize(self, serializer):
        result = super().serialize(serializer)
        if self._text_align is not None:
            result["text_align"] = self._text_align
        return result


class Text(
    NodeWithChildren,
    PositionMixin,
    SizeMixin,
    KeepAspectMixin,
    TextStyleMixin,
    ZLevelMixin,
):
    kind = "text"

    def __init__(self, text: StringLike | None = None):
        super().__init__()
        self.fill("black")
        self._add_attr("keep_aspect", True)
        self.sh_language = None
        self.sh_theme = None
        self._text_align = None
        self._current_line = None
        if text:
            for part in text.split("\n"):
                self.line(part)

    def sh(self, language, *, theme=None):
        """
        Enable Syntax highlighting
        """
        self.sh_language = language
        self.sh_theme = theme
        return self

    def wrap(self, width: FloatLike | RelValue | DefaultMarker) -> Self:
        """Wrap lines at a maximum width, in unscaled layout units (i.e.
        before any `size()` fit-scaling is applied).

        Args:
            width: Maximum line width in pixels, or `rel(f)` for `f` times
                the parent's width. ``DEFAULT`` turns wrapping back off —
                unlike `x(DEFAULT)`/`size(w=DEFAULT)`, there's no engine-side
                "auto wrap" to fall back to, so this removes the attribute
                entirely rather than writing a default-referencing expression.

        Returns:
            self, for method chaining.
        """
        if width is DEFAULT:
            self._attrs.pop("wrap", None)
        else:
            # No `_ATTR_DEFAULTS["wrap"]` entry exists (nothing to fall back
            # to), so this bypasses `_set_attr`/`_ensure_attr` — which always
            # need a default for a first-time attribute — the same way
            # `_add_attr` does, except re-assignable since `wrap()` can be
            # called more than once (never animated, so each call fully
            # replaces the prior value; no dur=/ease=).
            self._attrs["wrap"] = AnimatedValue(
                resolve_rel(width, self, "width"), self._start
            )
        return self

    def text_align(self, mode: TextAlignMode) -> Self:
        """Set the block-default paragraph alignment.

        Args:
            mode: ``"left"`` (default), ``"center"``, ``"right"``, or
                ``"justify"``. A `TextGroup` line can override this via its
                own `text_align()`.

        Returns:
            self, for method chaining.
        """
        self._text_align = mode
        return self

    def line(self, text: StringLike = "") -> Union[TextSpan, TextGroup]:
        """Start a new line, closing off the current one.

        With `text`, the line is seeded with a single run and that run is
        returned directly (no wrapper group — the common case); call
        `.span()`/`.group()` on the result to add more runs to it later.
        """
        if text:
            span = TextSpan(self, text)
            self._children.append(span)
            self._current_line = span
            return span
        group = TextGroup(self)
        self._children.append(group)
        self._current_line = group
        return group

    def _promote_to_group(self) -> TextGroup:
        """Convert the current bare-`TextSpan` line into a `TextGroup` in
        place (preserving its sibling position) so a second run can join
        it — mirrors `stext`'s own single-run collapsing, done lazily."""
        cur = self._current_line
        idx = self._children.index(cur)
        group = TextGroup(self)
        self._children[idx] = group
        cur._parent = group
        group._children.append(cur)
        self._current_line = group
        return group

    def group(self) -> TextGroup:
        """Start a nested inline group within the current (last) line,
        starting a fresh line first if none exists yet."""
        cur = self._current_line
        if cur is None:
            group = TextGroup(self)
            self._children.append(group)
            self._current_line = group
            return group
        if isinstance(cur, TextSpan):
            cur = self._promote_to_group()
        return cur.group()

    def span(self, text: StringLike) -> TextSpan:
        """Append an inline run to the current (last) line, starting a
        fresh line first if none exists yet."""
        cur = self._current_line
        if cur is None:
            span = TextSpan(self, text)
            self._children.append(span)
            self._current_line = span
            return span
        if isinstance(cur, TextSpan):
            cur = self._promote_to_group()
        return cur.span(text)

    def serialize(self, serializer):
        result = super().serialize(serializer)
        if self.sh_language is not None:
            result["sh_language"] = self.sh_language
        if self.sh_theme is not None:
            result["sh_theme"] = self.sh_theme
        if self._text_align is not None:
            result["text_align"] = self._text_align
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
            obj.fill(val)
        elif key in ("text-size", "font-size"):
            obj.font(size=float(val))
        elif key == "bold":
            obj.font(bold=True)
        elif key == "italic":
            obj.font(italic=True)
        elif key == "font":
            obj.font(val)
        elif key == "font-weight":
            obj.font(weight=float(val))


def _add_lines(parent, text_str, name=None, attrs=None):
    for line in text_str.split("\n"):
        if line or name is None:
            s = TextSpan(parent, line)
            parent._children.append(s)
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
        g = TextGroup(parent)
        parent._children.append(g)
        g.name(node.name)
        _apply_style(g, node.attrs)
        for child in node.children:
            _add_node_to(g, child)


def _flush_line(parent, current_line):
    if not current_line:
        return
    if len(current_line) == 1:
        item = current_line[0]
        if isinstance(item, str):
            s = TextSpan(parent, item)
            parent._children.append(s)
        else:
            _add_tag_to(parent, item)
    else:
        g = TextGroup(parent)
        parent._children.append(g)
        for item in current_line:
            if isinstance(item, str):
                s = TextSpan(g, item)
                g._children.append(s)
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
      color='...'            → .fill(...)
      font-size='...'        → .font(size=...)  (also: text-size)
      font='...'             → .font(...)
      font-weight='...'      → .font(weight=...)
      bold                   → .font(bold=True)
      italic                 → .font(italic=True)

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
