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


# from typing import Literal
# from .expr import eval_value

# type Transition = Literal["sharp", "linear"]

# class FrameObject:
#     kind = "fobject"
#     property_defaults = {}

#     def __init__(self, frame: int):
#         self._properties = {frame: {}}
#         self._start = frame
#         self._frame = frame
#         self._transition = "sharp"

#     def frame(self, frame: int):
#         self._frame = frame
#         if frame not in self._properties:
#             self._properties[frame] = {}
#         return self
    
#     def transition(self, transition: Transition):
#         self._transition = self.transition
#         return self

#     def set_property(self, name: str, value):
#         self._properties[self._frame][name] = (value, self._transition)

#     def get_property(self, name: str):
#         frame = self.frame
#         value = self._properties.get(self.frame)
#         if value is None:
#             return GetExpr(self, frame, name)
#         else:
#             return value[0]
        
#     def eval_property(self, frame, name, default):
#         p = self._properties
#         f = max((f for f in p if f <= frame and (name in p[f])), default=None)
#         if f is None:
#             f = self._start
#             v = default
#         else:
#             v = eval_value(p[f][name][0], frame)
#         if f == frame:
#             return v
        
#         f2 = min((f for f in p if f > frame and (name in p[f])), default=None)
#         if f2 is None:
#             return v
#         v2, transition = p[f2][name]
#         print(transition, name)
#         if transition == "sharp":
#             return v
#         v2 = eval_value(v2, frame)
#         t = (frame - f) / (f2 - f)
#         return t * (v2 - v) + v

#         return v
    
#     def eval_at_frame(self, frame):
#         result = {"kind": self.kind}
#         for name in self.property_defaults:
#             result[name] = self.eval_property(frame, name, self.property_defaults[name])
#         return result

#     def dump(self):
#         return {
#             "kind": self.kind,
#             "props": [
#                 {"frame": f, "values": self._properties[f]} for f in self._properties
#             ],
#         }
    
#     def key_frames(self, out: set):
#         for frame in self._properties:
#             out.add(frame)