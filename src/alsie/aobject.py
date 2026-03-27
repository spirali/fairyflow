from .avalue import AnimatedValue, Transition
from .exprs import Call, expr_add, expr_hold


class AnimatedObject:
    def __init__(self, frame: int):
        self._frame = frame
        self._start = frame
        self._transition = "step"
        self._attrs = {}

    def _add_attr(self, name, value):
        self._attrs[name] = AnimatedValue(value, self._start)

    def _set_attr(self, name, value):
        self._attrs[name].set(self._frame, value, self._transition)

    def _get_attr(self, name):
        return self._attrs[name]

    def frame(self, frame: int):
        self._frame = frame
        return self

    def step(self):
        self.transition("step")
        return self

    def linear(self):
        self.transition("linear")
        return self

    def transition(self, transition: Transition):
        self._transition = transition
        return self

    def hold(self):
        for v in self._attrs.values():
            v.hold(self._frame)
        return self

    def _move_attr(self, name, delta):
        self._attrs[name].move(self._frame, delta, self._transition)
