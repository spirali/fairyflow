class Expr:
    def __add__(self, other):
        return Call.add(self, other)

    def __sub__(self, other):
        return Call.sub(self, other)

    def __mul__(self, other):
        return Call.mul(self, other)

    def __truediv__(self, other):
        return Call.div(self, other)


class Call(Expr):
    """An `["op", arg, ...]` s-expr call. `op` is the wire-format op name
    (renamed from v1's `{"fn": ...}` object shape — see api-v2-impl.md §A.4);
    `args` are positional, matching the Rust-side arity table exactly."""

    def __init__(self, op, *args):
        self.op = op
        self.args = args

    def serialize_expr(self):
        from .serializer import serialize_expr

        return [self.op] + [serialize_expr(a) for a in self.args]

    @staticmethod
    def add(a, b):
        return Call("+", a, b)

    @staticmethod
    def sub(a, b):
        return Call("-", a, b)

    @staticmethod
    def mul(a, b):
        return Call("*", a, b)

    @staticmethod
    def div(a, b):
        return Call("/", a, b)

    @staticmethod
    def default_x(node):
        return Call("auto_x", node)

    @staticmethod
    def default_y(node):
        return Call("auto_y", node)

    @staticmethod
    def default_width(node):
        return Call("auto_w", node)

    @staticmethod
    def default_height(node):
        return Call("auto_h", node)

    @staticmethod
    def path_x(node, t):
        return Call("path_x", node, t)

    @staticmethod
    def path_y(node, t):
        return Call("path_y", node, t)

    @staticmethod
    def norm(a, b):
        return Call("norm", a, b)

    @staticmethod
    def path_length(path):
        return Call("path_len", path)

    def __repr__(self):
        return f"<Call {self.op} {self.args}>"


class Const(Expr):
    def __init__(self, value):
        self.value = value

    def serialize_expr(self):
        return self.value


class Inherited(Expr):
    """Marks a not-yet-set inherited attribute. No longer has its own wire
    shape — absence *is* the inherited signal now (`api-v2-impl.md` §A.4) — so
    this only exists as an internal placeholder; `Node.serialize` (nodes.py)
    omits attributes still marked default before `serialize_expr` ever sees
    them. Kept as a transparent passthrough purely as a defensive fallback in
    case one is ever embedded cross-node."""

    def __init__(self, expr):
        self.expr = expr

    def serialize_expr(self):
        from .serializer import serialize_expr

        return serialize_expr(self.expr)


def to_expr(obj):
    if isinstance(obj, Expr):
        return obj
    return Const(obj)
