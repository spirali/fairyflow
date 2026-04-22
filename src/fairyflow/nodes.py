from typing import Union, Literal, Self
from beartype import beartype
import os

from .types import ColorLike, FloatLike, StringLike
from .layout import CENTERING_LAYOUT, ColumnLayout, RowLayout
from .position import Position
from .info import get_info
from .aobject import AnimatedObject, get_frame
from .avalue import AnimatedValue
from .color import Color
from .exprs import (
    Call,
    to_expr,
)
from .ctxvars import (
    Transition,
    fwd_time,
    get_current_node,
    ROOT_OBJECTS,
    set_current_node,
    set_frame,
)
from .config import DEFAULT_SCENE_CONFIG, FPS

type OpTr = Transition | None

@beartype
class Node(AnimatedObject):
    def __init__(self, parent: Union[None, "NodeWithChildren"]):
        super().__init__()
        self._parent = parent
        if parent:
            self._id = parent._new_id()
        else:
            self._id = 0            
        self.info = get_info(self._id)
        self._name = None

    def name(self, name: str | None):
        """ Sets name of the node.

        Name is is uninterpred by FairyFlow and is used only for debugging or for searching nodes via methods like find_node.
        """
        self._name = name
        self.info["name"] = name
        return self

    def parent_chain(self) -> list["Group"]:
        """
        Return list of nodes to the parent node
        """
        result = []
        node = self
        while node is not None:
            if isinstance(node, Group):
                result.append(node)
            node = node._parent
        return result

    def parent_group(self) -> Union["Group", "Scene"]:
        """
        Returns the closest group (or scene) of the self. 
        
        Returns None if self is Scene
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
    

    def match(self, *, name: str | None = None, kind: str | None = None) -> bool:
        """
        Returns True if node matches the filter condition.
        """
        if name is not None and self.name != name:
            return False
        if kind is not None and self.kind != kind:
            return False        
        return True

    def find_node(self, *, name: str | None = None, kind: str | None = None) -> Union["Node", None]:
        """
        Finds the node that matches the filter (BFS).
        """
        if self.match(name, kind):
            return self        
        else:
            return None
        
    def get_scene(self) -> "Scene":
        """
        Return scene (root node).
        """
        return self._parent.get_scene()

    def __repr__(self):
        return f"<{self.kind} id={self._id}>"


@beartype
class AlphaMixin:
    def _init_alpha(self):
        self._add_attr("alpha", 1)

    def _init_alpha_from_parent(self):
        self._add_from_parent("alpha")

    def alpha(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Set alpha of the node (and its children)
        """
        self._set_attr("alpha", value, tr)
        return self
    
    def fade_in(self, time: float = 1) -> Self:
        """
        Animation: Fade in (start with alpha 0 and change it ot 1)
        """
        self._anim_attr("alpha", 1, time, start=0)
        return self
    
    def fade_out(self, time: float = 1) -> Self:
        """
        Animation: Fade out (change alpha to 0)
        """
        self._anim_attr("alpha", 0, time)        
        return self    


