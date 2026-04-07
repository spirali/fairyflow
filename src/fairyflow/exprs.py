from fairyflow.color import Color


class Expr:
    def __add__(self, other):
        return expr_add(self, other)
    def __sub__(self, other):
        return expr_sub(self, other)    
    def __mul__(self, other):
        return expr_mul(self, other)

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

    def __repr__(self):
        return f"<Call {self.name} {self.args}>"


class InheritedExprs(Expr):
    def __init__(self, expr):
        self.expr = expr

    def serialize_expr(self):
        from .serializer import serialize_expr

        return {"kind": "inherited", "expr": serialize_expr(self.expr)}


def expr_add(a, b):
    return Call("+", a=a, b=b)


def expr_sub(a, b):
    return Call("-", a=a, b=b)


def expr_mul(a, b):
    return Call("*", a=a, b=b)


def expr_hold(av):
    return Call("hold", av=av)


def expr_default_x(node):
    return Call("default_x", node=node)


def expr_default_y(node):
    return Call("default_y", node=node)


def expr_default_width(node):
    return Call("default_width", node=node)


def expr_default_height(node):
    return Call("default_height", node=node)


def expr_follow_path_x(node, start_frame, end_frame):
    return Call("follow_path_x", node=node, start_frame=start_frame, end_frame=end_frame)

def expr_follow_path_y(node, start_frame, end_frame):
    return Call("follow_path_y", node=node, start_frame=start_frame, end_frame=end_frame)

def expr_norm(a, b):
    return Call("norm", a=a, b=b)