from .exprs import Expr
from .color import Color
from .nodes import Node
from .position import SCENE_NODE_ID

import json as json

_active: "Serializer | None" = None


def serialize_expr(obj):
    if isinstance(obj, Expr):
        return obj.serialize_expr()
    if isinstance(obj, Node):
        # A node with no parent is the Scene (position.py::SCENE_NODE_ID) -
        # it has no wire id of its own, and re-serializing it here would
        # recurse into its own top-level {"nodes": [...], ...} output.
        return SCENE_NODE_ID if obj._parent is None else _active.add_node(obj)
    if isinstance(obj, Color):
        return obj.value
    return obj


class Serializer:
    def __init__(self):
        self.nodes = []
        self._index = {}  # node._id (construction order) -> JSON array index

    def add_node(self, node):
        """Return `node`'s wire-format id, serializing it (and reserving its
        slot) on first reference. Idempotent — safe to call from both the
        normal parent-before-children walk and from a cross-node expression
        reference (`serialize_expr`'s `Node` branch) that happens to reach a
        node before its "natural" position in the walk does."""
        if node._id in self._index:
            return self._index[node._id]
        idx = len(self.nodes)
        self._index[node._id] = idx
        self.nodes.append(None)  # reserve the slot before recursing (cycle-safe)
        self.nodes[idx] = node.serialize(self)
        return idx


def create_export(idx, scene):
    global _active
    if scene._name is None:
        scene._name = f"Scene_{idx + 1}"
    serializer = Serializer()
    _active = serializer
    try:
        return scene.serialize(serializer)
    finally:
        _active = None


def write_tree(objs, path):
    export = {
        "version": 2,
        "scenes": [create_export(idx, obj) for idx, obj in enumerate(objs)],
    }
    with open(path, "w") as f:
        json.dump(export, f)
