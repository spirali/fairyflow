from typing import TypeVar, Generic
from beartype import beartype
from .animtime import Duration, Easing
from .ctxvars import get_frame, wait
from .exprs import Call, Expr


class Hold:
    def __repr__(self):
        return "<HOLD>"


HOLD = Hold()

T = TypeVar("T")


@beartype
class AnimatedValue(Generic[T], Expr):
    def __init__(self, init_val: T, init_frame=None, is_default: bool = False):
        if init_frame is None:
            init_frame = get_frame()
        self.init_frame = init_frame
        self.values = {init_frame: init_val}
        self.transitions = {}
        self.single_value = True
        self.is_default = is_default

    def set(
        self,
        value: T,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ):
        self.is_default = False
        frame = get_frame()
        if dur is not None:
            new_frame = wait(dur)
            if frame != new_frame:
                if frame not in self.values:
                    self.values[frame] = HOLD
                self.transitions[new_frame] = ease or "linear"
                frame = new_frame
            else:
                self.transitions[frame] = "S"
        else:
            self.transitions[frame] = "S"
        self.values[frame] = value
        if frame != self.init_frame:
            self.single_value = False

    def _get(self) -> T:
        frame = get_frame()
        f = max(f for f in self.values if f <= frame and self.values[f] != HOLD)
        return self.values[f]

    def move(
        self,
        delta: T,
        *,
        dur: Duration = None,
        ease: Easing = None,
    ):
        self.set(Call.add(self._get(), delta), dur=dur, ease=ease)

    def is_single_value(self):
        return self.single_value

    def get_first_value(self) -> T:
        return self.values[self.init_frame]

    def serialize(self):
        frames = sorted(self.values)
        tuples = [
            serialize_frame_value(f, self.values[f], self.transitions) for f in frames
        ]
        return {"k": tuples}

    def serialize_expr(self):
        if self.is_single_value():
            from .serializer import serialize_expr

            return serialize_expr(self.get_first_value())
        else:
            return self.serialize()

    def __repr__(self):
        return f"<AV values={self.values} trs={self.transitions}>"


def serialize_frame_value(frame, obj, transitions):
    from .serializer import serialize_expr

    if obj == HOLD:
        return [frame]
    tr = transitions.get(frame, "S")
    if tr == "S":
        return [frame, serialize_expr(obj)]
    return [frame, serialize_expr(obj), tr]
