class Cursor:
    def __init__(self, node: Node):
        self._node = node
        self._root = root
        self._frame: int = 0
        self._position = Point.node_point(0.5, 0.5)

    def frame(self, frame: int):
        self._frame = frame

    def move(self, x, y):
        self._position = Point(self._position.x + x, self._position.y + y)

    def color(self, value: str):
        self._node.set(self._frame, "color", value)

    def size(self, x: float, y: float):
        self.width(x)
        self.height(y)

    def width(self, value: float):
        self._node.set(self._frame, "width", value)

    def height(self, value: float):
        self._node.set(self._frame, "height", value)
