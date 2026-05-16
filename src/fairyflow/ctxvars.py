import contextvars
from beartype import beartype

from .animtime import Transition, tr_to_frames

ROOT_OBJECTS = contextvars.ContextVar("root_context", default=[])
CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
COMPOSER = contextvars.ContextVar("composer", default=None)


@beartype
def wait(tr: Transition) -> int:
    """
    Advance the current time by the given duration in seconds.
    """
    frames = tr_to_frames(tr)
    return COMPOSER.get().move_frame(frames)


def next_frame() -> int:
    return COMPOSER.get().move_frame(1)


def cue():
    """
    Mark the current frame as a cue point; the player pauses here and waits for user input.
    """
    node = CURRENT_NODE.get()
    node.get_scene().cues.add(get_frame())


def get_frame() -> int:
    """
    Return the current frame number.
    """
    return COMPOSER.get().frame


def set_current_node(node):
    CURRENT_NODE.set(node)


def get_current_node():
    return CURRENT_NODE.get()


def reset_ctx():
    ROOT_OBJECTS.set([])
    CURRENT_NODE.set(None)
    COMPOSER.set(Seq())


def move_frame(frames: int):
    return COMPOSER.get().move_frame(frames)


def process_tr(tr: Transition):
    frames = tr_to_frames(tr)
    return COMPOSER.get().move_frame(frames)


def end_frame():
    return COMPOSER.get().end_frame()


class Seq:
    """
    Sequential composition of animations
    """

    def __init__(self):
        self.frame = 0
        self.parent = None

    def move_frame(self, frame):
        self.frame += frame
        return self.frame

    def join_frame(self, frame):
        self.frame = frame

    def end_frame(self):
        return self.frame

    def __enter__(self):
        assert self.parent is None
        current = COMPOSER.get()
        self.frame = current.frame
        self.parent = current
        COMPOSER.set(self)

    def __exit__(self, exc_type, exc, tb):
        assert self.parent is not None
        self.parent.join_frame(self.frame)
        COMPOSER.set(self.parent)
        self.parent = None


class Par:
    """
    Parallel composition of animations
    """

    def __init__(self):
        self.frame = 0
        self.max_frame = 0
        self.parent = None

    def move_frame(self, frame):
        new_frame = self.frame + frame
        self.max_frame = max(new_frame, self.max_frame)
        return new_frame

    def join_frame(self, frame):
        self.max_frame = max(frame, self.max_frame)

    def process_target_frame(self, frame):
        self.frame = frame

    def end_frame(self):
        return self.max_frame

    def __enter__(self):
        assert self.parent is None
        current = COMPOSER.get()
        self.frame = current.frame
        self.max_frame = current.frame
        self.parent = current
        COMPOSER.set(self)

    def __exit__(self, exc_type, exc, tb):
        assert self.parent is not None
        self.parent.join_frame(self.max_frame)
        COMPOSER.set(self.parent)
        self.parent = None
