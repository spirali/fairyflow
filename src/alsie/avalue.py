from .exprs import expr_add, expr_hold

type Transition = Literal["step", "linear"]


class Hold:
    pass


HOLD = Hold()


class AnimatedValue:
    def __init__(self, init_val, init_frame):
        self.init_frame = init_frame
        self.values = {init_frame: init_val}
        self.transitions = {}
        self.single_value = True

    def set(self, frame: int, value, transition: Transition):
        self.values[frame] = value
        self.transitions[frame] = transition
        if frame != self.init_frame:
            self.single_value = False

    def hold(self, frame: int):
        if frame not in self.values:
            self.values[frame] = HOLD

    def get_ignore_hold(self, frame):
        if frame in self.values:
            v = self.values[frame]
            if v != HOLD:
                return v
        return None

    def move(self, frame, delta, transition):
        f = max(f for f in self.values if f <= frame and self.values[f] != HOLD)
        self.set(frame, expr_add(self.values[f], delta), transition)

    def is_single_value(self):
        return self.single_value

    def get_first_value(self):
        return self.values[self.init_frame]

    def serialize(self):
        values = [
            serialize_frame_value(f, self.values[f], self.transitions)
            for f in self.values
        ]
        return {"id": id(self), "values": values}


def serialize_frame_value(frame, obj, transitions):
    from .serializer import serialize_expr

    if obj == HOLD:
        return {"frame": frame, "op": "hold"}
    else:
        return {
            "frame": frame,
            "value": serialize_expr(obj),
            "tr": transitions.get(frame, "step"),
        }
