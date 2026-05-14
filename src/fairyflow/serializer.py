from .exprs import Expr
from .color import Color
from .nodes import Node

import json as json
import os as os


def serialize_expr(obj):
    if isinstance(obj, Expr):
        return obj.serialize_expr()
    if isinstance(obj, Node):
        return obj._id
    if isinstance(obj, Color):
        return obj.value
    return obj


class Serializer:
    def __init__(self):
        self.nodes = []
        self.info = []

    def add_node(self, node):
        self.nodes.append(node.serialize(self))
        if len(node.info) > 1:
            self.info.append(node.info)
        return node._id


def create_export(idx, scene):
    if scene._name is None:
        scene._name = f"Scene_{idx + 1}"
    serializer = Serializer()
    serialized_scene = scene.serialize(serializer)
    return {
        "scene": serialized_scene,
        "nodes": serializer.nodes,
        "info": serializer.info,
    }


def write_tree(objs, path):
    export = [create_export(idx, obj) for idx, obj in enumerate(objs)]
    with open("/tmp/tree", "w") as f:
        f.write(json.dumps(export))
    with open(path, "w") as f:
        json.dump(export, f)
