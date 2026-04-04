import contextvars
from .avalue import Transition
from . import config

ROOT_OBJECTS = contextvars.ContextVar("root_context", default=[])

CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
FRAME = contextvars.ContextVar[int]("frame", default=0)
TRANSITION = contextvars.ContextVar[Transition]("transition", default="step")


def set_frame(frame: int):
    node = CURRENT_NODE.get()
    if node:
        node.get_scene().update_max_frame(frame)
    FRAME.set(frame)


def jump_frames(frames: int):
    set_frame(FRAME.get() + frames)


def set_time(time: float):
    set_frame(int(round(config.FPS * time)))


def jump_time(time: float):
    jump_frames(int(round(config.FPS * time)))


def cue():
    node = CURRENT_NODE.get()
    node.get_scene().cues.add(get_frame())


def transition(tr: Transition):
    TRANSITION.set(tr)


def step():
    transition("step")


def linear():
    transition("linear")


def get_frame() -> int:
    return FRAME.get()


def get_transition() -> Transition:
    return TRANSITION.get()


def set_current_node(node):
    CURRENT_NODE.set(node)


def get_current_node():
    return CURRENT_NODE.get()


def reset_ctx(ctx):
    ROOT_OBJECTS.set([])
    CURRENT_NODE.set(None)
    FRAME.set(0)
    TRANSITION.set("step")


class FContext:

    def __init__(self):
        self.frame = None
        self.transition = None

    def __enter__(self):
        self.frame = FRAME.get()
        self.transition = TRANSITION.get()

    def __exit__(self, exc_type, exc, tb):
        FRAME.set(self.frame)
        TRANSITION.set(self.transition)


def fctx():
    return FContext()