@beartype
class ZLevelMixin:
    def _init_z(self):
        self._add_from_parent("z_level", 0)

    def z_level(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Set z-level.
        """
        self._set_attr("z_level", value, tr)
        return self


@beartype
class SizeMixin:
    def _init_size(self, width=None, height=None):
        if width is None:
            width = Call.default_width(self)
        if height is None:
            height = Call.default_height(self)
        self._add_attr("width", width)
        self._add_attr("height", height)

    def width(self, value: FloatLike, tr: OpTr=None) -> Self:
        """
        Sets width of the node
        """
        self._set_attr("width", value, tr)
        return self

    def height(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Sets height of the node
        """
        self._set_attr("height", value, tr)
        return self

    def size(self, width, height: FloatLike, tr: OpTr = None) -> Self:
        """
        Sets width and height of the node
        """        
        self._set_attr("width", width, tr)
        self._set_attr("height", height, tr)
        return self


@beartype
class PositionMixin:
    def _init_position(self, x=None, y=None):
        if x is None:
            x = Call.default_x(self)
        if y is None:
            y = Call.default_y(self)
        self._add_attr("x", x)
        self._add_attr("y", y)

    def x(self, px: FloatLike, tr: OpTr = None) -> Self:
        """
        Sets x of the node.
        """
        self._set_attr("x", px, tr)
        return self

    def y(self, px: FloatLike, tr: OpTr = None) -> Self:
        """
        Sets y of the node.
        """        
        self._set_attr("y", px, tr)
        return self
    
    def x_reset(self, tr: OpTr = None) -> Self:
        """
        Resets x coordinate to default.
        """                
        self._set_attr("x", Call.default_x(self), tr)
        return self
    
    def y_reset(self, tr: OpTr = None) -> Self:
        """
        Resets y coordinate to default.
        """                        
        self._set_attr("y", Call.default_y(self), tr)
        return self
    
    def xy_reset(self, tr: OpTr = None) -> Self:
        """
        Resets x,y coordinate to default.
        """                                
        self.x_reset(tr)
        self.y_reset(tr)
        return self

    def xy(self, x: FloatLike, y: FloatLike, tr: OpTr = None) -> Self:
        """
        Sets both x and y of the node.
        """
        self._set_attr("x", x, tr)
        self._set_attr("y", y, tr)
        return self

    def align_x(self, value: FloatLike, tr: OpTr=None) -> Self:
        """
        Horizontally align item inside parent node (0 = left, 0.5 = center, 1.0 = right)
        """
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = Call.mul(
                Call.sub(parent._get_attr("width"), self._get_attr("width")), value
            )
        else:
            new_value = Call.mul(parent._get_attr("width"), value)
        self._set_attr("x", new_value, tr)
        return self

    def align_y(self, value: FloatLike, tr: OpTr=None) -> Self:
        """
        Vertically align item inside parent node (0 = top, 0.5 = center, 1.0 = bottom)
        """        
        parent = self.parent_group()
        if isinstance(self, SizeMixin):
            new_value = Call.mul(
                Call.sub(parent._get_attr("height"), self._get_attr("height")), value
            )
        else:
            new_value = Call.mul(parent._get_attr("height"), value)
        self._set_attr("y", new_value, tr)
        return self

    def pos(self, position: Position, tr: OpTr=None) -> Self:
        """
        Sets the position of the node
        """
        position = position.into_node(self._parent)
        self._set_attr("x", position.x, tr)
        self._set_attr("y", position.y, tr)
        return self

    def move(self, dx: FloatLike, dy: FloatLike, tr: OpTr=None) -> Self:
        """
        Move the position of the node
        """        
        self._move_attr("x", dx, tr)
        self._move_attr("y", dy, tr)
        return self

    def get_pos(self, align_x=0, align_y=0) -> Position:
        """
        Get position of the node
        """        
        x = self._get_attr("x")
        y = self._get_attr("y")
        if isinstance(self, SizeMixin):
            if align_x != 0:
                x = x + self._get_attr("width") * align_x
            if align_y != 0:
                y = y + self._get_attr("width") * align_x
        return Position(self._parent, x, y)
    
    def follow_path(self, path: "Path", *, time : float = 1):
        assert isinstance(path, Path)
        if frames is None:
            if time is None:
                time = 1
            frames = FPS * time
        start = get_frame()
        end = start + frames
        av = AnimatedValue(0, get_frame())
        av.set(end, 1, "L")
        x = Call.path_x(path, av)
        y = Call.path_y(path, av)
        if self._has_attr("width"):
            x = x - Call.mul(self._get_attr("width"), 0.5)
            y = y - Call.mul(self._get_attr("height"), 0.5)
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

    def color(self, value: ColorLike, tr: OpTr = None) -> Self:
        """
        Sets fill color of the node
        """
        self._set_attr("fill_color", Color.parse(value), tr)
        return self

    def stroke_color(self, value: ColorLike, tr: OpTr = None) -> Self:
        """
        Sets stroke color of the node
        """        
        self._set_attr("stroke_color", Color.parse(value), tr)
        return self

    def stroke_width(self, value: FloatLike, tr: OpTr = None):
        """
        Sets stroke width
        """                
        self._set_attr("stroke_width", value, tr)
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
    
    def find_node(self, *, name=None, kind=None) -> Node | None:
        result = super().find_node(name=name, kind=kind)
        if result is not None:
            return result
        for child in self._children:
            result = child.find_node(name=name, kind=kind)
            if result is not None:
                return result
            
    def get_child(self, *, name: str | None = None, kind: str | None = None) -> Node | None:
        """
        Get first direct descendant that maches the filter
        """
        for child in self._children:
            if child.match(name=name, kind=kind):
                return child
        return None
        
    def get_children(self) -> list[Node]:
        """
        Return children
        """
        return self._children    
    


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

    def scale_x(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Scales the node in x axis
        """
        self._set_attr("scale_x", value, tr)
        return self

    def scale_y(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Scales the node in y axis
        """        
        self._set_attr("scale_y", value, tr)
        return self

    def scale(self, value: FloatLike, tr: OpTr = None) -> Self:
        """Scales the node"""
        self._set_attr("scale_x", value, tr)
        self._set_attr("scale_y", value, tr)
        return self

    def rotate(self, value: FloatLike, tr: OpTr = None) -> Self:
        """Rotates the node"""
        self._set_attr("rotation", value, tr)
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

    def column(self, gap: FloatLike = 0, align: FloatLike = 0.5) -> Self:
        """ Set layout to "column" mode """
        self._layout = ColumnLayout(get_frame(), gap, align)
        return self

    def row(self, gap: FloatLike = 0, align: FloatLike = 0.5) -> Self:
        """ Set layout to "row" mode """
        self._layout = RowLayout(get_frame(), gap, align)
        return self

    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["layout"] = self._layout.serialize(serializer)
        return result
    
    def clip_x(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        x-axis of the clipped window. Relative with the width of the node; 0 - left, 1 - right
        """
        self._set_attr("clip_x", value, tr)
        return self
    
    def clip_y(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        y-axis of the clipped window. Relative with the height of the node; 0 - top, 1 - bottom
        """        
        self._set_attr("clip_y", value, transition)
        return self

    def clip_w(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        width of the clipped window. Relative with the width of the node; 1 = full width
        """                
        self._set_attr("clip_w", value, transition)
        return self

    def clip_h(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Height of the clipped window. Relative with the height of the node; 1 = full height
        """                
        self._set_attr("clip_h", value, tr)
        return self            
    
    def hide_right(self, time: float = 1) -> Self:
        """
        Animation; hide the node by sweaping to the right
        """
        self._anim_attr("clip_x", 1, time)
        return self

    def hide_left(self, time: float = 1) -> Self:
        """
        Animation; hide the node by sweaping to the left
        """        
        self._anim_attr("clip_w", 0, time)
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
        self.max_frame = 0
        self.cue_at_start = cue_at_start        
        self.cues = set()        

    def __enter__(self):
        super().__enter__()

    def color(self, value: ColorLike, tr: OpTr = None) -> Self:
        """
        Sets the background color of the scene
        """
        self._set_attr("fill_color", Color.parse(value), tr)
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter
    
    def serialize(self, serializer):
        result = super().serialize(serializer)
        result["name"] = self._name
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
        
    def crop_start(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Crops the path from the starts; [0-1] relative to the length of the path
        """
        self._set_attr("crop_start", value, tr)

    def crop_end(self, value: FloatLike, tr: OpTr = None) -> Self:
        """
        Crops the path from the end; [0-1] relative to the length of the path
        """        
        self._set_attr("crop_end", value, tr)

    def move_to(self) -> "PathMove":
        """
        Create "move" commnand on the path.
        """
        p = PathMove(self, *self._prev_coords())
        self._children.append(p)
        return p

    def line_to(self) -> "PathLine":
        """
        Create "line" commnand on the path.
        """
        p = PathLine(self, *self._prev_coords())
        self._children.append(p)
        return p

    def cubic_to(self) -> "PathCubic":
        """
        Create cubiec bezier curve commnand on the path.
        """
        p = PathCubic(self, *self._prev_coords())
        self._children.append(p)
        return p
    
    def close(self) -> "PathClose":
        """
        Close path
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

        path = Path(self._parent)
        self._parent._children.append(path)
        path.move_to().xy(px - dy, py + dx)
        path.line_to().pos(pos)
        path.line_to().xy(px + dy, py - dx)
        path.close()
        return path
        

    def triangle_arrow(self, placement: Literal["start", "end"]="end", *, length: FloatLike | None = None, width: FloatLike | None = None) -> "Path":
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
            length = self._get_attr("stroke_width") * 3
        if width is None:
            width  = length
        path = self._create_arrow(*dir, length, width)
        path.color(self._get_attr("stroke_color"))

        length = to_expr(length)

        if placement == "start":
            self.crop_start((length * 0.5) / Call.path_length(self))
        else:
            self.crop_end(to_expr(1.0) - (length * 0.5) / Call.path_length(self))
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

    def c1_x(self, px: FloatLike, tr: OpTr = None) -> Self:
        """Set x-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_x", px, tr)

    def c1_y(self, px: FloatLike, tr: OpTr = None) -> Self:
        """Set y-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_y", px, tr)

    def c2_x(self, px: FloatLike, tr: OpTr = None) -> Self:
        """Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_x", px, tr)

    def c2_y(self, px: FloatLike, tr: OpTr = None) -> Self:
        """Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_y", px, tr)

    def c1_xy(self, x: FloatLike, y: FloatLike, tr: OpTr = None) -> Self:
        """Set x,y-coordinates of control point 1. It is relative to the start point of the path"""
        self.c1_x(x, tr)
        self.c1_y(y, tr)
        return self

    def c2_xy(self, x: FloatLike, y: FloatLike, tr: OpTr = None) -> Self:
        """Set x,y-coordinates of control point 2. It is relative to the end point of the path"""
        self.c2_x(x, tr)
        self.c2_y(y, tr)
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
        self.file_name(image_path)
        self._add_attr("keep_aspect", keep_aspect)

    def layer(self, name: str) -> "ImageLayer":
        """
        Get layer from the image
        """
        for child in self._children:
            if child.layer_name == name:
                return child
        layer = ImageLayer(self, self._start, name)
        self._children.append(layer)
        return layer

    def file_name(self, image_path: str) -> Self:
        """
        Set the file name of the loaded image
        """
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


def scene(width: int | None = None, height: int | None = None, *, color: str | Color | None = None, cue_at_start: bool | None = None) -> Scene:
    """
    Creates a scene
    """
    scene = Scene(width, height, color, cue_at_start)
    ROOT_OBJECTS.get().append(scene)
    return scene


def group() -> Group:
    """
    Return a new group within the current context.
    """
    return make_node(Group)


def rect() -> Rect:
    """
    Return a new rect within the current context.
    """    
    return make_node(Rect)


def ellipse() -> Ellipse:
    """
    Return a new ellipse within the current context.
    """        
    return make_node(Ellipse)


def path() -> Path:
    """
    Return a new path within the current context
    """        
    return make_node(Path)


def image(path: str, *, keep_aspect: bool = True) -> Image:
    """
    Return a new image within the current context
    """            
    return make_node(Image, path, keep_aspect)
