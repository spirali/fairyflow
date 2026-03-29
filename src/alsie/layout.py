

from .exprs import Call
from .avalue import AnimatedValue


class CenteringLayout:

    def set_node_position(self, node):
        node.align_x(0.5)
        node.align_y(0.5)

    def serialize(self, serializer):
        return "center"

# As centering layour is not parametrizable, lets us create a singleton
CENTERING_LAYOUT = CenteringLayout()


class VerticalLayout:

    def __init__(self, frame, gap, x_align):
        self.gap = AnimatedValue(gap, frame)
        self.x_align = AnimatedValue(x_align, frame)

    def set_node_position(self, node):
        node.align_x(self.x_align)
        node._set_attr("y", Call("vlayout", node=node))
        
