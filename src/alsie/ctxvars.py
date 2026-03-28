import contextvars
from .avalue import Transition

ROOT_OBJECT = contextvars.ContextVar("root_context", default=None)

CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
FRAME = contextvars.ContextVar[int]("frame", default=0)
TRANSITION = contextvars.ContextVar[Transition]("transition", default="step")


def transition(tr: Transition):
    TRANSITION.set(tr)


def frame(frame: int):
    FRAME.set(frame)

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

def store_ctx():
    return CURRENT_NODE.get(), FRAME.get(), TRANSITION.get()


def restore_ctx(ctx):
    node, frame, tr = ctx
    CURRENT_NODE.set(node)
    FRAME.set(frame)
    TRANSITION.set(tr)

def reset_ctx(ctx):
    ROOT_OBJECT.set(None)
    CURRENT_NODE.set(None)
    FRAME.set(0)
    TRANSITION.set("step")