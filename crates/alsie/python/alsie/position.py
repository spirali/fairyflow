
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
        for node in path_up[1:]:
            x = FromNodePosX(node, x, y)
            y = FromNodePosY(node, x, y)
        for node in path_down:
            x = IntoNodePosX(node, x, y)
            y = IntoNodePosY(node, x, y)
        return Position(self.node, x, y)


class NodePosTransformBase(BaseExpr):
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y


class FromNodePosX(NodePosTransformBase):
    """Local → parent:  parent_x = local_x * scale_x + node_x"""

    def eval(self, ctx: EvalCtx):
        node_x = ctx.eval_obj(self.node._get_attr("x"))
        scale_x = ctx.eval_obj(self.node._get_attr("scale_x"))
        return ctx.eval_obj(self.x) * scale_x + node_x


class FromNodePosY(NodePosTransformBase):
    """Local → parent:  parent_y = local_y * scale_y + node_y"""

    def eval(self, ctx: EvalCtx):
        node_y = ctx.eval_obj(self.node._get_attr("y"))
        scale_y = ctx.eval_obj(self.node._get_attr("scale_y"))
        return ctx.eval_obj(self.y) * scale_y + node_y


class IntoNodePosX(NodePosTransformBase):
    """Parent → local:  local_x = (parent_x - node_x) / scale_x"""

    def eval(self, ctx: EvalCtx):
        node_x = ctx.eval_obj(self.node._get_attr("x"))
        scale_x = ctx.eval_obj(self.node._get_attr("scale_x"))
        return (ctx.eval_obj(self.x) - node_x) / scale_x


class IntoNodePosY(NodePosTransformBase):
    """Parent → local:  local_y = (parent_y - node_y) / scale_y"""

    def eval(self, ctx: EvalCtx):
        node_y = ctx.eval_obj(self.node._get_attr("y"))
        scale_y = ctx.eval_obj(self.node._get_attr("scale_y"))
        return (ctx.eval_obj(self.y) - node_y) / scale_y