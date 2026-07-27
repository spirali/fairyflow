from typing import Union, Literal, Self, SupportsFloat
from beartype import beartype
import os

from .types import ColorLike, FillLike, FloatLike
from .layout import CENTERING_LAYOUT, ColumnLayout, GridLayout, RowLayout
from .position import Position
from .info import get_info
from .animtime import Duration, Easing
from .sentinels import (
    DEFAULT,
    INHERITED_VALUE,
    OMITTED,
    DefaultMarker,
    OmittedMarker,
    RelValue,
    rel,
    resolve_rel,
)
from .aobject import AnimatedObject, get_frame
from .avalue import AnimatedValue
from .color import Color, Gradient
from .exprs import (
    Call,
    Expr,
    to_expr,
)
from .ctxvars import (
    AnimProxy,
    Par,
    Seq,
    end_frame,
    get_current_node,
    ROOT_OBJECTS,
    reset_scene,
    set_current_node,
)
from .config import DEFAULT_SCENE_CONFIG


@beartype
class Node(AnimatedObject):
    """Base class for all scene nodes.

    Manages parent-child relationships, frame-based visibility, and
    serialization. Not intended to be instantiated directly — use a concrete
    subclass such as `Rect`, `Group`, or `Path`.
    """

    def __init__(self, put_in_context: bool = True, parent=None):
        super().__init__()
        if put_in_context:
            parent = get_current_node()
            if parent is None:
                raise Exception("Element created out of context of a parent node")
            parent._children.append(self)
            self._parent = parent
        else:
            self._parent = parent
        if parent:
            self._id = parent._new_id()
        else:
            self._id = None
        self.info = get_info()
        self._name = None

    def name(self, name: str | None):
        """Set the display name of the node.

        The name is not interpreted by FairyFlow and is used only for debugging
        or for searching nodes via `find_node`.

        Args:
            name: A human-readable label for the node, or ``None`` to clear it.

        Returns:
            self, for method chaining.
        """
        self._name = name
        return self

    def parent_chain(self) -> list["Group"]:
        """Return all ancestor `Group` nodes from this node up to the root.

        Traverses the parent chain and collects every ancestor that is a
        `Group` instance, ordered from nearest to farthest.

        Returns:
            A list of `Group` ancestors, nearest first. Empty if no `Group`
            ancestor exists.
        """
        result = []
        node = self
        while node is not None:
            if isinstance(node, Group):
                result.append(node)
            node = node._parent
        return result

    def parent_group(self) -> Union["Group", "Scene"]:
        """Return the nearest ancestor that is a `Group` or `Scene`.

        Walks up the parent chain and returns the first node that is either a
        `Group` or a `Scene`.

        Returns:
            The nearest `Group` or `Scene` ancestor, or ``None`` if ``self``
            is the root `Scene`.
        """
        parent = self._parent
        if (
            isinstance(self._parent, Group)
            or isinstance(self._parent, Scene)
            or parent is None
        ):
            return parent
        return parent.parent_group()

    def _new_id(self):
        return self._parent._new_id()

    _WIRE_KEY = {
        "width": "w",
        "height": "h",
        "fill_color": "fill",
        "stroke_color": "stroke",
        "path": "file",
    }

    def serialize(self, serializer):
        from .serializer import serialize_expr

        result = {"kind": self.kind}
        if self._start > 0:
            result["start"] = self._start
        if self._end is not None:
            result["end"] = self._end
        for name, av in self._attrs.items():
            if av.is_default:
                continue
            result[self._WIRE_KEY.get(name, name)] = serialize_expr(av)
        if self.info:
            result["info"] = self.info
        return result

    def _get_parent(self):
        if self._parent is None:
            raise Exception("Node does not have parent")
        return self._parent

    def match(self, *, name: str | None = None, kind: str | None = None) -> bool:
        """Return whether this node satisfies all supplied filter criteria.

        Each argument is optional; only provided arguments are checked.

        Args:
            name: If given, the node's name must equal this value.
            kind: If given, the node's ``kind`` string must equal this value.

        Returns:
            ``True`` if every supplied criterion matches, ``False`` otherwise.
        """
        if name is not None and self._name != name:
            return False
        if kind is not None and self.kind != kind:
            return False
        return True

    def find_node(
        self, *, name: str | None = None, kind: str | None = None
    ) -> Union["Node", None]:
        """Find the first node matching the filter criteria using BFS.

        Searches the subtree rooted at this node in breadth-first order and
        returns the first node that satisfies all supplied criteria.

        Args:
            name: If given, only nodes with this name are considered.
            kind: If given, only nodes whose ``kind`` equals this are considered.

        Returns:
            The first matching `Node`, or ``None`` if no match is found.
        """
        if self.match(name=name, kind=kind):
            return self
        else:
            return None

    def get_scene(self) -> "Scene":
        """Return the root `Scene` of the scene tree.

        Returns:
            The `Scene` node that is the root ancestor of all nodes in this tree.
        """
        return self._parent.get_scene()

    def anim(self, dur: Duration = None, *, ease: Easing = None) -> AnimProxy:
        """Return a proxy for animating several attributes of this node at once.

        Every setter called on the returned proxy is pre-filled with `dur`/
        `ease` and runs in parallel with the others in the chain — starting
        at the same frame and advancing the clock by `dur` once, not once
        per call:

        ```python
        g.anim(0.8).scale(1.9).xy(550, 400)
        # equivalent to:
        # with Par():
        #     g.scale(1.9, dur=0.8)
        #     g.xy(550, 400, dur=0.8)
        ```

        A `dur=`/`ease=` passed to an individual chained call still
        overrides the proxy's.

        Args:
            dur: Duration applied to every chained setter call.
            ease: Optional easing curve applied to every chained setter call.

        Returns:
            An `AnimProxy` wrapping this node.
        """
        return AnimProxy(self, dur, ease)

    def __repr__(self):
        if self._name:
            return f"<{self.kind} id={self._id} name={self._name}>"
        else:
            return f"<{self.kind} id={self._id}>"


