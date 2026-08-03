import json
import math

from .avalue import AnimatedValue
from .color import Color
from .exprs import Call, Const, Expr
from .nodes import Node
from .position import SCENE_NODE_ID

_active: "Serializer | None" = None

_FOLDABLE_OPS = {"+", "-", "*", "/", "norm", "max", "neg"}


def _try_fold(obj):
    """Attempt to reduce `obj` to a plain number via constant folding, so
    expressions built entirely from literals and non-animated attributes
    (common for shape-sugar geometry math, e.g. `Arrow`'s gap/head
    computation) serialize as a single number instead of a nested `Call`
    tree. An attribute reference (`AnimatedValue`) folds too, exactly when
    it was never animated (`is_single_value()`) - the same condition
    `AnimatedValue.serialize_expr()` already uses to skip the keyframe
    table, just extended through arithmetic built on top of it.

    Returns `None` for anything that depends on live scene/animation state
    and can't be resolved without the engine (an `auto_x`/`map_x`/`path_x`
    -style `Call`, or a multi-keyframe value) - the caller then falls back
    to today's structural serialization.
    """
    if isinstance(obj, bool):
        return None
    if isinstance(obj, (int, float)):
        return obj
    if isinstance(obj, Const):
        return _try_fold(obj.value)
    if isinstance(obj, AnimatedValue):
        if not obj.is_single_value():
            return None
        return _try_fold(obj.get_first_value())
    if isinstance(obj, Call):
        if obj.op not in _FOLDABLE_OPS:
            return None
        if obj.op == "neg":
            a = _try_fold(obj.args[0])
            return None if a is None else -a
        a = _try_fold(obj.args[0])
        b = _try_fold(obj.args[1])
        if a is None or b is None:
            return None
        if obj.op == "+":
            return a + b
        if obj.op == "-":
            return a - b
        if obj.op == "*":
            return a * b
        if obj.op == "/":
            return 0.0 if abs(b) < 0.000001 else a / b
        if obj.op == "max":
            return max(a, b)
        d = a * a + b * b  # "norm"
        return 0.0 if d < 0.0001 else a / math.sqrt(d)
    return None


def serialize_expr(obj):
    folded = _try_fold(obj)
    if folded is not None:
        return folded
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
