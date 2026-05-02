from .avalue import AnimatedValue, Transition
from .exprs import Call, Inherited
from .ctxvars import adv_time, get_frame, get_transition


class AnimatedObject:
    def __init__(self):
        self._start = get_frame()
        self._end = None
        self._attrs = {}

    def _add_attr(self, name, value):
        self._attrs[name] = AnimatedValue(value, self._start)

    def _add_from_parent(self, name, default=None):
        if name in self._parent._attrs:
            self._add_attr(name, Inherited(self._parent._get_attr(name)))
        else:
            self._add_attr(name, Inherited(default))

    def _set_attr(self, name, value, tr=None):
        self._attrs[name].set(value, tr=tr)

    def _get_attr(self, name):
        return self._attrs[name]
    
    def _has_attr(self, name):
        return name in self._attrs

    def remove(self):
        self._end = get_frame()

    def hold(self):
        for v in self._attrs.values():
            v.hold(get_frame())
        if hasattr(self, "_children"):
            for child in self._children:
                child.hold()
        return self
    
    def _anim_attr(self, name, value, time, start=None):
        if start is None:
            self._get_attr(name).hold()
        else:
            self._set_attr(name, start, "S")
        adv_time(time)
        self._set_attr(name, value, "L")        

    def _move_attr(self, name, delta, transition=None):
        if transition is None:
            transition = get_transition()
        self._attrs[name].move(delta, transition)
