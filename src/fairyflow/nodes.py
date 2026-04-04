from typing import Union, Literal
import os


from .layout import CENTERING_LAYOUT, ColumnLayout, RowLayout
from .position import Position
from .info import get_info
from .aobject import AnimatedObject, get_frame
from .color import Color
from .exprs import (
    expr_default_height,
    expr_default_width,
    expr_default_x,
    expr_default_y,
    expr_follow_path_x,
    expr_follow_path_y,
    expr_mul,
    expr_sub,
)
from .ctxvars import (
    fctx,
    get_current_node,
    ROOT_OBJECTS,
    set_current_node,
    set_frame,
)
from .config import DEFAULT_SCENE_CONFIG, FPS


class Node(AnimatedObject):
    def __init__(self, parent: Union[None, "Group"], frame: int):
        super().__init__(frame)
        self._parent = parent        
        if parent:
            self._id = parent._new_id()
        else:
            self._id = 0            
        self.info = get_info(self._id)

    def parent_chain(self) -> list["Group"]:
        result = []
        node = self
        while node is not None:
            if isinstance(node, Group):
                result.append(node)
            node = node._parent
        return result

    def parent_group(self):
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

    def serialize(self, serializer):
        from .serializer import serialize_expr

        result = {"kind": self.kind, "id": self._id}
        if self._start > 0:
            result["start"] = self._start
        if self._end is not None:
            result["end"] = self._end
        attrs = self._attrs
        for name in attrs:
            v = attrs[name]
            if not v.is_single_value():
                serializer.add_av(attrs[name])
            result[name] = serialize_expr(v)
        return result

    def _get_parent(self):
        if self._parent is None:
            raise Exception("Node does not have parent")
        return self._parent
    
    def get_scene(self):
        return self._parent.get_scene()

    def __repr__(self):
        return f"<{self.kind} id={self._id}>"


class AlphaMixin:
    def _init_alpha(self):
        self._add_attr("alpha", 1)

    def _init_alpha_from_parent(self):
        self._add_from_parent("alpha")

    def alpha(self, value):
        self._set_attr("alpha", value)
        return self


class ZLevelMixin:
    def _init_z(self):
        self._add_from_parent("z_level", 0)

    def z_level(self, value):
        self._set_attr("z_level", value)
        return self


class SizeMixin:
    def _init_size(self, width=None, height=None):
        if width is None:
            width = expr_default_width(self)
        if height is None:
            height = expr_default_height(self)
        self._add_attr("width", width)
        self._add_attr("height", height)

    def width(self, value):
        self._set_attr("width", value)
        return self

    def height(self, value):
        self._set_attr("height", value)
        return self

    def size(self, width, height):
        self.width(width)
        self.height(height)
        return self


class PositionMixin:
    def _init_position(self, x=None, y=None):
        if x is None:
            x = expr_default_x(self)
        if y is None:
            y = expr_default_y(self)
        self._add_attr("x", x)
        self._add_attr("y", y)

    def x(self, px):
        self._set_attr("x", px)
        return self

    def y(self, px):
        self._set_attr("y", px)
        return self

    def xy(self, x, y):
        self.x(x)
        self.y(y)
        return self

    def align_x(self, value):
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = expr_mul(
                expr_sub(parent._get_attr("width"), self._get_attr("width")), value
            )
        else:
            new_value = expr_mul(parent._get_attr("width"), value)
        self._set_attr("x", new_value)
        return self

    def align_y(self, value):
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = expr_mul(
                expr_sub(parent._get_attr("height"), self._get_attr("height")), value
            )
        else:
            new_value = expr_mul(parent._get_attr("height"), value)
        self._set_attr("y", new_value)
        return self

    def pos(self, position: Position):
        position = position.into_node(self._parent)
        self._set_attr("x", position.x)
        self._set_attr("y", position.y)
        return self

    def move(self, dx, dy):
        self._move_attr("x", dx)
        self._move_attr("y", dy)
        return self

    def get_pos(self) -> Position:
        x = self._get_attr("x")
        y = self._get_attr("y")
        return Position(self._parent, x, y)
    
    def follow_path(self, path: "Path", *, frames=None, time=None):
        assert isinstance(path, Path)
        if frames is None:
            if time is None:
                time = 1
            frames = FPS * time
        start = get_frame()
        end = start + frames
        x = expr_follow_path_x(path, start, end)
        y = expr_follow_path_y(path, start, end)
        if self._has_attr("width"):
            x = x - expr_mul(self._get_attr("width"), 0.5)
            y = y - expr_mul(self._get_attr("height"), 0.5)
        self.xy(x, y)
        with fctx():
            set_frame(end)
            self.hold()
        return self


