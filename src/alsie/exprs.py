
from alsie.color import Color


class Expr:

    def __add__(self, other):
        return Expr("+", a=self, b=other)
    

class Call(Expr):
    def __init__(self, name, **kwargs):
        self.name = name
        self.args = kwargs

    def serialize_expr(self):
        from .serializer import serialize_expr
        r = {
            "kind": "call",
            "fn": self.name 
        }
        for k in self.args:
            r[k] = serialize_expr(self.args[k])
        return r
    
    def __repr__(self):
        return f"<Call {self.name} {self.args}>"


def expr_add(a, b):
    return Call("+", a=a, b=b)

def expr_hold(av):
    return Call("hold", av=av)


def expr_default_x(node):
    return Call("default_x", node=node)


def expr_default_y(node):
    return Call("default_y", node=node)