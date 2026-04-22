from typing import TypeVar, Generic
from .ctxvars import get_frame, get_transition, Transition
from .exprs import Call, Expr


class Hold:
    pass


HOLD = Hold()

T = TypeVar('T')

class AnimatedValue(Generic[T], Expr):
    def __init__(self, init_val: T, init_frame=None):
        if init_frame is None:
            init_frame = get_frame()
        self.init_frame = init_frame
        self.values = {init_frame: init_val}
        self.transitions = {}
        self.single_value = True

    def set(self, frame: int, value: T, transition: Transition | None = None):
        if transition is None:
            transition = get_transition()
        self.values[frame] = value
        self.transitions[frame] = transition
        if frame != self.init_frame:
            self.single_value = False

    def hold(self, frame: int | None = None):
        if frame is None:
            frame = get_frame()
        if frame not in self.values:
            self.values[frame] = HOLD

    def move(self, frame, delta, transition=None):
        f = max(f for f in self.values if f <= frame and self.values[f] != HOLD)
        self.set(frame, Call.add(self.values[f], delta), transition)

    def is_single_value(self):
        return self.single_value

    def get_first_value(self) -> T:
        return self.values[self.init_frame]

    def serialize(self):
        values = [
            serialize_frame_value(f, self.values[f], self.transitions)
            for f in self.values
        ]
        return {"values": values}

    def serialize_expr(self):
        if self.is_single_value():
            from .serializer import serialize_expr
            return serialize_expr(self.get_first_value())
        else:
            return self.serialize()



def serialize_frame_value(frame, obj, transitions):
    from .serializer import serialize_expr

    if obj == HOLD:
        return {"frame": frame, "op": "hold"}
    else:
        return {
            "frame": frame,
            "value": serialize_expr(obj),
            "tr": transitions.get(frame, "S"),
        }
