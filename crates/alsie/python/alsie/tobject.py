from .expr import TimedValue, Transition

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