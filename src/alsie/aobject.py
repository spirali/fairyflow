from .avalue import AnimatedValue, Transition
from .exprs import Call, InheritedExprs, expr_add, expr_hold
from .ctxvars import get_frame, get_transition


class AnimatedObject:
    def __init__(self, frame: int):
        self._start = frame
        self._end = None
        self._attrs = {}

    def _add_attr(self, name, value):
        self._attrs[name] = AnimatedValue(value, self._start)

    def _add_from_parent(self, name, default=None):
        if name in self._parent._attrs:
            self._add_attr(name, InheritedExprs(self._parent._get_attr(name)))
        else:
            self._add_attr(name, InheritedExprs(default))

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
