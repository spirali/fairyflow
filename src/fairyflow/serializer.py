from .exprs import Expr
from .color import Color
from .nodes import Node

import json as json
import os as os


def serialize_expr(obj):
    if isinstance(obj, Expr):
        return obj.serialize_expr()
    if isinstance(obj, Node):
        return {"id": obj._id}
    if isinstance(obj, Color):
        return {"color": obj.value}
    return obj


class Serializer:
    def __init__(self):
        self.animated_values = []
        self.nodes = []
        self.info = []

    def add_av(self, av):
        self.animated_values.append(av.serialize())
        return id(av)

    def add_node(self, node):
        self.nodes.append(node.serialize(self))
        self.info.append(node.info)
        return node._id


def create_export(idx, scene):
    if scene.name is None:
        scene.name = f"Scene_{idx + 1}"
    serializer = Serializer()
    serialized_scene = scene.serialize(serializer)    
    return {
        "scene": serialized_scene,
        "animated_values": serializer.animated_values,
        "nodes": serializer.nodes,
        "info": serializer.info
    }


def write_tree(objs, path):
    export = [create_export(idx, obj) for idx, obj in enumerate(objs)]
    print(json.dumps(export))
    with open(path, "w") as f:
        json.dump(export, f)
