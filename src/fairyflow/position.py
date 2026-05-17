from .exprs import Call


class Position:
    def __init__(self, node, x, y):
        self.node = node
        self.x = x
        self.y = y

    def into_node(self, node):
        x = Call("node_transform_x", source=self.node, target=node, x=self.x, y=self.y)
        y = Call("node_transform_y", source=self.node, target=node, x=self.x, y=self.y)
        return Position(node, x, y)

    def move(self, x, y):
        x = Call.add(self.x, x)
        y = Call.add(self.y, y)
        return Position(self.node, x, y)
