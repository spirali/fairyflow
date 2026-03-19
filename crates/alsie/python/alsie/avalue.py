from .exprs import expr_add, expr_hold

type Transition = Literal["sharp", "linear"]

class Hold():
    pass

HOLD = Hold()

class AnimatedValue:

    def __init__(self, init_val, init_frame):
        self.init_frame = init_frame
        self.values = {init_frame: init_val}
        self.transitions = {}

    def set(self, frame: int, value, transition: Transition):
        self.values[frame] = value
        self.transitions[frame] = transition

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

    def serialize(self):
        from .serializer import serialize_expr       
        if len(self.values) == 1:
            return { "kind": "const", "id": id(self), "value": serialize_expr(self.values[self.init_frame])}
        values = [serialize_frame_value(f, self.values[f], self.transitions) for f in self.values]
        return {"kind": "animated", "id": id(self), "values": values}
    

def serialize_frame_value(frame, obj, transitions):
    from .serializer import serialize_expr
    if obj == HOLD:
        return {"frame": frame, "op": "hold"}
    else:
        return {"frame": frame, "value": serialize_expr(obj), "tr": transitions.get(frame, "sharp")}