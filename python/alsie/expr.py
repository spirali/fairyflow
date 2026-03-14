from typing import TypeVar, Callable

type Transition = Literal["sharp", "linear"]
    

class EvalCtx:

    def __init__(self, frame):
        self.frame = frame

    def eval_obj(self, obj):
        if isinstance(obj, BaseExpr):
            return obj.eval(self)
        else:
            return obj


class BaseExpr:
    def __add__(self, other):
        return BinOpExpr("+", self, other)

    def map(self, fn):
        return MapExpr(self, fn)

    def eval(self, ctx: EvalCtx):
        raise NotImplemented
    
  
class DynExpr(BaseExpr):
    def __init__(self, fn):
        self.fn = fn
    
    def eval(self, ctx: EvalCtx):
        return self.fn(ctx)


class Const(BaseExpr):
    def __init__(self, const_val):
        self.const_val = const_val

    def eval(self, ctx: EvalCtx):
        return self.const_val


class TimedValue(BaseExpr):

    def __init__(self, init_val, init_frame):
        self.init_val = init_val
        self.init_frame = init_frame
        self.frames = None

    def set(self, frame: int, value, transition: Transition):
        if self.frames is None:
            self.frames = {}
        self.frames[frame] = (value, transition)

    def get_or_self(self, frame):
        if self.frames and frame in self.frames:
            return self.frames[frame]
        else:
            return self
        
    def key_frames(self, out):
        if self.frames:
            out.update(self.frames)

    def eval(self, ctx: EvalCtx):
        if self.frames is None:
            return ctx.eval_obj(self.init_val)
        frame = ctx.frame
        f = max((f for f in self.frames if f <= frame), default=None)
        if f is None:
            f = self.init_frame
            v = ctx.eval_obj(self.init_val)
        else:
            v = ctx.eval_obj(self.frames[f][0])
        if f == frame:                        
            return v        
        f2 = min((f for f in self.frames if f > frame), default=None)
        if f2 is None:
            return v
        v2, transition = self.frames[f2]
        if transition == "sharp":
            return v
        v2 = ctx.eval_obj(v2)
        t = (frame - f) / (f2 - f)
        return t * (v2 - v) + v


class BinOpExpr(BaseExpr):
    def __init__(self, op, expr1, expr2):
        self.op = op
        self.expr1 = expr1
        self.expr2 = expr2

    def eval(self, ctx):
        raise Exception("TODO")
        # v1 = ctx.eval_obj(self.expr1) 
        # v2 = eval_value(self.expr2, frame)
        # if self.op == "+":
        #     return v1 + v2
        # elif self.op == "*":
        #     return v1 * v2
        # else:
        #     raise ValueError(f"Invalid operation {self.op}")
        
class MapExpr(BaseExpr):
    def __init__(self, expr, fn):
        self.expr = expr
        self.fn = fn

    def eval(self, ctx: EvalCtx):
        return self.fn(ctx.eval_obj(self.expr))


class Tuple(BaseExpr):
    def __init__(self, exprs: tuple):
        self.exprs = exprs

    def eval(self, ctx):
        return tuple(ctx.eval_obj(e) for e in self.exprs)