class StyleMixin(AlphaMixin):
    def _init_style(self):
        self._add_attr("fill_color", None)
        self._add_attr("stroke_color", None)
        self._add_attr("stroke_width", 1)
        self._init_alpha()

    def _init_style_from_parent(self):
        self._add_from_parent("fill_color")
        self._add_from_parent("stroke_color")
        self._add_from_parent("stroke_width")
        self._init_alpha_from_parent()

    def color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self

    def stroke_color(self, value: str):
        self._set_attr("stroke_color", Color.parse(value))
        return self

    def stroke_width(self, value: float):
        self._set_attr("stroke_width", value)
        return self


class NodeWithChildren(Node):
    def __init__(self, parent, frame, children=None):
        super().__init__(parent, frame)
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

    # def build(self, ctx):
    #     result = super().build(ctx)
    #     if self._children:
    #         result["children"] = [child.build(ctx) for child in self._children]
    #     return result

    # def key_frames(self, out: set):
    #     super().key_frames(out)
    #     for child in self._children:
    #         child.key_frames(out)


class ContextManagerMixin:
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


class RotAndScaleMixin:
    def _init_rot_and_scale(self):
        self._add_attr("rotation", 0)
        self._add_attr("pivot_x", 0.5)
        self._add_attr("pivot_y", 0.5)
        self._add_attr("scale_x", 1)
        self._add_attr("scale_y", 1)

    def scale_x(self, value):
        self._set_attr("scale_x", value)
        return self

    def scale_y(self, value):
        self._set_attr("scale_y", value)
        return self

    def scale(self, value):
        self.scale_x(value)
        self.scale_y(value)
        return self

    def rotate(self, value):
        self._set_attr("rotation", value)
        return self


class Group(
    NodeWithChildren,
    ContextManagerMixin,
    PositionMixin,
    SizeMixin,
    AlphaMixin,
    ZLevelMixin,
    RotAndScaleMixin,
):
    kind = "group"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_context_manager()
        self._init_size()
        self._init_alpha()
        self._init_z()
        self._init_rot_and_scale()

        self._init_position()
        self._layout = CENTERING_LAYOUT

    def column(self, gap=0, align=0.5):
        self._layout = ColumnLayout(get_frame(), gap, align)
        return self

    def row(self, gap=0, align=0.5):
        self._layout = RowLayout(get_frame(), gap, align)
        return self

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layout"] = self._layout.serialize(serializer)
        return result


class Scene(NodeWithChildren, ContextManagerMixin, SizeMixin):
    kind = "scene"

    def __init__(self, width, height, color, cue_at_end):
        super().__init__(None, 0)
        self._init_context_manager()
        if width is None:
            width = DEFAULT_SCENE_CONFIG["width"]
        if height is None:
            height = DEFAULT_SCENE_CONFIG["height"]
        if color is None:
            color = DEFAULT_SCENE_CONFIG["color"]
        if cue_at_end is None:
            cue_at_end = DEFAULT_SCENE_CONFIG["cue_at_end"]            
        self._init_size(width, height)
        self._add_attr("fill_color", color)
        self._id_counter = 0
        self._layout = CENTERING_LAYOUT
        self.name = None        
        self.max_frame = 0
        self.cue_at_end = cue_at_end
        self.cues = set()

    def __enter__(self):
        super().__enter__()

    def color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter
    
    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["name"] = self.name
        result["frames"] = self.max_frame + 1
        if self.cue_at_end:
            self.cues.add(self.max_frame)
        if self.cues:
            result["cues"] = sorted(self.cues)
        return result
    
    def get_scene(self):
        return self
    
    def update_max_frame(self, frame):
        self.max_frame = max(self.max_frame, frame)


