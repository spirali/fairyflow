from typing import Union, Literal
import os

from .layout import CENTERING_LAYOUT, ColumnLayout, RowLayout
from .position import Position
from .info import get_info
from .aobject import AnimatedObject, get_frame
from .avalue import AnimatedValue
from .color import Color
from .exprs import (
    expr_default_height,
    expr_default_width,
    expr_default_x,
    expr_default_y,
    expr_path_length,
    expr_path_x,
    expr_path_y,
    expr_mul,
    expr_norm,
    expr_sub,
    to_expr,
)
from .ctxvars import (
    get_current_node,
    ROOT_OBJECTS,
    set_current_node,
    set_frame,
)
from .config import DEFAULT_SCENE_CONFIG, FPS


class Node(AnimatedObject):
    def __init__(self, parent: Union[None, "Group"]):
        super().__init__()
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
            result[name] = serialize_expr(attrs[name])
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

    def alpha(self, value, transition=None):
        self._set_attr("alpha", value, transition)
        return self


class ZLevelMixin:
    def _init_z(self):
        self._add_from_parent("z_level", 0)

    def z_level(self, value, transition=None):
        self._set_attr("z_level", value, transition)
        return self


class SizeMixin:
    def _init_size(self, width=None, height=None):
        if width is None:
            width = expr_default_width(self)
        if height is None:
            height = expr_default_height(self)
        self._add_attr("width", width)
        self._add_attr("height", height)

    def width(self, value, transition=None):
        self._set_attr("width", value, transition)
        return self

    def height(self, value, transition=None):
        self._set_attr("height", value, transition)
        return self

    def size(self, width, height, transition=None):
        self.width(width, transition)
        self.height(height, transition)
        return self


class PositionMixin:
    def _init_position(self, x=None, y=None):
        if x is None:
            x = expr_default_x(self)
        if y is None:
            y = expr_default_y(self)
        self._add_attr("x", x)
        self._add_attr("y", y)

    def x(self, px, transition=None):
        self._set_attr("x", px, transition)
        return self

    def y(self, px, transition=None):
        self._set_attr("y", px, transition)
        return self

    def xy(self, x, y, transition=None):
        self.x(x, transition)
        self.y(y, transition)
        return self

    def align_x(self, value, transition=None):
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = expr_mul(
                expr_sub(parent._get_attr("width"), self._get_attr("width")), value
            )
        else:
            new_value = expr_mul(parent._get_attr("width"), value)
        self._set_attr("x", new_value, transition)
        return self

    def align_y(self, value, transition=None):
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = expr_mul(
                expr_sub(parent._get_attr("height"), self._get_attr("height")), value
            )
        else:
            new_value = expr_mul(parent._get_attr("height"), value)
        self._set_attr("y", new_value, transition)
        return self

    def pos(self, position: Position, transition=None):
        position = position.into_node(self._parent)
        self._set_attr("x", position.x, transition)
        self._set_attr("y", position.y, transition)
        return self

    def move(self, dx, dy, transition=None):
        self._move_attr("x", dx, transition)
        self._move_attr("y", dy, transition)
        return self

    def get_pos(self) -> Position:
        x = self._get_attr("x")
        y = self._get_attr("y")
        return Position(self._parent, x, y)
    
    def follow_path(self, path: "Path", *, time=1, auto_fwd=None):
        assert isinstance(path, Path)
        if frames is None:
            if time is None:
                time = 1
            frames = FPS * time
        start = get_frame()
        end = start + frames
        av = AnimatedValue(0, get_frame())
        av.set(end, 1, "linear")
        x = expr_path_x(path, av)
        y = expr_path_y(path, av)
        if self._has_attr("width"):
            x = x - expr_mul(self._get_attr("width"), 0.5)
            y = y - expr_mul(self._get_attr("height"), 0.5)
        self.xy(x, y)
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

    def color(self, value: str, transition = None):
        self._set_attr("fill_color", Color.parse(value), transition)
        return self

    def stroke_color(self, value: str, transition = None):
        self._set_attr("stroke_color", Color.parse(value), transition)
        return self

    def stroke_width(self, value: float, transition = None):
        self._set_attr("stroke_width", value, transition)
        return self


class NodeWithChildren(Node):
    def __init__(self, parent, children=None):
        super().__init__(parent)
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

    def scale_x(self, value, transition=None):
        self._set_attr("scale_x", value, transition)
        return self

    def scale_y(self, value, transition=None):
        self._set_attr("scale_y", value, transition)
        return self

    def scale(self, value, transition=None):
        self.scale_x(value, transition)
        self.scale_y(value, transition)
        return self

    def rotate(self, value, transition=None):
        self._set_attr("rotation", value, transition)
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

    def __init__(self, parent):
        super().__init__(parent)
        self._init_context_manager()
        self._init_size()
        self._init_alpha()
        self._init_z()
        self._init_rot_and_scale()
        self._add_attr("clip_x", 0)
        self._add_attr("clip_y", 0)
        self._add_attr("clip_w", 1)
        self._add_attr("clip_h", 1)

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
    
    def clip_x(self, value, transition=None):
        self._set_attr("clip_x", value, transition)
        return self
    
    def clip_y(self, value, transition=None):
        self._set_attr("clip_y", value, transition)
        return self

    def clip_w(self, value, transition=None):
        self._set_attr("clip_w", value, transition)
        return self

    def clip_h(self, value, transition=None):
        self._set_attr("clip_h", value, transition)
        return self            