@beartype
class AlphaMixin:
    """Mixin that adds animatable opacity (`alpha`) to a node. Own (literal)
    default of 1 — used standalone (Group/Image/Layer) and as the base of
    `StyleMixin`. `InheritedStyleMixin` (listed before `AlphaMixin` in its
    own bases, so its `_ATTR_DEFAULTS` entry is found first) overrides
    `alpha` to be inherited instead (text runs)."""

    _ATTR_DEFAULTS = {"alpha": 1}

    def alpha(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the opacity of this node and all its children.

        Args:
            value: Opacity in the range ``[0.0, 1.0]``, where ``0.0`` is fully
                transparent and ``1.0`` is fully opaque.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("alpha", value, dur, ease)
        return self

    def fade_in(self, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Animate a fade-in effect by transitioning alpha from 0 to 1.

        Sets the node alpha to ``0`` at the current frame and animates it to
        ``1`` over the given duration.

        Args:
            dur: Duration of the animation in seconds. If unset, uses the
                enclosing `anim()` block's default, or is instant if there
                is none.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            self.alpha(0, dur=0)
            self.alpha(1, dur=dur, ease=ease)
        return self

    def fade_out(self, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Animate a fade-out effect by transitioning alpha to 0.

        Animates the node alpha from ``1`` down to ``0`` over the
        given duration.

        Args:
            dur: Duration of the animation in seconds. If unset, uses the
                enclosing `anim()` block's default, or is instant if there
                is none.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            self.alpha(1, dur=0)
            self.alpha(0, dur=dur, ease=ease)
        return self


@beartype
class ZLevelMixin:
    """Mixin that adds a `z()` setter for the node's z-level (rendering
    order). Always inherited-from-parent when unset — there is no "own"
    variant."""

    _ATTR_DEFAULTS = {"z": INHERITED_VALUE}

    def z(self, value: FloatLike, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Set the z-level (rendering order) of the node.

        Nodes with higher z-levels are drawn on top of nodes with lower values.

        Args:
            value: The z-level. Higher values appear in front.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("z", value, dur, ease)
        return self


@beartype
class SizeMixin:
    """Mixin that adds animatable `width` and `height` attributes to a node.
    Defaults to the layout-computed size when unset — for shape kinds with no
    layout concept of their own (rect/ellipse) the engine's fallback already
    resolves this to 0 either way (`layout.rs::auto_width/height`'s
    catch-all), so there's no need for those kinds to special-case it."""

    _ATTR_DEFAULTS = {
        "width": Call.auto_width,
        "height": Call.auto_height,
    }

    def width(
        self,
        value: FloatLike | RelValue,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the width of the node in pixels.

        Args:
            value: The new width in pixels. `rel(f)` sets it to `f` times the
                parent's width instead.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("width", resolve_rel(value, self, "width"), dur, ease)
        return self

    def height(
        self,
        value: FloatLike | RelValue,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the height of the node in pixels.

        Args:
            value: The new height in pixels. `rel(f)` sets it to `f` times the
                parent's height instead.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("height", resolve_rel(value, self, "height"), dur, ease)
        return self

    def size(
        self,
        w: FloatLike | RelValue | None = None,
        h: FloatLike | RelValue | None = None,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the width and/or height of the node in pixels.

        Args:
            w: The new width in pixels, or `rel(f)`. ``None`` (default)
                leaves the width untouched.
            h: The new height in pixels, or `rel(f)`. ``None`` (default)
                leaves the height untouched.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            if w is not None:
                self.width(w, dur=dur, ease=ease)
            if h is not None:
                self.height(h, dur=dur, ease=ease)
        return self

    def expand(self, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Size the node to fill its parent completely (``rel(1)`` on both axes).

        Args:
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        return self.size(rel(1), rel(1), dur=dur, ease=ease)


class KeepAspectMixin:
    """Mixin that adds a `keep_aspect()` runtime setter for nodes whose
    content is scaled to fit an explicit box (`Image`, `Text`). Boolean and
    instant only (no `dur=`/`ease=`) — it only matters when both size axes
    are explicit, and toggling it is a discrete fit-mode switch, not
    something to interpolate."""

    def keep_aspect(self, value: bool = True) -> Self:
        """Set whether content is letterboxed (preserving aspect ratio) or
        stretched to fill an explicit `size(w=, h=)` box.

        Args:
            value: ``True`` (default) letterboxes/pillarboxes and centers
                the content, preserving its aspect ratio. ``False`` stretches
                it to exactly fill the box.

        Returns:
            self, for method chaining.
        """
        self._set_attr("keep_aspect", value)
        return self


AnchorName = Literal[
    "center",
    "top",
    "bottom",
    "left",
    "right",
    "top_left",
    "top_right",
    "bottom_left",
    "bottom_right",
]

_ANCHORS: dict[AnchorName, tuple[float, float]] = {
    "center": (0.5, 0.5),
    "top": (0.5, 0.0),
    "bottom": (0.5, 1.0),
    "left": (0.0, 0.5),
    "right": (1.0, 0.5),
    "top_left": (0.0, 0.0),
    "top_right": (1.0, 0.0),
    "bottom_left": (0.0, 1.0),
    "bottom_right": (1.0, 1.0),
}


def _effective_width(node):
    if isinstance(node, SizeMixin):
        return node._get_attr("width")
    if isinstance(node, (PathMove, PathLine, PathCubic)):
        return 0
    return Call.auto_width(node)


def _effective_height(node):
    if isinstance(node, SizeMixin):
        return node._get_attr("height")
    if isinstance(node, (PathMove, PathLine, PathCubic)):
        return 0
    return Call.auto_height(node)


def _resolve_anchor(x, y) -> tuple[float, float]:
    if isinstance(x, str):
        if y is not None:
            raise TypeError("at(): a named anchor can't be combined with a y fraction")
        return _ANCHORS[x]
    if x is None and y is None:
        return 0.5, 0.5
    if x is None or y is None:
        raise TypeError("at(): fractions must be given in pairs, e.g. at(0.5, 0.5)")
    return x, y


@beartype
class PositionQueryMixin:
    """Allows to read a position"""

    _ATTR_DEFAULTS = {
        "x": Call.auto_x,
        "y": Call.auto_y,
    }

    def at(
        self,
        x: FloatLike | AnchorName | None = None,
        y: FloatLike | None = None,
    ) -> Position:
        """Return a point on the node as a live `Position`.

        Args:
            x: Horizontal fraction (``0.0`` left edge, ``1.0`` right edge), or
                a named anchor string (``"center"``, ``"top_left"``, ...).
                ``None`` with `y` also `None` defaults to the center.
            y: Vertical fraction (``0.0`` top edge, ``1.0`` bottom edge).
                Fractions must be given in pairs — a bare `at(0.5)` raises
                `TypeError` rather than silently meaning top-center.

        Returns:
            A `Position` representing the node's resolved coordinates. For
            `SizeMixin` nodes this uses the actual (possibly overridden) box;
            for path command handles (genuinely zero-size) every anchor
            resolves to the same raw point; for anything else (currently
            `Text`, `TextSpan`, `TextGroup`) it falls back to the engine's
            measured/computed extent, so e.g. a text node's `at("center")` is
            its true visual center.
        """
        align_x, align_y = _resolve_anchor(x, y)

        px = self._get_attr("x")
        py = self._get_attr("y")
        if align_x != 0:
            w = _effective_width(self)
            if w != 0:
                px = px + w * align_x
        if align_y != 0:
            h = _effective_height(self)
            if h != 0:
                py = py + h * align_y
        # A node's own x/y are relative to its parent's frame - except when
        # there is no parent, i.e. this node is the Scene itself, whose
        # frame *is* the root frame (Position.into_node()/serialize_expr's
        # Scene guard both key off this same "self is its own frame" case).
        frame = self._parent if self._parent is not None else self
        return Position(frame, px, py)

    def get_x(self) -> Expr:
        """Return the node's x position as a live expression.

        Returns:
            An `Expr` for the node's x coordinate, usable in arithmetic and
            as an argument to other setters.
        """
        return self._get_attr("x")

    def get_y(self) -> Expr:
        """Return the node's y position as a live expression.

        Returns:
            An `Expr` for the node's y coordinate, usable in arithmetic and
            as an argument to other setters.
        """
        return self._get_attr("y")

    def get_w(self) -> FloatLike:
        """Return the node's effective width as a live expression.

        Returns:
            An `Expr` for the node's width — its own `width` attribute if it
            has one, or the engine's measured/computed extent otherwise (e.g.
            for `Text`-like nodes). The literal `0` for zero-size path
            command handles, matching `.at()`'s own handling of them.
        """
        return _effective_width(self)

    def get_h(self) -> FloatLike:
        """Return the node's effective height as a live expression.

        Returns:
            An `Expr` for the node's height — its own `height` attribute if
            it has one, or the engine's measured/computed extent otherwise
            (e.g. for `Text`-like nodes). The literal `0` for zero-size path
            command handles, matching `.at()`'s own handling of them.
        """
        return _effective_height(self)


@beartype
class PositionMixin(PositionQueryMixin):
    """Allows to set a position"""

    def x(
        self,
        px: FloatLike | RelValue | DefaultMarker,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the x coordinate of the node.

        Args:
            px: The x position in pixels, relative to the parent node's origin.
                `rel(f)` sets it to `f` times the parent's width; ``DEFAULT``
                resets it to the layout-computed position.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        value = Call.auto_x(self) if px is DEFAULT else resolve_rel(px, self, "width")
        self._set_attr("x", value, dur, ease)
        return self

    def y(
        self,
        px: FloatLike | RelValue | DefaultMarker,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the y coordinate of the node.

        Args:
            px: The y position in pixels, relative to the parent node's origin.
                `rel(f)` sets it to `f` times the parent's height; ``DEFAULT``
                resets it to the layout-computed position.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        value = Call.auto_y(self) if px is DEFAULT else resolve_rel(px, self, "height")
        self._set_attr("y", value, dur, ease)
        return self

    def xy(
        self,
        x: FloatLike | DefaultMarker | None = None,
        y: FloatLike | DefaultMarker | None = None,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the x and/or y coordinate of the node.

        Args:
            x: The x position in pixels, relative to the parent node's origin.
                ``None`` (default) leaves x untouched; ``DEFAULT`` resets it to
                the layout-computed position.
            y: The y position in pixels, relative to the parent node's origin.
                ``None`` (default) leaves y untouched; ``DEFAULT`` resets it to
                the layout-computed position.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            if x is not None:
                self.x(x, dur=dur, ease=ease)
            if y is not None:
                self.y(y, dur=dur, ease=ease)
        return self

    def align(
        self,
        x: FloatLike | None = None,
        y: FloatLike | None = None,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Align the node within its parent, horizontally and/or vertically.

        Args:
            x: Horizontal alignment factor. ``0.0`` aligns to the left edge,
                ``0.5`` to the center, ``1.0`` to the right edge. ``None``
                (default) leaves the x axis untouched.
            y: Vertical alignment factor. ``0.0`` aligns to the top edge,
                ``0.5`` to the center, ``1.0`` to the bottom edge. ``None``
                (default) leaves the y axis untouched.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        parent = self.parent_group()
        # Only `Group` carries padding attrs (`Scene` doesn't) - align()
        # within the parent's *padded* box, matching the engine's
        # padding-aware Center-layout formula, so `.align()` and
        # `.padding()` compose instead of `.align()` silently ignoring
        # padding entirely.
        if isinstance(parent, Group):
            pl = parent._get_attr("padding_left")
            pr = parent._get_attr("padding_right")
            pt = parent._get_attr("padding_top")
            pb = parent._get_attr("padding_bottom")
        else:
            pl = pr = pt = pb = 0
        with Par():
            if x is not None:
                inner_w = Call.sub(Call.sub(parent._get_attr("width"), pl), pr)
                new_x = Call.add(
                    pl, Call.mul(Call.sub(inner_w, _effective_width(self)), x)
                )
                self._set_attr("x", new_x, dur, ease)
            if y is not None:
                inner_h = Call.sub(Call.sub(parent._get_attr("height"), pt), pb)
                new_y = Call.add(
                    pt, Call.mul(Call.sub(inner_h, _effective_height(self)), y)
                )
                self._set_attr("y", new_y, dur, ease)
        return self

    def pos(
        self, position: Position, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the position of the node using a `Position` object.

        Args:
            position: The target position, resolved relative to the parent node.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            position = position.into_node(self._parent)
            self._set_attr("x", position.x, dur, ease)
            self._set_attr("y", position.y, dur, ease)
        return self

    def move(
        self, dx: FloatLike, dy: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Shift the node's position by a relative offset.

        Args:
            dx: Horizontal offset in pixels.
            dy: Vertical offset in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._move_attr("x", dx, dur, ease)
            self._move_attr("y", dy, dur, ease)
        return self

    def next_to(
        self,
        node: "PositionQueryMixin",
        direction: Literal["right", "left", "above", "below"] = "right",
        gap: FloatLike = 0,
        align: FloatLike = 0.5,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Place this node beside another node, accounting for both boxes' size.

        Args:
            node: The sibling (or any other) node to place next to.
            direction: Which side of `node` to place this node on.
            gap: Pixel gap between the facing edges.
            align: Placement along the perpendicular axis: ``0.0`` start-aligned,
                ``0.5`` centered, ``1.0`` end-aligned.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        nx = node._get_attr("x")
        ny = node._get_attr("y")
        nw = _effective_width(node)
        nh = _effective_height(node)

        if direction in ("right", "left"):
            target_y = Call.add(ny, Call.mul(nh, align))
            target_x = (
                Call.add(Call.add(nx, nw), gap)
                if direction == "right"
                else Call.sub(nx, gap)
            )
        else:
            target_x = Call.add(nx, Call.mul(nw, align))
            target_y = (
                Call.add(Call.add(ny, nh), gap)
                if direction == "below"
                else Call.sub(ny, gap)
            )

        # node's own x/y are relative to its parent's frame - except when node
        # is the Scene itself, whose frame IS the root frame (same fix as
        # PositionQueryMixin.at()).
        node_frame = node._parent if node._parent is not None else node
        position = Position(node_frame, target_x, target_y).into_node(self._parent)
        sw = _effective_width(self)
        sh = _effective_height(self)

        final_x = position.x
        final_y = position.y
        if direction in ("right", "left"):
            final_y = Call.sub(final_y, Call.mul(sh, align))
            if direction == "left":
                final_x = Call.sub(final_x, sw)
        else:
            final_x = Call.sub(final_x, Call.mul(sw, align))
            if direction == "above":
                final_y = Call.sub(final_y, sh)

        with Par():
            self._set_attr("x", final_x, dur, ease)
            self._set_attr("y", final_y, dur, ease)
        return self

    def follow_path(
        self,
        path: "Path",
        *,
        dur: Duration = 1,
        ease: Easing = None,
        start: FloatLike = 0,
        end: FloatLike = 1,
    ) -> Self:
        """Animate this node along a path.

        Args:
            path: The `Path` to follow.
            dur: Duration of the animation in seconds.
            ease: Optional easing curve (``"in_out"`` default).
            start: Path parameter at the start of the animation (0 = path start, 1 = path end).
            end: Path parameter at the end of the animation.

        Pass ``start=1, end=0`` to travel backwards (from the path's end to its start).

        Returns:
            self, for method chaining.
        """
        assert isinstance(path, Path)
        av = AnimatedValue(start)
        x = Call.path_x(path, av)
        y = Call.path_y(path, av)
        # `isinstance` here, not `_has_attr("width")`: under lazy attributes,
        # a freshly-created sizeable node has no "width" entry yet either —
        # the question is whether this *kind* has a size concept at all
        # (used to center it on the path), not whether it's been touched.
        if isinstance(self, SizeMixin):
            x = x - Call.mul(self._get_attr("width"), 0.5)
            y = y - Call.mul(self._get_attr("height"), 0.5)
            self.xy(x, y)
        av.set(end, dur=dur, ease=ease)
        return self


@beartype
class StyleMethods:
    """Public fill/stroke setters, shared by `StyleMixin` (own defaults) and
    `InheritedStyleMixin` (inherited defaults) — the methods themselves don't
    care which default strategy backs the attribute."""

    def fill(
        self, value: FillLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the fill color (or gradient) of the node.

        Args:
            value: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance), or a
                `gradient(...)`.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        parsed = value if isinstance(value, Gradient) else Color.parse(value)
        self._set_attr("fill_color", parsed, dur, ease)
        return self

    def stroke(
        self,
        color: ColorLike | OmittedMarker = OMITTED,
        width: FloatLike | None = None,
        *,
        dash: tuple[SupportsFloat, SupportsFloat] | None = None,
        offset: FloatLike | None = None,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the stroke (outline) of the node: color, width, and dash pattern.

        Args:
            color: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance). Pass `None` to
                disable the stroke (matches `.fill(None)` for fill); omit
                entirely to leave the current stroke color untouched.
            width: The stroke width in pixels.
            dash: ``(on, off)`` pixel lengths for a dashed stroke; omitted
                means a solid stroke. Only supported on `Rect`, `Ellipse`,
                and `Path` — not animatable (structural, set once).
            offset: Shifts the dash pattern along the stroke, e.g. to animate
                a "marching ants" effect. Only meaningful together with
                `dash` (own or previously set).
            dur: Optional duration for animation (`color`/`width`/`offset`).
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        if (dash is not None or offset is not None) and not isinstance(
            self, (Rect, Ellipse, Path)
        ):
            raise TypeError("dash/offset are only supported on Rect, Ellipse, and Path")
        if isinstance(color, Gradient):
            raise TypeError("stroke() does not support gradients, only fill() does")
        with Par():
            if color is not OMITTED:
                self._set_attr("stroke_color", Color.parse(color), dur, ease)
            if width is not None:
                self._set_attr("stroke_width", width, dur, ease)
            if dash is not None:
                on, off = dash
                on = float(on)
                off = float(off)
                if on <= 0 or off <= 0:
                    raise ValueError("dash on/off lengths must be positive")
                self._set_attr("dash_on", on)
                self._set_attr("dash_off", off)
            if offset is not None:
                self._set_attr("dash_offset", offset, dur, ease)
        return self


@beartype
class StyleMixin(AlphaMixin, StyleMethods):
    """Mixin that adds fill color, stroke color, stroke width, and alpha to a
    node, with literal (own) defaults — used by shapes (rect/ellipse/path)
    and the top-level `Text` block. See `InheritedStyleMixin` for the
    cascading variant used by text runs."""

    _ATTR_DEFAULTS = {
        "fill_color": "",
        "stroke_color": "",
        "stroke_width": 1,
        "dash_on": 0.0,
        "dash_off": 0.0,
        "dash_offset": 0.0,
    }


@beartype
class InheritedStyleMixin(AlphaMixin, StyleMethods):
    """Like `StyleMixin`, but fill/stroke/stroke_width/alpha default to the
    parent's resolved value when unset instead of a literal constant — used
    by text runs (`tline`/`tspan`), which cascade style from their
    ambient `Text`/`TextGroup` ancestor. `alpha` is overridden here (listed
    before `AlphaMixin` in the bases, so this entry is found first) since it
    cascades the same way as the rest of the style here, unlike everywhere
    else it's used."""

    _ATTR_DEFAULTS = {
        "fill_color": INHERITED_VALUE,
        "stroke_color": INHERITED_VALUE,
        "stroke_width": INHERITED_VALUE,
        "alpha": INHERITED_VALUE,
    }


@beartype
class NodeWithChildren(Node):
    """Base class for nodes that own a list of child nodes."""

    def __init__(self, put_in_context: bool = True, parent: Node = None, children=None):
        super().__init__(put_in_context=put_in_context, parent=parent)
        if children is None:
            children = []
        self._children = children
        self._ctx = None

    def serialize(self, serializer):
        result = super().serialize(serializer)
        if self._children:
            result["children"] = [
                serializer.add_node(child) for child in self._children
            ]
        return result

    def find_node(self, *, name=None, kind=None) -> Node | None:
        result = super().find_node(name=name, kind=kind)
        if result is not None:
            return result
        for child in self._children:
            result = child.find_node(name=name, kind=kind)
            if result is not None:
                return result
        return None

    def get_child(
        self, *, name: str | None = None, kind: str | None = None
    ) -> Node | None:
        """Return the first direct child matching the given filter criteria.

        Only immediate children are searched, not deeper descendants.

        Args:
            name: If given, only children with this name are considered.
            kind: If given, only children whose ``kind`` equals this are considered.

        Returns:
            The first matching child `Node`, or ``None`` if no match is found.
        """
        for child in self._children:
            if child.match(name=name, kind=kind):
                return child
        return None

    def get_children(self) -> list[Node]:
        """Return the list of all direct children of this node.

        Returns:
            A list of child `Node` objects in the order they were added.
        """
        return self._children


@beartype
class ContextManagerMixin:
    """Mixin that makes a node usable as a ``with`` context manager.

    While the block is active, the node is set as the current node so that
    newly created child nodes are automatically attached to it.
    """

    def _init_context_manager(self):
        self._ctx = None

    def __enter__(self):
        assert self._ctx is None
        self._ctx = get_current_node()
        set_current_node(self)
        return self

    def __exit__(self, *args):
        set_current_node(self._ctx)
        self._ctx = None


@beartype
class RotAndScaleMixin:
    """Mixin that adds rotation, pivot point, and x/y scale attributes to a node."""

    _ATTR_DEFAULTS = {
        "rotation": 0,
        # Wire-inert placeholders: AnimatedValue.is_default entries are never
        # serialized (Node.serialize() skips them), so these values never
        # reach the wire. The engine's own w*0.5/h*0.5 default is what
        # actually governs an unset pivot; kept at 0.5 rather than churned
        # since there's no single static value that would be more "correct".
        "pivot_x": 0.5,
        "pivot_y": 0.5,
        "scale_x": 1,
        "scale_y": 1,
    }

    def scale_x(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Scale the node along the x axis.

        Args:
            value: Scale factor. ``1.0`` is the original size; ``2.0`` doubles
                the width.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("scale_x", value, dur, ease)
        return self

    def scale_y(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Scale the node along the y axis.

        Args:
            value: Scale factor. ``1.0`` is the original size; ``2.0`` doubles
                the height.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("scale_y", value, dur, ease)
        return self

    def scale(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Scale the node uniformly along both axes.

        Args:
            value: Scale factor applied to both x and y. ``1.0`` is the
                original size; ``2.0`` doubles both dimensions.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._set_attr("scale_x", value, dur, ease)
            self._set_attr("scale_y", value, dur, ease)
        return self

    def rotate(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Rotate the node around its pivot point.

        The pivot defaults to the node's center (``0.5``, ``0.5``) and is
        set with ``pivot()``.

        Args:
            value: Rotation angle in degrees, clockwise.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("rotation", value, dur, ease)
        return self

    def pivot(
        self,
        point: AnchorName | Position | float | int | None = None,
        /,
        *,
        x: FloatLike | None = None,
        y: FloatLike | None = None,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the transform origin used by ``rotate()`` and ``scale()``.

        Args:
            point: A named anchor (``"center"``, ``"top_left"``, ...) or a
                live ``Position`` — e.g. ``planet.pivot(sun.at("center"))``
                for a tidally-locked orbit. Mutually exclusive with
                ``x=``/``y=``.
            x: Absolute pixels from the node's own top-left corner. Negative
                or out-of-box values are allowed — pivoting around an
                external point is a real technique. Wrap in ``rel(f)`` for
                an own-box-relative fraction instead (``rel(0.5)`` is the
                default center, equivalent to ``pivot("center")``).
                Mutually exclusive with ``point``.
            y: Same as ``x``, for the vertical axis.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.

        Own-box fractions are spelled ``node.pivot(x=rel(0.3), y=rel(0.7))``
        — unlike every other setter accepting ``rel()``, which is always
        *parent*-relative, a pivot's ``rel()`` is relative to the node's own
        box (there's no useful parent-relative reading for a transform
        origin). ``node.pivot(node.at(0.3, 0.7))`` remains an equivalent,
        more verbose spelling via a live ``Position``.
        """
        if point is not None and not isinstance(point, (str, Position)):
            raise TypeError(
                "pivot(): a bare number isn't accepted here — use a named "
                "anchor, x=rel(f)/y=rel(f) for an own-box fraction, or a "
                "Position, e.g. node.pivot(x=rel(0.3), y=rel(0.7))"
            )
        if point is not None and (x is not None or y is not None):
            raise TypeError("pivot(): pass either a point or x=/y=, not both")

        fx = fy = None
        if isinstance(point, str):
            afx, afy = _ANCHORS[point]
            fx = Call.mul(afx, _effective_width(self))
            fy = Call.mul(afy, _effective_height(self))
        elif isinstance(point, Position):
            if isinstance(self, Path):
                raise TypeError(
                    "pivot(): Path has no measurable box yet — use a named "
                    "anchor instead of a Position"
                )
            mapped = point.into_node(self)
            px, py = mapped.x, mapped.y
            if isinstance(self, (Rect, Ellipse)):
                px = Call.sub(px, self._get_attr("x"))
                py = Call.sub(py, self._get_attr("y"))
            fx, fy = px, py
        elif x is not None or y is not None:
            if isinstance(self, Path):
                raise TypeError(
                    "pivot(): Path has no measurable box yet — use a named "
                    "anchor instead of x=/y="
                )
            fx = (
                Call.mul(x.factor, _effective_width(self))
                if isinstance(x, RelValue)
                else x
            )
            fy = (
                Call.mul(y.factor, _effective_height(self))
                if isinstance(y, RelValue)
                else y
            )
        else:
            return self

        with Par():
            if fx is not None:
                self._set_attr("pivot_x", fx, dur, ease)
            if fy is not None:
                self._set_attr("pivot_y", fy, dur, ease)
        return self


@beartype
class CameraProxy:
    """Returned by `.camera` — a fresh, stateless wrapper each access (same
    shape as `Position`), not stored on the node."""

    def __init__(self, node):
        self._node = node

    def zoom(
        self, factor: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Magnify this group's content around the current camera center.

        Does not change the group's own box — only its content is scaled.
        Pair with `.clip()` if overflowing content should not be visible.

        Args:
            factor: Zoom factor. ``1.0`` is the original scale; ``2.0``
                magnifies content 2x.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._node._set_attr("camera_zoom", factor, dur, ease)
        return self

    def center(
        self,
        x: FloatLike | Position,
        y: FloatLike | None = None,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the content point the camera looks at.

        Args:
            x: Either the x coordinate (in this group's own content space) or
                a live `Position` to track — e.g. `g.camera.center(node.at("center"))`.
            y: The y coordinate. Required unless `x` is a `Position`.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        if isinstance(x, Position):
            if y is not None:
                raise TypeError(
                    "camera.center(): pass either a Position or x, y — not both"
                )
            mapped = x.into_node(self._node)
            cx, cy = mapped.x, mapped.y
        else:
            if y is None:
                raise TypeError(
                    "camera.center(): pass two numbers or a single Position"
                )
            cx, cy = x, y
        with Par():
            self._node._set_attr("camera_x", cx, dur, ease)
            self._node._set_attr("camera_y", cy, dur, ease)
        return self

    def reset(self, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Reset zoom to 1 and re-center on the group's own box center.

        Args:
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._node._set_attr("camera_zoom", 1, dur, ease)
            self._node._set_attr(
                "camera_x", Call.mul(0.5, _effective_width(self._node)), dur, ease
            )
            self._node._set_attr(
                "camera_y", Call.mul(0.5, _effective_height(self._node)), dur, ease
            )
        return self


def _default_camera_x(node):
    return Call.mul(0.5, _effective_width(node))


def _default_camera_y(node):
    return Call.mul(0.5, _effective_height(node))


@beartype
class CameraMixin:
    """Mixin that adds an animatable content-space `.camera` to `Group`/`Scene`.

    `camera` is distinct from `scale()`/`rotate()`: those transform the node
    as a widget (box, layout, hit-testing all move with it). The camera only
    transforms the *content* relative to the node's own fixed box.
    """

    # Unlike pivot_x/pivot_y's wire-inert 0.5 placeholder, these seeds are not
    # inert: `AnimatedValue.set()` bakes whatever is seeded here into the wire
    # as the animation's start keyframe the moment a `dur=` call first touches
    # the attribute (`avalue.py`) - a plain literal 0 would make the very
    # first `camera.zoom()`/`camera.center()` with a `dur=` animate from the
    # top-left corner instead of the true box center. These must compute the
    # same box-center default the engine itself falls back to
    # (`eval_or_else(ctx, |_ctx| Ok(w * 0.5))`, `eval.rs`), mirroring
    # `PositionQueryMixin`'s `Call.auto_x`/`Call.auto_y` factories rather than
    # RotAndScaleMixin's static pivot placeholder.
    _ATTR_DEFAULTS = {
        "camera_zoom": 1,
        "camera_x": _default_camera_x,
        "camera_y": _default_camera_y,
    }

    @property
    def camera(self) -> CameraProxy:
        return CameraProxy(self)


@beartype
class Group(
    NodeWithChildren,
    ContextManagerMixin,
    PositionMixin,
    SizeMixin,
    AlphaMixin,
    ZLevelMixin,
    RotAndScaleMixin,
    CameraMixin,
):
    """A rectangular container node that positions, clips, and transforms its children.

    Supports column and row layout modes and an animatable clipping window.
    Must be used as a context manager to add children.
    """

    kind = "group"

    _ATTR_DEFAULTS = {
        "clip_x": 0,
        "clip_y": 0,
        "clip_w": 1,
        "clip_h": 1,
        "padding_top": 0,
        "padding_right": 0,
        "padding_bottom": 0,
        "padding_left": 0,
    }

    def __init__(self):
        super().__init__()
        self._init_context_manager()
        self._layout = CENTERING_LAYOUT

    def column(
        self, gap: FloatLike = 0, align: FloatLike = 0.5, reserve: bool = True
    ) -> Self:
        """Switch the group to column (vertical) layout.

        Children are stacked vertically with an optional gap and horizontal
        alignment.

        Args:
            gap: Vertical gap between children in pixels.
            align: Horizontal alignment of children within the column.
                ``0.0`` = left, ``0.5`` = center, ``1.0`` = right.
            reserve: If ``True`` (default), inactive children (not yet visible
                or already removed) still occupy their full height in the
                layout so that siblings never shift when items appear or
                disappear.  If ``False``, only currently active children
                contribute to the layout; siblings reposition as items come
                and go.

        Returns:
            self, for method chaining.
        """
        self._layout = ColumnLayout(get_frame(), gap, align, reserve)
        return self

    def row(
        self, gap: FloatLike = 0, align: FloatLike = 0.5, reserve: bool = True
    ) -> Self:
        """Switch the group to row (horizontal) layout.

        Children are placed side by side with an optional gap and vertical
        alignment.

        Args:
            gap: Horizontal gap between children in pixels.
            align: Vertical alignment of children within the row.
                ``0.0`` = top, ``0.5`` = center, ``1.0`` = bottom.
            reserve: If ``True`` (default), inactive children (not yet visible
                or already removed) still occupy their full width in the
                layout so that siblings never shift when items appear or
                disappear.  If ``False``, only currently active children
                contribute to the layout; siblings reposition as items come
                and go.

        Returns:
            self, for method chaining.
        """
        self._layout = RowLayout(get_frame(), gap, align, reserve)
        return self

    def grid(
        self, cols: int, gap: FloatLike = 0, gap_y: FloatLike | None = None
    ) -> Self:
        """Switch the group to grid layout.

        Children are placed row-major (``row = i // cols``, ``col = i % cols``).
        Each column is sized to its widest child, each row to its tallest.
        Children keep their own natural size, anchored at their cell's
        top-left corner — there is no per-cell align/stretch in this layout;
        size cells explicitly if uniform backgrounds are needed (as `Table`
        does).

        Args:
            cols: Number of columns. Must be >= 1.
            gap: Horizontal gap between columns, in pixels. Also used as the
                vertical gap between rows if `gap_y` is left as ``None``.
            gap_y: Vertical gap between rows, in pixels. ``None`` (default)
                reuses `gap` for both axes.

        Returns:
            self, for method chaining.
        """
        if cols < 1:
            raise ValueError("grid(): cols must be >= 1")
        self._layout = GridLayout(
            get_frame(), cols, gap, gap if gap_y is None else gap_y, reserve=True
        )
        return self

    def padding(
        self,
        all: FloatLike | None = None,
        *,
        x: FloatLike | None = None,
        y: FloatLike | None = None,
        top: FloatLike | None = None,
        right: FloatLike | None = None,
        bottom: FloatLike | None = None,
        left: FloatLike | None = None,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set inner spacing between the group's own box and its laid-out
        children. Applies to `Center`/`Column`/`Row`/`Grid` layout alike.

        Most specific wins: `all` is applied first, then `x`/`y`, then the
        named per-side args, each overriding whatever came before within
        this same call. A later call (e.g. ``card.padding(top=32)``) only
        touches the sides it names, leaving the others at their current
        value.

        Args:
            all: Padding for all four sides.
            x: Left and right padding.
            y: Top and bottom padding.
            top: Top padding.
            right: Right padding.
            bottom: Bottom padding.
            left: Left padding.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            if all is not None:
                for side in (
                    "padding_top",
                    "padding_right",
                    "padding_bottom",
                    "padding_left",
                ):
                    self._set_attr(side, all, dur, ease)
            if x is not None:
                self._set_attr("padding_left", x, dur, ease)
                self._set_attr("padding_right", x, dur, ease)
            if y is not None:
                self._set_attr("padding_top", y, dur, ease)
                self._set_attr("padding_bottom", y, dur, ease)
            for name, val in (
                ("padding_top", top),
                ("padding_right", right),
                ("padding_bottom", bottom),
                ("padding_left", left),
            ):
                if val is not None:
                    self._set_attr(name, val, dur, ease)
        return self

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layout"] = self._layout.serialize(serializer)
        return result

    def clip(
        self,
        x: FloatLike | None = None,
        y: FloatLike | None = None,
        w: FloatLike | None = None,
        h: FloatLike | None = None,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Set the clipping window, in one or more axes at once.

        `x`/`w` are relative to the node's own width (``0.0`` = left edge,
        ``1.0`` = right edge); `y`/`h` are relative to its height. Each
        argument left as `None` is untouched.

        Args:
            x: Relative x start of the clipping window, in ``[0.0, 1.0]``.
            y: Relative y start of the clipping window, in ``[0.0, 1.0]``.
            w: Relative width of the clipping window, in ``[0.0, 1.0]``.
            h: Relative height of the clipping window, in ``[0.0, 1.0]``.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.

        A bare call (all four args left as `None`) enables clipping to the
        group's own box without changing the numeric clip window - useful
        e.g. to bound overflow even when the full box is the desired window.
        """
        with Par():
            if x is None and y is None and w is None and h is None:
                self._set_attr("clip_x", self._ATTR_DEFAULTS["clip_x"], dur, ease)
                self._set_attr("clip_y", self._ATTR_DEFAULTS["clip_y"], dur, ease)
                self._set_attr("clip_w", self._ATTR_DEFAULTS["clip_w"], dur, ease)
                self._set_attr("clip_h", self._ATTR_DEFAULTS["clip_h"], dur, ease)
            else:
                if x is not None:
                    self._set_attr("clip_x", x, dur, ease)
                if y is not None:
                    self._set_attr("clip_y", y, dur, ease)
                if w is not None:
                    self._set_attr("clip_w", w, dur, ease)
                if h is not None:
                    self._set_attr("clip_h", h, dur, ease)
        return self

    def hide(
        self,
        direction: Literal["right", "left", "up", "down"] = "right",
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Animate hiding the group with a wipe effect.

        `direction` is the sweep direction the content disappears toward:
        ``"right"``/``"left"`` sweep or shrink the clip window horizontally,
        ``"down"``/``"up"`` do the same vertically.

        Args:
            direction: Sweep direction, one of ``"right"``, ``"left"``,
                ``"up"``, ``"down"``.
            dur: Duration of the animation in seconds. If unset, uses the
                enclosing `anim()` block's default, or is instant if there
                is none.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            if direction == "right":
                self.clip(x=0, dur=0)
                self.clip(x=1, dur=dur, ease=ease)
            elif direction == "left":
                self.clip(w=1, dur=0)
                self.clip(w=0, dur=dur, ease=ease)
            elif direction == "down":
                self.clip(y=0, dur=0)
                self.clip(y=1, dur=dur, ease=ease)
            else:
                self.clip(h=1, dur=0)
                self.clip(h=0, dur=dur, ease=ease)
        return self

    def reveal(
        self,
        direction: Literal["right", "left", "up", "down"] = "right",
        *,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Animate revealing the group with a wipe effect.

        `direction` is the sweep direction the content appears from:
        ``"right"``/``"left"`` expand or sweep the clip window horizontally,
        ``"down"``/``"up"`` do the same vertically.

        Args:
            direction: Sweep direction, one of ``"right"``, ``"left"``,
                ``"up"``, ``"down"``.
            dur: Duration of the animation in seconds. If unset, uses the
                enclosing `anim()` block's default, or is instant if there
                is none.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            if direction == "right":
                self.clip(w=0, dur=0)
                self.clip(w=1, dur=dur, ease=ease)
            elif direction == "left":
                self.clip(x=1, dur=0)
                self.clip(x=0, dur=dur, ease=ease)
            elif direction == "down":
                self.clip(h=0, dur=0)
                self.clip(h=1, dur=dur, ease=ease)
            else:
                self.clip(y=1, dur=0)
                self.clip(y=0, dur=dur, ease=ease)
        return self


@beartype
class Scene(
    NodeWithChildren, ContextManagerMixin, SizeMixin, PositionQueryMixin, CameraMixin
):
    """Top-level container for an animation, defining canvas size and background color.

    Must be used as a context manager (``with Scene(...) as s:``) before adding
    child nodes. Registers itself as a root object upon creation.

    Args:
        width: Canvas width in pixels.
        height: Canvas height in pixels.
        background: Background color (string or `Color` instance).
        flow: If ``True``, the player does not pause at the end of this scene
            and runs straight into the next one.
    """

    kind = "scene"

    # Scene has no z-ordering of its own — this exists purely to terminate
    # every top-level node's inherited z-level walk (`ZLevelMixin`) at a real
    # value instead of the parent chain running out with nothing declared.
    _ATTR_DEFAULTS = {"z": 0}

    def __init__(
        self,
        width: SupportsFloat | None = None,
        height: SupportsFloat | None = None,
        background: str | Color | None = None,
        flow: bool | None = None,
    ):
        reset_scene()
        super().__init__(put_in_context=False)
        self._init_context_manager()
        if width is None:
            width = DEFAULT_SCENE_CONFIG["width"]
        if height is None:
            height = DEFAULT_SCENE_CONFIG["height"]
        if background is None:
            background = DEFAULT_SCENE_CONFIG["background"]
        if flow is None:
            flow = DEFAULT_SCENE_CONFIG["flow"]
        self._add_attr("width", width)
        self._add_attr("height", height)
        self._add_attr("fill_color", background)
        self._id_counter = 0
        self._id = 0
        self._layout = CENTERING_LAYOUT
        self.max_frame = 0
        self.flow = flow
        self.cues = set()
        self._cue_ordinal = 0
        self.notes = []
        ROOT_OBJECTS.get().append(self)

    def __enter__(self):
        return super().__enter__()

    def __exit__(self, *args):
        self.max_frame = end_frame()
        return super().__exit__(*args)

    def background(
        self, value: ColorLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the background color of the scene.

        Args:
            value: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance).
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        if isinstance(value, Gradient):
            raise TypeError("background() does not support gradients")
        self._set_attr("fill_color", Color.parse(value), dur, ease)
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter

    def serialize(self, serializer):
        from .serializer import serialize_expr

        result = {
            "name": self._name,
            "width": serialize_expr(self._attrs["width"]),
            "height": serialize_expr(self._attrs["height"]),
            "background": serialize_expr(self._attrs["fill_color"]),
            "frames": self.max_frame + 1,
        }
        for name in ("camera_zoom", "camera_x", "camera_y"):
            av = self._attrs.get(name)
            if av is not None and not av.is_default:
                result[name] = serialize_expr(av)
        if self.flow:
            result["flow"] = True
        if self.cues:
            result["cues"] = sorted(self.cues)
        if self.notes:
            result["notes"] = [
                [f, seg, t] for f, seg, t in sorted(self.notes, key=lambda n: n[0])
            ]
        if self._children:
            result["children"] = [serializer.add_node(c) for c in self._children]
        result["nodes"] = serializer.nodes
        return result

    def get_scene(self):
        return self

    def update_max_frame(self, frame):
        self.max_frame = max(self.max_frame, frame)


@beartype
class Rect(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin, RotAndScaleMixin):
    """A rectangle shape node with animatable position, size, fill, and stroke.

    Position/size/style/z all come from their mixins' lazy defaults — an
    unsized rect resolves to (0, 0) via `layout.rs`'s default-size catch-all
    for shape kinds, same as the old eager `width(0).height(0)` seed."""

    kind = "rect"
    _ATTR_DEFAULTS = {"radius": 0.0}

    def radius(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the corner radius in pixels, rounding the rect's corners.

        Clamped to `min(w, h) / 2` at render time — an oversized radius
        degrades to a fully-rounded "stadium" shape rather than
        self-intersecting geometry.

        Args:
            value: Corner radius in pixels. `0` (the default) is square corners.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("radius", value, dur, ease)
        return self


@beartype
class Ellipse(
    Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin, RotAndScaleMixin
):
    """An ellipse shape node with animatable position, size, fill, and stroke."""

    kind = "ellipse"


def _resolve_path_point(parent: "Path", x, y, *, method: str):
    """Resolve a `move_to`/`line_to` argument pair into a plain (x, y) pair
    already in `parent`'s own frame - `x` may be a live `Position` (in which
    case `y` must be omitted), or a plain coordinate (in which case `y` is
    required)."""
    if isinstance(x, Position):
        pos = x.into_node(parent)
        return pos.x, pos.y
    if y is None:
        raise TypeError(f"{method}() requires y when x is not a Position")
    return x, y


@beartype
class Path(NodeWithChildren, StyleMixin, ZLevelMixin):
    """A vector path composed of move, line, cubic, and close command nodes.

    Build the shape by calling `move_to`, `line_to`, `cubic_to`, and `close` in
    sequence. Supports crop animations and optional arrowheads via `arrow`.
    """

    kind = "path"

    _ATTR_DEFAULTS = {
        "crop_start": 0.0,
        "crop_end": 1.0,
    }

    def crop(
        self,
        *,
        start: FloatLike | None = None,
        end: FloatLike | None = None,
        dur: Duration = None,
        ease: Easing = None,
    ) -> Self:
        """Crop the path by relative start/end offsets.

        Args:
            start: Relative start offset in ``[0.0, 1.0]``. ``0.0`` keeps the
                full path; ``1.0`` hides it entirely from the start. ``None``
                (default) leaves the start untouched.
            end: Relative end offset in ``[0.0, 1.0]``. ``1.0`` keeps the
                full path; ``0.0`` hides it entirely from the end. ``None``
                (default) leaves the end untouched.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).
        """
        with Par():
            if start is not None:
                self._set_attr("crop_start", start, dur, ease)
            if end is not None:
                self._set_attr("crop_end", end, dur, ease)
        return self

    def draw(self, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Animate the path drawing itself in, from invisible to complete.

        Sugar for snapping `crop_end` to ``0`` at the current frame, then
        animating it to ``1`` over `dur`.

        Args:
            dur: Duration of the animation in seconds. If unset, uses the
                enclosing `anim()` block's default, or is instant if there
                is none.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        self.crop(end=0)
        self.crop(end=1, dur=dur, ease=ease)
        return self

    def move_to(
        self, x: FloatLike | Position, y: FloatLike | None = None
    ) -> "PathMove":
        """Append a move-to command to the path.

        Moves the current drawing position without drawing a line. Use this
        as the first command of a path or to start a new subpath.

        Args:
            x: The target x coordinate, or a `Position` (in which case `y`
                must be omitted).
            y: The target y coordinate. Required unless `x` is a `Position`.

        Returns:
            The newly created `PathMove` node, still animatable via
            `xy()`/`pos()`/`move()`.
        """
        rx, ry = _resolve_path_point(self, x, y, method="move_to")
        p = PathMove(self, rx, ry)
        self._children.append(p)
        return p

    def line_to(
        self, x: FloatLike | Position, y: FloatLike | None = None
    ) -> "PathLine":
        """Append a line-to command to the path.

        Draws a straight line from the current position to the target.

        Args:
            x: The target x coordinate, or a `Position` (in which case `y`
                must be omitted).
            y: The target y coordinate. Required unless `x` is a `Position`.

        Returns:
            The newly created `PathLine` node, still animatable via
            `xy()`/`pos()`/`move()`.
        """
        rx, ry = _resolve_path_point(self, x, y, method="line_to")
        p = PathLine(self, rx, ry)
        self._children.append(p)
        return p

    def cubic_to(
        self,
        x: FloatLike,
        y: FloatLike,
        *,
        c1: tuple[FloatLike, FloatLike] | None = None,
        c2: tuple[FloatLike, FloatLike] | None = None,
    ) -> "PathCubic":
        """Append a cubic Bézier curve command to the path.

        Draws a cubic Bézier segment from the current position to the target,
        shaped by two control points.

        Args:
            x: The target x coordinate.
            y: The target y coordinate.
            c1: Control point 1 as an ``(dx, dy)`` offset relative to the
                segment's start point.
            c2: Control point 2 as an ``(dx, dy)`` offset relative to the
                segment's end point.

        Returns:
            The newly created `PathCubic` node. Use its `c1()`/`c2()` methods
            to animate the control points later.
        """
        p = PathCubic(self, x, y)
        self._children.append(p)
        if c1 is not None:
            p.c1(*c1)
        if c2 is not None:
            p.c2(*c2)
        return p

    def close(self) -> "PathClose":
        """Append a close-path command.

        Draws a straight line back to the start of the current subpath and
        closes it.

        Returns:
            The newly created `PathClose` node.
        """
        p = PathClose(self)
        self._children.append(p)
        return p

    def _get_start_direction(self):
        if len(self._children) < 2:
            return None
        child0 = self._children[0]
        child1 = self._children[1]
        if isinstance(child1, PathCubic):
            vx = child1._get_attr("c1_x")
            vy = child1._get_attr("c1_y")
        else:
            p = child0.at()
            vx = Call.sub(child1._get_attr("x"), p.x)
            vy = Call.sub(child1._get_attr("y"), p.y)
        return (child0, vx, vy)

    def _get_end_direction(self):
        if len(self._children) < 2:
            return None
        child0 = self._children[-1]
        child1 = self._children[-2]
        if isinstance(child0, PathCubic):
            vx = child0._get_attr("c2_x")
            vy = child0._get_attr("c2_y")
        else:
            p = child0.at()
            vx = Call.sub(child1._get_attr("x"), p.x)
            vy = Call.sub(child1._get_attr("y"), p.y)
        return (child0, vx, vy)

    def _create_arrow(self, child, vx, vy, length, width):
        nx = Call.norm(vx, vy)
        ny = Call.norm(vy, vx)

        pos = child.at()
        px = pos.x + nx * length
        py = pos.y + ny * length

        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path()
        path.move_to(px - dy, py + dx)
        path.line_to(pos)
        path.line_to(px + dy, py - dx)
        path.close()
        return path

    def _create_open_arrow(self, child, vx, vy, length, width):
        nx = Call.norm(vx, vy)
        ny = Call.norm(vy, vx)

        pos = child.at()
        px = pos.x + nx * length
        py = pos.y + ny * length

        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path()
        path.move_to(px - dy, py + dx)
        path.line_to(pos)
        path.move_to(px + dy, py - dx)
        path.line_to(pos)
        return path

    def _create_stealth_arrow(self, child, vx, vy, length, width):
        nx = Call.norm(vx, vy)
        ny = Call.norm(vy, vx)

        pos = child.at()
        px = pos.x + nx * length
        py = pos.y + ny * length
        notch_x = pos.x + nx * (length * 0.5)
        notch_y = pos.y + ny * (length * 0.5)

        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path()
        path.move_to(px - dy, py + dx)
        path.line_to(pos)
        path.line_to(px + dy, py - dx)
        path.line_to(notch_x, notch_y)
        path.close()
        return path

    def _create_bar_arrow(self, child, vx, vy, length, width):
        nx = Call.norm(vx, vy)
        ny = Call.norm(vy, vx)

        pos = child.at()
        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path()
        path.move_to(pos.x - dy, pos.y + dx)
        path.line_to(pos.x + dy, pos.y - dx)
        return path

    def _create_dot_arrow(self, child, vx, vy, length, width):
        pos = child.at()
        dot = Ellipse()
        dot.size(width, width)
        dot.xy(pos.x - width * 0.5, pos.y - width * 0.5)
        return dot

    def arrow(
        self,
        placement: Literal["start", "end"] = "end",
        *,
        style: Literal["triangle", "open", "stealth", "bar", "dot"] = "triangle",
        length: FloatLike | None = None,
        width: FloatLike | None = None,
    ) -> Union["Path", "Ellipse", None]:
        """Add an arrowhead at one end of the path.

        The arrowhead is created as a separate sibling node (a `Path` for
        every style except ``"dot"``, which is an `Ellipse`). For styles that
        visually overlap the shaft (``"triangle"``, ``"stealth"``, ``"dot"``),
        ``crop_start``/``crop_end`` is automatically adjusted to prevent the
        main path from poking out through the head.

        Args:
            placement: Which end of the path receives the arrowhead.
                ``"start"`` places it at the first point; ``"end"`` at the last.
            style: The arrowhead shape — ``"triangle"`` (filled, default),
                ``"open"`` (two stroked lines forming a V), ``"stealth"``
                (concave filled head), ``"bar"`` (perpendicular stroke), or
                ``"dot"`` (filled circle).
            length: Length of the arrowhead in pixels. Defaults to three times
                the current stroke width. Unused by ``"bar"``.
            width: Base width of the arrowhead in pixels. Defaults to the same
                value as ``length``.

        Returns:
            The new node representing the arrowhead, or ``None`` if the path
            has fewer than two points.
        """
        if placement == "start":
            dir = self._get_start_direction()
        else:
            dir = self._get_end_direction()
        if dir is None:
            return None
        if length is None:
            length = self._get_attr("stroke_width") * 3
        if width is None:
            width = length

        builder = {
            "triangle": self._create_arrow,
            "open": self._create_open_arrow,
            "stealth": self._create_stealth_arrow,
            "bar": self._create_bar_arrow,
            "dot": self._create_dot_arrow,
        }[style]
        head = builder(*dir, length, width)
        head.alpha(self._get_attr("alpha"))
        if style in ("triangle", "stealth", "dot"):
            head.fill(self._get_attr("stroke_color"))
        else:
            head.stroke(self._get_attr("stroke_color"), self._get_attr("stroke_width"))

        crop_len = {
            "triangle": length * 0.5,
            "stealth": length * 0.5,
            "dot": width * 0.5,
        }.get(style)
        if crop_len is not None:
            crop_len = to_expr(crop_len)
            if placement == "start":
                self.crop(start=crop_len / Call.path_length(self))
            else:
                self.crop(end=to_expr(1.0) - crop_len / Call.path_length(self))
        return head


class PathMove(Node, PositionMixin):
    """A move-to path command that repositions the drawing cursor without drawing."""

    kind = "move"

    def __init__(self, parent, x, y):
        super().__init__(put_in_context=False, parent=parent)
        # x/y are always required here (never "auto") — set directly rather
        # than through PositionMixin's lazy default.
        self._add_attr("x", x)
        self._add_attr("y", y)


class PathLine(Node, PositionMixin):
    """A line-to path command that draws a straight line to its target position."""

    kind = "line"

    def __init__(self, parent, x, y):
        super().__init__(put_in_context=False, parent=parent)
        self._add_attr("x", x)
        self._add_attr("y", y)


class PathClose(Node):
    """A close-path command that draws a straight line back to the start of the current subpath."""

    def __init__(self, parent):
        super().__init__(put_in_context=False, parent=parent)

    kind = "close"


@beartype
class PathCubic(Node, PositionMixin):
    """A cubic Bézier curve command with two animatable control points (`c1` and `c2`)."""

    kind = "cubic"

    _ATTR_DEFAULTS = {"c1_x": 0, "c1_y": 0, "c2_x": 0, "c2_y": 0}

    def __init__(self, parent, x, y):
        super().__init__(put_in_context=False, parent=parent)
        self._add_attr("x", x)
        self._add_attr("y", y)

    def c1(
        self, dx: FloatLike, dy: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set control point 1, relative to the segment's start point.

        Args:
            dx: Relative x offset of control point 1 in pixels.
            dy: Relative y offset of control point 1 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._set_attr("c1_x", dx, dur, ease)
            self._set_attr("c1_y", dy, dur, ease)
        return self

    def c2(
        self, dx: FloatLike, dy: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set control point 2, relative to the segment's end point.

        Args:
            dx: Relative x offset of control point 2 in pixels.
            dy: Relative y offset of control point 2 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"in_out"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._set_attr("c2_x", dx, dur, ease)
            self._set_attr("c2_y", dy, dur, ease)
        return self


@beartype
class Image(
    NodeWithChildren,
    PositionMixin,
    SizeMixin,
    KeepAspectMixin,
    ZLevelMixin,
    AlphaMixin,
    RotAndScaleMixin,
):
    """An image node that loads and displays a raster image file.

    Supports optional aspect-ratio preservation and per-layer visibility control
    for ORA (OpenRaster) files via `layer`.
    """

    kind = "image"

    def __init__(self, image_path: str | os.PathLike, keep_aspect: bool = True):
        super().__init__()
        self._add_attr("path", None)
        self.file_name(image_path)
        # No setter exists for keep_aspect (it's constructor-only), so it must
        # always be explicit — there's nothing that would ever clear an
        # `is_default` marking on it, unlike every lazy-defaulted attribute.
        self._add_attr("keep_aspect", keep_aspect)

    def layer(self, name: str) -> "ImageLayer":
        """Get or create a named layer from the image.

        If a layer with the given name already exists it is returned unchanged.
        Otherwise a new `ImageLayer` child node is created and appended.

        Args:
            name: The name of the layer to retrieve or create.

        Returns:
            The `ImageLayer` node for the specified layer name.
        """
        for child in self._children:
            if child.layer_name == name:
                return child
        layer = ImageLayer(self, name)
        self._children.append(layer)
        return layer

    def file_name(self, image_path: str | os.PathLike) -> Self:
        """Set the file path of the image to load.

        The path is resolved to an absolute path before storing. Raises if the
        file does not exist.

        Args:
            image_path: Path to the image file (absolute or relative to the
                current working directory).

        Returns:
            self, for method chaining.

        Raises:
            Exception: If the resolved path does not point to an existing file.
        """
        image_path = os.path.abspath(image_path)
        if not os.path.exists(image_path):
            raise Exception(f"Path '{image_path}' does not exists.")
        self._set_attr("path", str(image_path))
        return self


@beartype
class ImageLayer(
    NodeWithChildren,
    PositionMixin,
    SizeMixin,
    ZLevelMixin,
    AlphaMixin,
    RotAndScaleMixin,
):
    """A named layer within an ORA image, rendered as a child of an `Image` node."""

    kind = "layer"

    def __init__(self, parent: Image, layer_name: str):
        super().__init__(put_in_context=False, parent=parent)
        self.layer_name = layer_name

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layer_name"] = self.layer_name
        return result
