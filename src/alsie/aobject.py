from .avalue import AnimatedValue, Transition
from .exprs import Call, expr_add, expr_hold

import contextvars

FRAME = contextvars.ContextVar[int]("frame", default=0)
TRANSITION = contextvars.ContextVar[Transition]("transition", default="step")

def reset_frame():
    frame(0)
    step()

def frame(frame: int):
    FRAME.set(frame)


def transition(transition: Transition):
    TRANSITION.set(transition)


def step():
    transition("step")


def linear():
    transition("linear")


def get_frame() -> int:
    return FRAME.get()


def get_transition() -> Transition:
    return TRANSITION.get()


class AnimatedObject:
    def __init__(self, frame: int):
        self._start = frame
        self._end = None
        self._attrs = {}

    def _add_attr(self, name, value):
        self._attrs[name] = AnimatedValue(value, self._start)

    def _set_attr(self, name, value):
        self._attrs[name].set(get_frame(), value, get_transition())

    def _get_attr(self, name):
        return self._attrs[name]

    def remove(self):
        self._end = get_frame()

    def hold(self):
        for v in self._attrs.values():
            v.hold(get_frame())
        return self

    def _move_attr(self, name, delta):
        self._attrs[name].move(get_frame(), delta, get_transition())