class Rect(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    kind = "rect"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_size(0, 0)
        self._init_style()
        self._init_z()
        self._init_position()


class Ellipse(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    kind = "ellipse"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_size(0, 0)
        self._init_style()
        self._init_z()
        self._init_position()


class Path(NodeWithChildren, StyleMixin, ZLevelMixin):
    kind = "path"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_style()
        self._init_z()

    def _prev_coords(self):
        if self._children:
            c = self._children[0]
            return (c._get_attr("y"), c._get_attr("y"))
        else:
            return (0, 0)

    def move_to(self):
        p = PathMove(self, get_frame(), *self._prev_coords())
        self._children.append(p)
        return p

    def line_to(self):
        p = PathLine(self, get_frame(), *self._prev_coords())
        self._children.append(p)
        return p

    def cubic_to(self):
        p = PathCubic(self, get_frame(), *self._prev_coords())
        self._children.append(p)
        return p
    
    def close(self):
        p = PathClose(self, get_frame())
        self._children.append(p)
        return p
    

class PathMove(Node, PositionMixin):
    kind = "move"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)


class PathLine(Node, PositionMixin):
    kind = "line"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)


class PathClose(Node):
    kind = "close"


class PathCubic(Node, PositionMixin):
    kind = "cubic"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)
        self._add_attr("c1_x", 0)
        self._add_attr("c1_y", 0)
        self._add_attr("c2_x", 0)
        self._add_attr("c2_y", 0)

    def c1_x(self, px):
        """Set x-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_x", px)

    def c1_y(self, px):
        """Set y-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_y", px)

    def c2_x(self, px):
        """Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_x", px)

    def c2_y(self, px):
        """Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_y", px)

    def c1_xy(self, x, y):
        self.c1_x(x)
        self.c1_y(y)
        return self

    def c2_xy(self, x, y):
        self.c2_x(x)
        self.c2_y(y)
        return self


class Image(NodeWithChildren, PositionMixin, SizeMixin, ZLevelMixin, AlphaMixin):
    kind = "image"

    def __init__(self, parent, frame, image_path, keep_aspect):
        super().__init__(parent, frame)
        self._init_size()
        self._init_z()
        self._init_position()
        self._init_alpha()
        self._add_attr("path", None)
        self.path(image_path)
        self._add_attr("keep_aspect", keep_aspect)

    def layer(self, name):
        for child in self._children:
            if child.layer_name == name:
                return child
        layer = ImageLayer(self, self._start, name)
        self._children.append(layer)
        return layer

    def path(self, image_path):
        image_path = os.path.abspath(image_path)
        if not os.path.exists(image_path):
            raise Exception(f"Path '{image_path}' does not exists.")
        self._set_attr("path", image_path)
        return self


class ImageLayer(NodeWithChildren, PositionMixin, SizeMixin, ZLevelMixin, AlphaMixin):
    kind = "layer"

    def __init__(self, parent, frame, layer_name):
        super().__init__(parent, frame)
        self.layer_name = layer_name
        self._init_size()
        self._init_z()
        self._init_position()
        self._init_alpha()

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layer_name"] = self.layer_name
        return result


def make_node(cls, *args):
    current_node = get_current_node()
    if current_node is None:
        raise Exception("Element created out of context of a parent ndoe")
    item = cls(current_node, get_frame(), *args)
    current_node._children.append(item)
    return item


def scene(width: int | None = None, height: int | None = None, color: str | None = None, cue_at_end: bool | None = None):
    scene = Scene(width, height, color, cue_at_end)
    ROOT_OBJECTS.get().append(scene)
    return scene


def group():
    return make_node(Group)


def rect():
    return make_node(Rect)


def ellipse():
    return make_node(Ellipse)


def path():
    return make_node(Path)


def image(path, keep_aspect=True):
    return make_node(Image, path, keep_aspect)
