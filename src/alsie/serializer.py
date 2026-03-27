from .exprs import Expr
from .avalue import AnimatedValue
from .color import Color
from .nodes import Node

import json as json
import os as os


def serialize_expr(obj):
    if isinstance(obj, Expr):
        return obj.serialize_expr()
    if isinstance(obj, AnimatedValue):
        if obj.is_single_value():
            return serialize_expr(obj.get_first_value())
        else:
            return {"av": id(obj)}
    if isinstance(obj, Node):
        return {"id": obj._id}
    if isinstance(obj, Color):
        return {"color": obj.value}
    return obj


class Serializer:
    def __init__(self):
        self.animated_values = []
        self.nodes = []

    def add_av(self, av):
        self.animated_values.append(av.serialize())
        return id(av)

    def add_node(self, node):
        self.nodes.append(node.serialize(self))
        return node._id


def create_export(scene):
    serializer = Serializer()
    serialized_scene = scene.serialize(serializer)
    return {
        "scene": serialized_scene,
        "animated_values": serializer.animated_values,
        "nodes": serializer.nodes,
    }


def write_tree(path):
    from .nodes import ROOT_OBJECT

    export = create_export(ROOT_OBJECT)
    print(export)
    print(json.dumps(export))
    with open(path, "w") as f:
        json.dump(export, f)
