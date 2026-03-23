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
    def __init__(self, name, **kwargs):
        self.name = name
        self.args = kwargs

    def serialize_expr(self):
        from .serializer import serialize_expr

        r = {"fn": self.name}
        for k in self.args:
            r[k] = serialize_expr(self.args[k])
        return r

    @staticmethod
    def add(a, b):
        return Call("+", a=a, b=b)

    @staticmethod
    def sub(a, b):
        return Call("-", a=a, b=b)

    @staticmethod
    def mul(a, b):
        return Call("*", a=a, b=b)

    @staticmethod
    def div(a, b):
        return Call("/", a=a, b=b)

    @staticmethod
    def hold(av):
        return Call("hold", av=av)

    @staticmethod
    def default_x(node):
        return Call("default_x", node=node)

    @staticmethod
    def default_y(node):
        return Call("default_y", node=node)

    @staticmethod
    def default_width(node):
        return Call("default_width", node=node)

    @staticmethod
    def default_height(node):
        return Call("default_height", node=node)

    @staticmethod
    def path_x(node, t):
        return Call("path_x", node=node, t=t)

    @staticmethod
    def path_y(node, t):
        return Call("path_y", node=node, t=t)

    @staticmethod
    def norm(a, b):
        return Call("norm", a=a, b=b)

    @staticmethod
    def path_length(path):
        return Call("path_length", node=path._id)

    def __repr__(self):
        return f"<Call {self.name} {self.args}>"


class Const(Expr):
    def __init__(self, value):
        self.value = value

    def serialize_expr(self):
        return self.value


class Inherited(Expr):
    def __init__(self, expr):
        self.expr = expr

    def serialize_expr(self):
        from .serializer import serialize_expr

        return {"kind": "inherited", "expr": serialize_expr(self.expr)}


def to_expr(obj):
    if isinstance(obj, Expr):
        return obj
    return Const(obj)
