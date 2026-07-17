from typing import Union, Literal, Self, SupportsFloat
from beartype import beartype
import os

from .types import ColorLike, FloatLike
from .layout import CENTERING_LAYOUT, ColumnLayout, RowLayout
from .position import Position
from .info import get_info
from .animtime import Duration, Easing
from .sentinels import (
    DEFAULT,
    INHERITED_VALUE,
    DefaultMarker,
    RelValue,
    rel,
    resolve_rel,
)
from .aobject import AnimatedObject, get_frame
from .avalue import AnimatedValue
from .color import Color
from .exprs import (
    Call,
    to_expr,
)
from .ctxvars import (
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

    # v1 attribute name -> v2 wire key, for the handful that were renamed
    # (api-v2-impl.md §A.5); everything else keeps its Python-internal name.
    # Python method names are unaffected either way — this only changes what
    # gets written to JSON.
    _WIRE_KEY = {
        "width": "w",
        "height": "h",
        "fill_color": "fill",
        "stroke_color": "stroke",
        "z_level": "z",
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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("alpha", value, dur, ease)
        return self

    def fade_in(self, *, dur: Duration = 1, ease: Easing = None) -> Self:
        """Animate a fade-in effect by transitioning alpha from 0 to 1.

        Sets the node alpha to ``0`` at the current frame and animates it to
        ``1`` over the given duration.

        Args:
            dur: Duration of the animation in seconds.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            self.alpha(0)
            self.alpha(1, dur=dur, ease=ease)
        return self

    def fade_out(self, *, dur: Duration = 1, ease: Easing = None) -> Self:
        """Animate a fade-out effect by transitioning alpha to 0.

        Animates the node alpha from ``1`` down to ``0`` over the
        given duration.

        Args:
            dur: Duration of the animation in seconds.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            self.alpha(1)
            self.alpha(0, dur=dur, ease=ease)
        return self


@beartype
class ZLevelMixin:
    """Mixin that adds a `z_level` attribute for controlling rendering order.
    Always inherited-from-parent when unset — there is no "own" variant."""

    _ATTR_DEFAULTS = {"z_level": INHERITED_VALUE}

    def z_level(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the z-level (rendering order) of the node.

        Nodes with higher z-levels are drawn on top of nodes with lower values.

        Args:
            value: The z-level. Higher values appear in front.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("z_level", value, dur, ease)
        return self


@beartype
class SizeMixin:
    """Mixin that adds animatable `width` and `height` attributes to a node.
    Defaults to the layout-computed size when unset — for shape kinds with no
    layout concept of their own (rect/ellipse) the engine's fallback already
    resolves this to 0 either way (`layout.rs::default_width/height`'s
    catch-all), so there's no need for those kinds to special-case it."""

    _ATTR_DEFAULTS = {
        "width": Call.default_width,
        "height": Call.default_height,
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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        return self.size(rel(1), rel(1), dur=dur, ease=ease)


@beartype
class PositionMixin:
    """Mixin that adds animatable `x` / `y` position and alignment helpers to
    a node. Defaults to the layout-computed position when unset. Path
    commands (move/line/cubic) also mix this in for their `.x()`/`.y()`/
    `.xy()` setters, but their coordinates are always required constructor
    arguments — `Path.move_to`/`line_to`/`cubic_to` seed them directly via
    `_add_attr`, so this lazy default never actually gets consulted there."""

    _ATTR_DEFAULTS = {
        "x": Call.default_x,
        "y": Call.default_y,
    }

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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        value = (
            Call.default_x(self) if px is DEFAULT else resolve_rel(px, self, "width")
        )
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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        value = (
            Call.default_y(self) if px is DEFAULT else resolve_rel(px, self, "height")
        )
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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        parent = self.parent_group()
        sized = isinstance(self, SizeMixin)
        with Par():
            if x is not None:
                if sized:
                    new_x = Call.mul(
                        Call.sub(parent._get_attr("width"), self._get_attr("width")), x
                    )
                else:
                    new_x = Call.mul(parent._get_attr("width"), x)
                self._set_attr("x", new_x, dur, ease)
            if y is not None:
                if sized:
                    new_y = Call.mul(
                        Call.sub(parent._get_attr("height"), self._get_attr("height")),
                        y,
                    )
                else:
                    new_y = Call.mul(parent._get_attr("height"), y)
                self._set_attr("y", new_y, dur, ease)
        return self

    def pos(
        self, position: Position, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the position of the node using a `Position` object.

        Args:
            position: The target position, resolved relative to the parent node.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
            self._move_attr("x", dx, dur, ease)
            self._move_attr("y", dy, dur, ease)
        return self

    def get_pos(self, align_x=0, align_y=0) -> Position:
        """Return the current position of the node as a `Position`.

        Args:
            align_x: Horizontal alignment offset within the node. ``0.0``
                returns the left edge; ``0.5`` the center; ``1.0`` the right edge.
            align_y: Vertical alignment offset within the node. ``0.0``
                returns the top edge; ``0.5`` the center; ``1.0`` the bottom edge.

        Returns:
            A `Position` representing the node's resolved coordinates.
        """
        x = self._get_attr("x")
        y = self._get_attr("y")
        if isinstance(self, SizeMixin):
            if align_x != 0:
                x = x + self._get_attr("width") * align_x
            if align_y != 0:
                y = y + self._get_attr("height") * align_y
        return Position(self._parent, x, y)

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
            ease: Optional easing curve (``"linear"`` default).
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

    def color(
        self, value: ColorLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the fill color of the node.

        Args:
            value: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance).
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("fill_color", Color.parse(value), dur, ease)
        return self

    def stroke_color(
        self, value: ColorLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the stroke (outline) color of the node.

        Args:
            value: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance).
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("stroke_color", Color.parse(value), dur, ease)
        return self

    def stroke_width(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ):
        """Set the stroke width of the node in pixels.

        Args:
            value: The stroke width in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("stroke_width", value, dur, ease)
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
    }


@beartype
class InheritedStyleMixin(AlphaMixin, StyleMethods):
    """Like `StyleMixin`, but fill/stroke/stroke_width/alpha default to the
    parent's resolved value when unset instead of a literal constant — used
    by text runs (`t_group`/`t_span`), which cascade style from their
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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

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
            ease: Optional easing curve (``"linear"`` default).

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

        The pivot is controlled by ``pivot_x`` / ``pivot_y`` attributes,
        which default to the node's center (``0.5``, ``0.5``).

        Args:
            value: Rotation angle in degrees, clockwise.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self._set_attr("rotation", value, dur, ease)
        return self


@beartype
class Group(
    NodeWithChildren,
    ContextManagerMixin,
    PositionMixin,
    SizeMixin,
    AlphaMixin,
    ZLevelMixin,
    RotAndScaleMixin,
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
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Par():
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
        dur: Duration = 1,
        ease: Easing = None,
    ) -> Self:
        """Animate hiding the group with a wipe effect.

        `direction` is the sweep direction the content disappears toward:
        ``"right"``/``"left"`` sweep or shrink the clip window horizontally,
        ``"down"``/``"up"`` do the same vertically.

        Args:
            direction: Sweep direction, one of ``"right"``, ``"left"``,
                ``"up"``, ``"down"``.
            dur: Duration of the animation in seconds.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            if direction == "right":
                self.clip(x=0)
                self.clip(x=1, dur=dur, ease=ease)
            elif direction == "left":
                self.clip(w=1)
                self.clip(w=0, dur=dur, ease=ease)
            elif direction == "down":
                self.clip(y=0)
                self.clip(y=1, dur=dur, ease=ease)
            else:
                self.clip(h=1)
                self.clip(h=0, dur=dur, ease=ease)
        return self

    def reveal(
        self,
        direction: Literal["right", "left", "up", "down"] = "right",
        *,
        dur: Duration = 1,
        ease: Easing = None,
    ) -> Self:
        """Animate revealing the group with a wipe effect.

        `direction` is the sweep direction the content appears from:
        ``"right"``/``"left"`` expand or sweep the clip window horizontally,
        ``"down"``/``"up"`` do the same vertically.

        Args:
            direction: Sweep direction, one of ``"right"``, ``"left"``,
                ``"up"``, ``"down"``.
            dur: Duration of the animation in seconds.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        with Seq():
            if direction == "right":
                self.clip(w=0)
                self.clip(w=1, dur=dur, ease=ease)
            elif direction == "left":
                self.clip(x=1)
                self.clip(x=0, dur=dur, ease=ease)
            elif direction == "down":
                self.clip(h=0)
                self.clip(h=1, dur=dur, ease=ease)
            else:
                self.clip(y=1)
                self.clip(y=0, dur=dur, ease=ease)
        return self


@beartype
class Scene(NodeWithChildren, ContextManagerMixin, SizeMixin):
    """Top-level container for an animation, defining canvas size and background color.

    Must be used as a context manager (``with Scene(...) as s:``) before adding
    child nodes. Registers itself as a root object upon creation.

    Args:
        width: Canvas width in pixels.
        height: Canvas height in pixels.
        color: Background color (string or `Color` instance).
        cue_at_start: If ``True``, frame 0 is automatically added as a cue point.
    """

    kind = "scene"

    # Scene has no z-ordering of its own — this exists purely to terminate
    # every top-level node's inherited z_level walk (`ZLevelMixin`) at a real
    # value instead of the parent chain running out with nothing declared.
    _ATTR_DEFAULTS = {"z_level": 0}

    def __init__(
        self,
        width: SupportsFloat | None = None,
        height: SupportsFloat | None = None,
        color: str | Color | None = None,
        cue_at_start: bool | None = None,
    ):
        reset_scene()
        super().__init__(put_in_context=False)
        self._init_context_manager()
        if width is None:
            width = DEFAULT_SCENE_CONFIG["width"]
        if height is None:
            height = DEFAULT_SCENE_CONFIG["height"]
        if color is None:
            color = DEFAULT_SCENE_CONFIG["color"]
        if cue_at_start is None:
            cue_at_start = DEFAULT_SCENE_CONFIG["cue_at_start"]
        self._add_attr("width", width)
        self._add_attr("height", height)
        self._add_attr("fill_color", color)
        self._id_counter = 0
        self._id = 0
        self._layout = CENTERING_LAYOUT
        self.max_frame = 0
        self.cue_at_start = cue_at_start
        self.cues = set()
        ROOT_OBJECTS.get().append(self)

    def __enter__(self):
        super().__enter__()

    def __exit__(self, *args):
        self.max_frame = end_frame()
        return super().__exit__(*args)

    def color(
        self, value: ColorLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set the background color of the scene.

        Args:
            value: Any color value accepted by `Color.parse` (e.g. a hex
                string, an RGB tuple, or a `Color` instance).
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
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
        if self.cue_at_start:
            self.cues.add(0)
        if self.cues:
            result["cues"] = sorted(self.cues)
        if self._children:
            result["children"] = [serializer.add_node(c) for c in self._children]
        result["nodes"] = serializer.nodes
        return result

    def get_scene(self):
        return self

    def update_max_frame(self, frame):
        self.max_frame = max(self.max_frame, frame)


@beartype
class Rect(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    """A rectangle shape node with animatable position, size, fill, and stroke.

    Position/size/style/z all come from their mixins' lazy defaults — an
    unsized rect resolves to (0, 0) via `layout.rs`'s default-size catch-all
    for shape kinds, same as the old eager `width(0).height(0)` seed."""

    kind = "rect"


@beartype
class Ellipse(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    """An ellipse shape node with animatable position, size, fill, and stroke."""

    kind = "ellipse"


@beartype
class Path(NodeWithChildren, StyleMixin, ZLevelMixin):
    """A vector path composed of move, line, cubic, and close command nodes.

    Build the shape by calling `move_to`, `line_to`, `cubic_to`, and `close` in
    sequence. Supports crop animations and optional arrowheads via
    `triangle_arrow`.
    """

    kind = "path"

    _ATTR_DEFAULTS = {
        "crop_start": 0.0,
        "crop_end": 1.0,
    }

    def _prev_coords(self):
        if self._children:
            c = self._children[0]
            return (c._get_attr("y"), c._get_attr("y"))
        else:
            return (0, 0)

    def crop_start(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Crop the path from its start.

        Args:
            value: Relative start offset in ``[0.0, 1.0]``. ``0.0`` keeps the
                full path; ``1.0`` hides it entirely from the start.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("crop_start", value, dur, ease)
        return self

    def crop_end(
        self, value: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Crop the path from its end.

        Args:
            value: Relative end offset in ``[0.0, 1.0]``. ``1.0`` keeps the
                full path; ``0.0`` hides it entirely from the end.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("crop_end", value, dur, ease)
        return self

    def move_to(self) -> "PathMove":
        """Append a move-to command to the path.

        Moves the current drawing position without drawing a line. Use this
        as the first command of a path or to start a new subpath.

        Returns:
            The newly created `PathMove` node for setting the target position.
        """
        p = PathMove(self, *self._prev_coords())
        self._children.append(p)
        return p

    def line_to(self) -> "PathLine":
        """Append a line-to command to the path.

        Draws a straight line from the current position to the target.

        Returns:
            The newly created `PathLine` node for setting the target position.
        """
        p = PathLine(self, *self._prev_coords())
        self._children.append(p)
        return p

    def cubic_to(self) -> "PathCubic":
        """Append a cubic Bézier curve command to the path.

        Draws a cubic Bézier segment from the current position to the target,
        shaped by two control points.

        Returns:
            The newly created `PathCubic` node. Use its ``c1_xy`` / ``c2_xy``
            methods to set the control points and ``xy`` to set the endpoint.
        """
        p = PathCubic(self, *self._prev_coords())
        self._children.append(p)
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
            p = child0.get_pos()
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
            p = child0.get_pos()
            vx = Call.sub(child1._get_attr("x"), p.x)
            vy = Call.sub(child1._get_attr("y"), p.y)
        return (child0, vx, vy)

    def _create_arrow(self, child, vx, vy, length, width):
        nx = Call.norm(vx, vy)
        ny = Call.norm(vy, vx)

        pos = child.get_pos()
        px = pos.x + nx * length
        py = pos.y + ny * length

        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path()
        path.move_to().xy(px - dy, py + dx)
        path.line_to().pos(pos)
        path.line_to().xy(px + dy, py - dx)
        path.close()
        return path

    def triangle_arrow(
        self,
        placement: Literal["start", "end"] = "end",
        *,
        length: FloatLike | None = None,
        width: FloatLike | None = None,
    ) -> "Path":
        """Add a filled triangle arrowhead at one end of the path.

        The arrowhead is created as a separate sibling `Path` node. To prevent
        the main path from overlapping the arrowhead, ``crop_start`` or
        ``crop_end`` is automatically adjusted.

        Args:
            placement: Which end of the path receives the arrowhead.
                ``"start"`` places it at the first point; ``"end"`` at the last.
            length: Length of the arrowhead in pixels. Defaults to three times
                the current stroke width.
            width: Base width of the arrowhead in pixels. Defaults to the same
                value as ``length``.

        Returns:
            The new `Path` node representing the arrowhead, or ``None`` if the
            path has fewer than two points.
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
        path = self._create_arrow(*dir, length, width)
        path.color(self._get_attr("stroke_color"))

        length = to_expr(length)

        if placement == "start":
            self.crop_start((length * 0.5) / Call.path_length(self))
        else:
            self.crop_end(to_expr(1.0) - (length * 0.5) / Call.path_length(self))
        return path


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

    def c1_x(self, px: FloatLike, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Set the x coordinate of control point 1, relative to the segment's start point.

        Args:
            px: Relative x offset of control point 1 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("c1_x", px, dur, ease)
        return self

    def c1_y(self, px: FloatLike, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Set the y coordinate of control point 1, relative to the segment's start point.

        Args:
            px: Relative y offset of control point 1 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("c1_y", px, dur, ease)
        return self

    def c2_x(self, px: FloatLike, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Set the x coordinate of control point 2, relative to the segment's end point.

        Args:
            px: Relative x offset of control point 2 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("c2_x", px, dur, ease)
        return self

    def c2_y(self, px: FloatLike, *, dur: Duration = None, ease: Easing = None) -> Self:
        """Set the y coordinate of control point 2, relative to the segment's end point.

        Args:
            px: Relative y offset of control point 2 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).
        """
        self._set_attr("c2_y", px, dur, ease)
        return self

    def c1_xy(
        self, x: FloatLike, y: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set both coordinates of control point 1, relative to the segment's start point.

        Args:
            x: Relative x offset of control point 1 in pixels.
            y: Relative y offset of control point 1 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self.c1_x(x, dur=dur, ease=ease)
        self.c1_y(y, dur=dur, ease=ease)
        return self

    def c2_xy(
        self, x: FloatLike, y: FloatLike, *, dur: Duration = None, ease: Easing = None
    ) -> Self:
        """Set both coordinates of control point 2, relative to the segment's end point.

        Args:
            x: Relative x offset of control point 2 in pixels.
            y: Relative y offset of control point 2 in pixels.
            dur: Optional duration for animation.
            ease: Optional easing curve (``"linear"`` default).

        Returns:
            self, for method chaining.
        """
        self.c2_x(x, dur=dur, ease=ease)
        self.c2_y(y, dur=dur, ease=ease)
        return self


@beartype
class Image(NodeWithChildren, PositionMixin, SizeMixin, ZLevelMixin, AlphaMixin):
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
class ImageLayer(NodeWithChildren, PositionMixin, SizeMixin, ZLevelMixin, AlphaMixin):
    """A named layer within an ORA image, rendered as a child of an `Image` node."""

    kind = "layer"

    def __init__(self, parent: Image, layer_name: str):
        super().__init__(put_in_context=False, parent=parent)
        self.layer_name = layer_name

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layer_name"] = self.layer_name
        return result
