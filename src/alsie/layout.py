

class CenteringLayout:

    def set_node_position(self, node):
        node.align_x(0.5)
        node.align_y(0.5)


centering_layout = CenteringLayout()