class Scene(NodeWithChildren, ContextManagerMixin, SizeMixin):
    kind = "scene"

    def __init__(self, width, height, color, cue_at_start):
        set_frame(0)
        super().__init__(None)
        self._init_context_manager()
        if width is None:
            width = DEFAULT_SCENE_CONFIG["width"]
        if height is None:
            height = DEFAULT_SCENE_CONFIG["height"]
        if color is None:
            color = DEFAULT_SCENE_CONFIG["color"]
        if cue_at_start is None:
            cue_at_start = DEFAULT_SCENE_CONFIG["cue_at_start"]
        self._init_size(width, height)
        self._add_attr("fill_color", color)
        self._id_counter = 0
        self._layout = CENTERING_LAYOUT
        self.name = None        
        self.max_frame = 0
        self.cue_at_start = cue_at_start
        self.cues = set()

    def __enter__(self):
        super().__enter__()

    def color(self, value: str, transition=None):
        self._set_attr("fill_color", Color.parse(value), transition)
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter
    
    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["name"] = self.name
        result["frames"] = self.max_frame + 1
        if self.cue_at_start:
            self.cues.add(0)
        if self.cues:
            result["cues"] = sorted(self.cues)
        return result
    
    def get_scene(self):
        return self
    
    def update_max_frame(self, frame):
        self.max_frame = max(self.max_frame, frame)


class Rect(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    kind = "rect"

    def __init__(self, parent):
        super().__init__(parent)
        self._init_size(0, 0)
        self._init_style()
        self._init_z()
        self._init_position()


class Ellipse(Node, PositionMixin, SizeMixin, StyleMixin, ZLevelMixin):
    kind = "ellipse"

    def __init__(self, parent):
        super().__init__(parent)
        self._init_size(0, 0)
        self._init_style()
        self._init_z()
        self._init_position()


class Path(NodeWithChildren, StyleMixin, ZLevelMixin):
    kind = "path"

    def __init__(self, parent):
        super().__init__(parent)
        self._init_style()
        self._init_z()
        self._add_attr("crop_start", 0.0)
        self._add_attr("crop_end", 1.0)

    def _prev_coords(self):
        if self._children:
            c = self._children[0]
            return (c._get_attr("y"), c._get_attr("y"))
        else:
            return (0, 0)
        
    def crop_start(self, value, transition=None):
        self._set_attr("crop_start", value, transition)

    def crop_end(self, value, transition=None):
        self._set_attr("crop_end", value, transition)

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
            vx = expr_sub(child1._get_attr("x"), p.x)
            vy = expr_sub(child1._get_attr("y"), p.y)
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
            vx = expr_sub(child1._get_attr("x"), p.x)
            vy = expr_sub(child1._get_attr("y"), p.y)
        return (child0, vx, vy)    


    def _create_arrow(self, child, vx, vy, length, width):                
        nx = expr_norm(vx, vy)
        ny = expr_norm(vy, vx)
        
        pos = child.get_pos()
        px = pos.x + nx * length
        py = pos.y + ny * length
                
        dx = nx * width * 0.5
        dy = ny * width * 0.5

        path = Path(self._parent, get_frame())
        self._parent._children.append(path)
        path.move_to().xy(px - dy, py + dx)
        path.line_to().pos(pos)
        path.line_to().xy(px + dy, py - dx)
        path.close()
        return path
        

    def triangle_arrow(self, placement: Literal["start", "end"]="end", *, length=None, width=None):
        """
        Creates a triangle arrow on the path. 

        Important: It sets `crop_start` of the `self` to not overlap with arrow.
        """
        if placement == "start":
            dir = self._get_start_direction()
        else:
            dir = self._get_end_direction()
        if dir is None:            
            return None
        if length is None:
            length = self._get_attr("stroke_width") * 5
        if width is None:
            width  = length
        path = self._create_arrow(*dir, length, width)
        path.color(self._get_attr("stroke_color"))

        if placement == "start":
            self.crop_start((to_expr(length) * 0.5) / expr_path_length(self))
        else:
            self.crop_end(to_expr(1.0) - ((to_expr(length) * 0.5) / expr_path_length(self)))
        return path
    

class PathMove(Node, PositionMixin):
    kind = "move"

    def __init__(self, parent, x, y):
        super().__init__(parent)
        self._init_position(x, y)


class PathLine(Node, PositionMixin):
    kind = "line"

    def __init__(self, parent, x, y):
        super().__init__(parent)
        self._init_position(x, y)


class PathClose(Node):
    kind = "close"


class PathCubic(Node, PositionMixin):
    kind = "cubic"

    def __init__(self, parent, x, y):
        super().__init__(parent)
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

    def __init__(self, parent, image_path, keep_aspect):
        super().__init__(parent)
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

    def __init__(self, parent, layer_name):
        super().__init__(parent)
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
    item = cls(current_node, *args)
    current_node._children.append(item)
    return item


def scene(width: int | None = None, height: int | None = None, color: str | None = None, cue_at_start: bool | None = None):
    scene = Scene(width, height, color, cue_at_start)
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
