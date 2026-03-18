from .position import Position
from .expr import BaseExpr, DynExpr, EvalCtx, TimedValue, Transition
from .tobject import TimedObject
from .color import Color
from typing import Union


def _serialize(val):
    if isinstance(val, Color):
        return val.to_css()
    return val

class ItemBase(TimedObject):

    def __init__(self, parent: Union[None, "Node"], frame: int):
        super().__init__(frame)
        self._parent = parent
        if parent:
            self._id = parent._new_id()
        else:
            self._id = 0
        self._add_attr("x", 0)
        self._add_attr("y", 0)
        self._add_attr("width", 0)
        self._add_attr("height", 0)
        self._add_attr("scale", 1)
        self._add_attr("rotation", 0)
        self._add_attr("alpha", 1)

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
    
    def pos_x(self, *, px: float | None, align: float | None = None):
        if align is not None:
            parent_val = self._get_parent()._get_attr("width")
            self._set_attr("x", DynExpr(lambda ctx: ctx.eval_obj(parent_val) * align))
            return self
        if px is not None:
            self._set_attr("x", px)
        return self
    
    def pos_y(self, *, px: float | None, align: float | None = None):
        if align is not None:
            parent_val = self._get_parent()._get_attr("width")
            self._set_attr("y", DynExpr(lambda ctx: ctx.eval_obj(parent_val) * align))
            return self
        if px is None:
            self._set_attr("y", px)
        return self

    
    def pos(self, position: Position):
        position = position.into_node(self._parent)
        self._set_attr("x", position.x)
        self._set_attr("y", position.y)
        return self

    def get_pos(self) -> Position:
        x = self._get_attr("x")
        y = self._get_attr("y")
        return Position(self._parent, x, y)

    def alpha(self, value):
        self._set_attr("alpha", value)
        return self

    def move_x(self, value):
        # TODO        
        return self
    
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


NODE_CONTEXT = None
ROOT_OBJECT = None


class Node(ItemBase):
    kind = "node"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._parent = parent
        self._children = []
        self._ctx = None

    def dump(self):
        obj = super().dump()
        if self._children:
            obj["children"] = [c.dump() for c in self._children]
        return obj
    
    def key_frames(self, out: set):
        super().key_frames(out)
        for child in self._children:
            child.key_frames(out)

    def build(self, ctx):
        result = super().build(ctx)
        if self._children:
            result["children"] = [child.build(ctx) for child in self._children]
        return result
    
    def parent_chain(self) -> list["Node"]:
        result = [self]
        node = self._parent
        while node is not None:
            result.append(node)
            node = node._parent
        return result
        

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

    def __repr__(self):
        return f"<{self.kind} id={self.id} #c={len(self._children)}>"


class Scene(Node):
    kind = "scene"

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._add_attr("fill_color", Color.parse("white"))
        self._id_counter = 0

    def fill_color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self

    def _new_id(self):
        self._id_counter += 1
        return self._id_counter
    
    def build(self, ctx: EvalCtx):
        attrs = self._attrs
        return {"kind": self.kind, "width": ctx.eval_obj(attrs["width"]), "height": ctx.eval_obj(attrs["height"]), "fill_color": _serialize(ctx.eval_obj(attrs["fill_color"])), "children": [child.build(ctx) for child in self._children]}


class StyledItem(ItemBase):

    def __init__(self, parent, frame):
        super().__init__(parent, frame)
        self._add_attr("fill_color", None)

    def fill_color(self, value: str):
        self._set_attr("fill_color", Color.parse(value))
        return self


class Rect(StyledItem):
    kind = "rect"


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
    scene = Scene(None, 0).width(width).height(height)
    ROOT_OBJECT = scene
    return scene


def node(*, frame=None):
    return make_item(Node, frame)


def rect(*, frame=None):
    return make_item(Rect, frame)
