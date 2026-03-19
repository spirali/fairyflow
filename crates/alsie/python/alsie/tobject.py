from .expr import HoldExpr, AddExpr, TimedValue, Transition

class TimedObject:

    def __init__(self, frame: int):
        self._frame = frame
        self._start = frame
        self._transition = "sharp"
        self._attrs = {}

    def _add_attr(self, name, value):
        self._attrs[name] = TimedValue(value, self._start)

    def _set_attr(self, name, value):
        self._attrs[name].set(self._frame, value, self._transition)

    def _get_attr(self, name):
        return self._attrs[name]

    def frame(self, frame: int):
        self._frame = frame
        return self

    def transition(self, transition: Transition):
        self._transition = transition
        return self

    def hold(self):
        for tval in self._attrs.values():
            if tval.frames is None or self._frame not in tval.frames:
                tval.set(self._frame, HoldExpr(tval, self._frame), "sharp")
        return self

    def _move_attr(self, name, delta):
        tval = self._attrs[name]
        frame = self._frame
        if tval.frames is not None and frame in tval.frames:
            existing_val, _ = tval.frames[frame]
            tval.frames[frame] = (AddExpr(existing_val, delta), self._transition)
        else:
            tval.set(frame, AddExpr(HoldExpr(tval, frame), delta), self._transition)