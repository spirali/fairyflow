
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

    def eval(self, ctx: EvalCtx):
        return ctx.eval_obj(self.x) + ctx.eval_obj(self.node._get_attr("x"))


class FromNodePosY(NodePosTransformBase):

    def eval(self, ctx: EvalCtx):
        return ctx.eval_obj(self.y) + ctx.eval_obj(self.node._get_attr("y"))


class IntoNodePosX(NodePosTransformBase):

    def eval(self, ctx: EvalCtx):
        return ctx.eval_obj(self.x) - ctx.eval_obj(self.node._get_attr("x"))


class IntoNodePosY(NodePosTransformBase):

    def eval(self, ctx: EvalCtx):
        return ctx.eval_obj(self.y) - ctx.eval_obj(self.node._get_attr("y"))