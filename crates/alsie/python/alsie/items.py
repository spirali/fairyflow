from .position import Position
from .expr import BaseExpr, DynExpr, EvalCtx, TimedValue, Transition
from .tobject import TimedObject
from .color import Color
from typing import Union


NODE_CONTEXT = None
ROOT_OBJECT = None

def _serialize(val):
    if isinstance(val, Color):
        return str(val)
    return val

class ItemBase(TimedObject):

    def __init__(self, parent: Union[None, "Node"], frame: int):
        super().__init__(frame)
        self._parent = parent
        if parent:
            self._id = parent._new_id()
        else:
            self._id = 0

    def parent_chain(self) -> list["Node"]:
        result = []
        node = self
        while node is not None:
            if isinstance(node, Node):
                result.append(node)
            node = node._parent
        return result
                
    def _new_id(self):
        return self._parent._new_id()
    
    def build(self, ctx: EvalCtx):
        result = {"kind": self.kind, "id": self._id}
        attrs = self._attrs
        for name in attrs:
            result[name] = _serialize(ctx.eval_obj(attrs[name]))
        return result
    
    def key_frames(self, out):
        for value in self._attrs.values():
            value.key_frames(out)
    
    def _get_parent(self):
        if self._parent is None:
            raise Exception("Node does not have parent")
        return self._parent

    def __repr__(self):
        return f"<{self.kind} id={self._id}>"


class AlphaMixin:

    def _init_alpha(self):
        self._add_attr("alpha", 1)

    
    def alpha(self, value):
        self._set_attr("alpha", value)
        return self


class SizeMixin:

    def _init_size(self, width, height):
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

    def _init_position(self, x, y):
        self._add_attr("x", 0)
        self._add_attr("y", 0)     

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



class StyleMixin(AlphaMixin):

    def _init_style(self):
        self._add_attr("fill_color", None)
        self._add_attr("stroke_color", None)
        self._add_attr("stroke_width", 1)
        self._init_alpha()

    def fill_color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self

    def stroke_color(self, value: str):
        self._set_attr("stroke_color", Color.parse(value))
        return self

    def stroke_width(self, value: float):
        self._set_attr("stroke_width", value)
        return self



class ItemWithChildren(ItemBase):

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._children = []
        self._ctx = None

    def build(self, ctx):
        result = super().build(ctx)
        if self._children:
            result["children"] = [child.build(ctx) for child in self._children]
        return result
    
    def key_frames(self, out: set):
        super().key_frames(out)
        for child in self._children:
            child.key_frames(out)

class ContextManagerMixin:

    def _init_context_manager(self):
        self._ctx = None
      
    def __enter__(self):
        global NODE_CONTEXT
        assert self._ctx is None
        self._ctx = NODE_CONTEXT
        NODE_CONTEXT = self
        return self

    def __exit__(self, *args):
        global NODE_CONTEXT
        NODE_CONTEXT = self._ctx
        self._ctx = None


class Node(ItemWithChildren, ContextManagerMixin, PositionMixin, SizeMixin, AlphaMixin):
    kind = "node"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_context_manager()
        self._init_position(0, 0)
        self._init_size(0, 0)
        self._init_alpha()
        self._add_attr("rotation", 0)
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



class Scene(ItemWithChildren, ContextManagerMixin, SizeMixin):
    kind = "scene"

    def __init__(self, width, height):
        super().__init__(None, 0)
        self._init_context_manager()
        self._init_size(width, height)
        self._add_attr("fill_color", Color.parse("white"))
        self._id_counter = 0

    def fill_color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter
    

class Rect(ItemBase, PositionMixin, SizeMixin, StyleMixin):
    kind = "rect"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_position(0, 0)
        self._init_size(0, 0)
        self._init_style()


class Ellipse(ItemBase, PositionMixin, SizeMixin, StyleMixin):
    kind = "ellipse"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_position(0, 0)
        self._init_size(0, 0)
        self._init_style()


class Path(ItemWithChildren, StyleMixin):
    kind = "path"
    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._init_style()

    def _prev_coords(self):
        if self._children:
            c = self._children[0]
            return (c._get_attr("y"), c._get_attr("y"))
        else:
            return (0, 0)

    def move(self):
        p = PathMove(self, self._frame, *self._prev_coords())
        self._children.append(p)
        return p

    def line(self):
        p = PathLine(self, self._frame, *self._prev_coords())
        self._children.append(p)
        return p

    def cubic(self):
        p = PathCubic(self, self._frame, *self._prev_coords())
        self._children.append(p)
        return p

class PathMove(ItemBase, PositionMixin):

    kind = "move"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)


class PathLine(ItemBase, PositionMixin):

    kind = "line"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)


class PathCubic(ItemBase, PositionMixin):

    kind = "cubic"

    def __init__(self, parent, frame, x, y):
        super().__init__(parent, frame)
        self._init_position(x, y)
        self._add_attr("c1_x", 0)
        self._add_attr("c1_y", 0)
        self._add_attr("c2_x", 0)
        self._add_attr("c2_y", 0)

    def c1_x(self, px):
        """ Set x-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_x", px)

    def c1_y(self, px):
        """ Set y-coordinate of control point 1. It is relative to the start point of the path"""
        self._set_attr("c1_x", px)

    def c2_x(self, px):
        """ Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_x", px)

    def c2_y(self, px):
        """ Set x-coordinate of control point 1. It is relative to the end point of the path"""
        self._set_attr("c2_y", px)

    def c1_xy(self, x, y):
        self.c1_x(x)
        self.c1_y(y)
        return self

    def c2_xy(self, x, y):
        self.c2_x(x)
        self.c2_y(y)
        return self


def make_item(cls, frame):
    if NODE_CONTEXT is None:
        raise Exception("Element created out of context of a parent ndoe")
    if frame is None:
        frame = NODE_CONTEXT._start
    item = cls(NODE_CONTEXT, frame)
    NODE_CONTEXT._children.append(item)
    return item


def scene(width: int, height: int):
    global ROOT_OBJECT
    scene = Scene(width, height)
    ROOT_OBJECT = scene
    return scene


def node(*, frame=None):
    return make_item(Node, frame)


def rect(*, frame=None):
    return make_item(Rect, frame)


def ellipse(*, frame=None):
    return make_item(Ellipse, frame)


def path(*, frame=None):
    return make_item(Path, frame)