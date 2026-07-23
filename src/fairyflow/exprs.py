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
    def __init__(self, op, *args):
        self.op = op
        self.args = args

    def serialize_expr(self):
        from .serializer import serialize_expr

        args = [serialize_expr(a) for a in self.args]
        args.insert(0, self.op)
        return args

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
    def auto_x(node):
        return Call("auto_x", node)

    @staticmethod
    def auto_y(node):
        return Call("auto_y", node)

    @staticmethod
    def auto_width(node):
        return Call("auto_w", node)

    @staticmethod
    def auto_height(node):
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
    def max(a, b):
        return Call("max", a, b)

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


def to_expr(obj):
    if isinstance(obj, Expr):
        return obj
    return Const(obj)
