
import math
from .expr import BaseExpr, EvalCtx

def tree_path(node1, node2):
    """
    Returns a path from node1 to node2
    Path is returned as two lists as path up and then followed by path down
    """
    if node1:
        chain1 = node1.parent_chain()
    else:
        chain1 = []
    if node2:
        chain2 = node2.parent_chain()
    else:
        chain2 = []
    last = None
    while chain1 and chain2 and chain1[-1] == chain2[-1]:
        last = chain1.pop()
        chain2.pop()
    if last is not None:
        chain1.append(last)
    chain2.reverse()
    return chain1, chain2


class Position:
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y

    def into_node(self, node):
        if node == self.node:
            return self
        path_up, path_down = tree_path(self.node, node)
        x = self.x
        y = self.y
        for n in path_up[1:]:
            new_x = FromNodePosX(n, x, y)
            new_y = FromNodePosY(n, x, y)
            x, y = new_x, new_y
        for n in path_down:
            new_x = IntoNodePosX(n, x, y)
            new_y = IntoNodePosY(n, x, y)
            x, y = new_x, new_y
        return Position(self.node, x, y)


class NodePosTransformBase(BaseExpr):
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y

    def _node_attrs(self, ctx: EvalCtx):
        node_x   = ctx.eval_obj(self.node._get_attr("x"))
        node_y   = ctx.eval_obj(self.node._get_attr("y"))
        scale_x  = ctx.eval_obj(self.node._get_attr("scale_x"))
        scale_y  = ctx.eval_obj(self.node._get_attr("scale_y"))
        rotation = ctx.eval_obj(self.node._get_attr("rotation"))
        r = math.radians(rotation)
        return node_x, node_y, scale_x, scale_y, math.cos(r), math.sin(r)


class FromNodePosX(NodePosTransformBase):
    """Local → parent:  parent_x = cos(r)*sx*lx - sin(r)*sy*ly + node_x"""

    def eval(self, ctx: EvalCtx):
        node_x, _node_y, sx, sy, cos_r, sin_r = self._node_attrs(ctx)
        lx = ctx.eval_obj(self.x)
        ly = ctx.eval_obj(self.y)
        return cos_r * sx * lx - sin_r * sy * ly + node_x


class FromNodePosY(NodePosTransformBase):
    """Local → parent:  parent_y = sin(r)*sx*lx + cos(r)*sy*ly + node_y"""

    def eval(self, ctx: EvalCtx):
        _node_x, node_y, sx, sy, cos_r, sin_r = self._node_attrs(ctx)
        lx = ctx.eval_obj(self.x)
        ly = ctx.eval_obj(self.y)
        return sin_r * sx * lx + cos_r * sy * ly + node_y


class IntoNodePosX(NodePosTransformBase):
    """Parent → local:  local_x = (cos(r)*qx + sin(r)*qy) / sx
       where q = parent_pos - node_translation"""

    def eval(self, ctx: EvalCtx):
        node_x, node_y, sx, _sy, cos_r, sin_r = self._node_attrs(ctx)
        qx = ctx.eval_obj(self.x) - node_x
        qy = ctx.eval_obj(self.y) - node_y
        return (cos_r * qx + sin_r * qy) / sx


class IntoNodePosY(NodePosTransformBase):
    """Parent → local:  local_y = (-sin(r)*qx + cos(r)*qy) / sy
       where q = parent_pos - node_translation"""

    def eval(self, ctx: EvalCtx):
        node_x, node_y, _sx, sy, cos_r, sin_r = self._node_attrs(ctx)
        qx = ctx.eval_obj(self.x) - node_x
        qy = ctx.eval_obj(self.y) - node_y
        return (-sin_r * qx + cos_r * qy) / sy
