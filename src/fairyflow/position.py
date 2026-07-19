from .exprs import Call

# Matches engine::NodeId::SCENE (crates/engine/src/basictypes.rs), which
# parses the wire value -1 into that reserved sentinel
SCENE_NODE_ID = -1


class Position:
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y

    def into_node(self, node):
        x = Call("map_x", self.node, node, self.x, self.y)
        y = Call("map_y", self.node, node, self.x, self.y)
        return Position(node, x, y)

    def move(self, x, y):
        x = Call.add(self.x, x)
        y = Call.add(self.y, y)
        return Position(self.node, x, y)
