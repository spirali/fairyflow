import contextvars
from beartype import beartype

from .animtime import Duration, Easing, duration_to_frames

ROOT_OBJECTS = contextvars.ContextVar("root_context", default=[])
CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
COMPOSER = contextvars.ContextVar("composer", default=None)


@beartype
def wait(dur: Duration) -> int:
    """
    Advance the current time by the given duration in seconds.
    """
    frames = duration_to_frames(dur)
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


def reset_scene():
    CURRENT_NODE.set(None)
    COMPOSER.set(Seq())


def move_frame(frames: int):
    return COMPOSER.get().move_frame(frames)


def end_frame():
    return COMPOSER.get().end_frame()


def reset_frame(frame: int):
    COMPOSER.get().reset_frame(frame)


class Composer:
    """Shared base for `Seq`/`Par`: the composer stack (`.parent`), the
    optional per-block `dur`/`ease` defaults consulted by `_set_attr` when a
    call doesn't specify its own (api-v2-proposal.md §3.4), and the
    enter/exit bookkeeping that pushes/pops `COMPOSER`."""

    def __init__(self, dur: Duration = None, ease: Easing = None):
        self.frame = 0
        self.parent = None
        self.dur = dur
        self.ease = ease

    def reset_frame(self, frame):
        self.frame = frame

    def __enter__(self):
        assert self.parent is None
        current = COMPOSER.get()
        self.frame = current.frame
        self.parent = current
        COMPOSER.set(self)
        return self

    def __exit__(self, exc_type, exc, tb):
        assert self.parent is not None
        self.parent.join_frame(self.end_frame())
        COMPOSER.set(self.parent)
        self.parent = None


class Seq(Composer):
    """
    Sequential composition of animations
    """

    def __init__(self):
        super().__init__()

    def move_frame(self, frame):
        self.frame += frame
        return self.frame

    def join_frame(self, frame):
        self.frame = frame

    def end_frame(self):
        return self.frame


class Par(Composer):
    """
    Parallel composition of animations
    """

    def __init__(self):
        super().__init__()
        self.max_frame = 0

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
        super().__enter__()
        self.max_frame = self.frame
        return self


class anim(Composer):
    """Global context manager (api-v2-proposal.md §3.4): block-scoped
    `dur`/`ease` defaults, consulted by `_set_attr`/`_move_attr` (via
    `aobject.py::_resolve_dur_ease`) for any setter call inside that doesn't
    specify its own. Composition-transparent — it carries no frame state of
    its own and delegates every clock operation to its parent, so nesting it
    inside/around a `Par`/`Seq` doesn't change that block's composition
    semantics (`with Par(), anim(0.5):` still runs its children in
    parallel)."""

    def __init__(self, dur: Duration, *, ease: Easing = None):
        # Deliberately not calling Composer.__init__: it does `self.frame =
        # 0`, which would hit the read-only `frame` property below.
        self.parent = None
        self.dur = dur
        self.ease = ease

    @property
    def frame(self):
        return self.parent.frame

    def move_frame(self, frame):
        return self.parent.move_frame(frame)

    def join_frame(self, frame):
        self.parent.join_frame(frame)

    def end_frame(self):
        return self.parent.end_frame()

    def reset_frame(self, frame):
        self.parent.reset_frame(frame)

    def __enter__(self):
        assert self.parent is None
        self.parent = COMPOSER.get()
        COMPOSER.set(self)
        return self


class AnimProxy:
    """Lightweight fluent proxy returned by `Node.anim()` (api-v2-proposal.md
    §3.3): pre-fills `dur`/`ease` on every chained setter call and runs them
    in parallel, without opening a real nested `Composer` (which would need
    an explicit close that a bare fluent chain has no hook for). Instead it
    remembers the frame the chain started at and rewinds the ambient
    composer back to it before each call after the first, so every call
    starts from the same base frame — the same effect `Par` achieves, without
    the lifecycle."""

    def __init__(self, node, dur: Duration, ease: Easing = None):
        self._node = node
        self._dur = dur
        self._ease = ease
        self._base_frame = get_frame()
        self._started = False

    def __getattr__(self, name):
        method = getattr(self._node, name)

        def wrapper(*args, **kwargs):
            if self._started:
                reset_frame(self._base_frame)
            self._started = True
            kwargs.setdefault("dur", self._dur)
            kwargs.setdefault("ease", self._ease)
            method(*args, **kwargs)
            return self

        return wrapper
