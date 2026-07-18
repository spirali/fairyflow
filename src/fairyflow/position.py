from .exprs import Call

SCENE_NODE_ID = -1


def _node_ref(node):
    """Wire-format reference for a map_x/map_y operand. A node with no
    parent is the Scene (verified: Scene is the only class that constructs
    itself with put_in_context=False and no explicit parent=); substituting
    the reserved sentinel (a plain int) instead of the raw Node object means
    it never reaches serializer.add_node, which would otherwise try to
    re-serialize the Scene mid-walk and produce a circular reference."""
    return SCENE_NODE_ID if node._parent is None else node


class Position:
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y

    def into_node(self, node):
        x = Call("map_x", _node_ref(self.node), _node_ref(node), self.x, self.y)
        y = Call("map_y", _node_ref(self.node), _node_ref(node), self.x, self.y)
        return Position(node, x, y)

    def move(self, x, y):
        x = Call.add(self.x, x)
        y = Call.add(self.y, y)
        return Position(self.node, x, y)
