import contextvars
from typing import Literal
from . import config
from copy import copy

type Transition = Literal["S", "L"]

class BuildState:

    def __init__(self):
        self._frame = 0
        self._time = 0
        self._transition = "S"
        self._prev = None

    def get_frame(self):
        return self._frame

    def _set_frame(self, frame: int):
        node = CURRENT_NODE.get()
        if node:
            node.get_scene().update_max_frame(frame)
        self._frame = frame

    def set_frame(self, frame: int):
        self._set_frame(frame)
        self._time = (frames_to_time(frame))

    def fwd_frames(self, frames: int):    
        new_frames = self.frames + frames
        self._set_frame(new_frames)
        self._time = frames_to_time(new_frames)

    def set_time(self, time: float):
        self.time = time
        self._set_frame(time_to_frames(time))

    def fwd_time(self, time: float):
        self.time += time        
        self._set_frame(time_to_frames(self.time))

    def transition(self, tr: Transition):
        self._transition = tr


    def __enter__(self):
        assert self._prev is None
        self._prev = B_STATE.get()
        B_STATE.set(self)


    def __exit__(self, exc_type, exc, tb):
        B_STATE.set(self._prev)
        self._prev = None


ROOT_OBJECTS = contextvars.ContextVar("root_context", default=[])
CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
B_STATE = contextvars.ContextVar("B_state", default=BuildState())
  
def time_to_frames(time: float) -> int:
    return int(round(config.FPS * time))

def frames_to_time(frames: int) -> float:
    return frames / config.FPS


def set_frame(frame: int):
    B_STATE.get().set_frame(frame)

def fwd_frames(frames: int):
    B_STATE.get().fwd_frames(frames)
    
def set_time(time: float):
    B_STATE.get().set_time(time)
    
def fwd_time(time: float):
    B_STATE.get().fwd_time(time)

def cue():
    node = CURRENT_NODE.get()
    node.get_scene().cues.add(get_frame())


def step():
    B_STATE.get().transition("S")


def linear():
    B_STATE.get().transition("L")


def get_frame() -> int:
    return B_STATE.get()._frame


def get_transition() -> Transition:
    return B_STATE.get()._transition


def set_current_node(node):
    CURRENT_NODE.set(node)


def get_current_node():
    return CURRENT_NODE.get()


def reset_ctx():
    ROOT_OBJECTS.set([])
    CURRENT_NODE.set(None)
    B_STATE.set(BuildState())


def get_bstate():
    return B_STATE.get()

def bstate():
    return copy(B_STATE.get())