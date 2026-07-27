import contextvars

from beartype import beartype

from .animtime import Duration, Easing, duration_to_frames

ROOT_OBJECTS = contextvars.ContextVar("root_context", default=None)
CURRENT_NODE = contextvars.ContextVar("node_context", default=None)
COMPOSER = contextvars.ContextVar("composer", default=None)
PENDING_CUE_ADVANCE = contextvars.ContextVar("pending_cue_advance", default=False)


@beartype
def wait(dur: Duration) -> int:
    """
    Advance the current time by the given duration in seconds.
    """
    frames = duration_to_frames(dur)
    return move_frame(frames)


def next_frame() -> int:
    return move_frame(1)


def cue():
    """
    Mark the current frame as a cue point; the player pauses here and waits for user input.

    Also arms a pending one-frame advance: the next instant attribute set, node
    creation, `remove()`, or transition start happens one frame later instead of
    on the cue frame itself. Explicit clock movement (`wait()`, `next_frame()`)
    disarms the pending advance instead of stacking with it.
    """
    node = CURRENT_NODE.get()
    scene = node.get_scene()
    frame = get_frame()
    # A note() right after cue() lands on the cue's own frame (it doesn't flush
    # the pending advance), so segment membership can't be inferred from frame
    # number alone — track a separate ordinal, one per *distinct* cue frame, so
    # notes before/after a cue stay distinguishable even when their frames tie.
    if frame not in scene.cues:
        scene._cue_ordinal += 1
    scene.cues.add(frame)
    PENDING_CUE_ADVANCE.set(True)


def _flush_pending_cue_advance():
    if PENDING_CUE_ADVANCE.get():
        PENDING_CUE_ADVANCE.set(False)
        COMPOSER.get().move_frame(1)


@beartype
def note(text: str):
    """
    Attach a speaker note to the current segment (from the previous cue or scene
    start, to the next cue or scene end). Does not move the clock — several
    `note()` calls in one segment stack as paragraphs in the presenter view.
    """
    node = CURRENT_NODE.get()
    scene = node.get_scene()
    scene.notes.append((get_frame(), scene._cue_ordinal, text))


def get_frame() -> int:
    """
    Return the current frame number.
    """
    return COMPOSER.get().frame


def set_current_node(node):
    CURRENT_NODE.set(node)


def get_current_node():
    return CURRENT_NODE.get()


def add_root_object(obj):
    objs = ROOT_OBJECTS.get()
    if objs is None:
        objs = []
        ROOT_OBJECTS.set(objs)
    objs.append(obj)


def reset_ctx():
    ROOT_OBJECTS.set([])
    CURRENT_NODE.set(None)
    COMPOSER.set(Seq())
    PENDING_CUE_ADVANCE.set(False)


def reset_scene():
    CURRENT_NODE.set(None)
    COMPOSER.set(Seq())
    PENDING_CUE_ADVANCE.set(False)


def move_frame(frames: int):
    PENDING_CUE_ADVANCE.set(False)
    return COMPOSER.get().move_frame(frames)


def end_frame():
    return COMPOSER.get().end_frame()


def reset_frame(frame: int):
    COMPOSER.get().reset_frame(frame)


class Composer:
    def __init__(self, dur: Duration = None, ease: Easing = None):
        self.frame = 0
        self.parent = None
        self.dur = dur
        self.ease = ease

    def reset_frame(self, frame):
        self.frame = frame

    def _begin_unit(self):
        """Hook for `Par(stagger=)`."""

    def __enter__(self):
        assert self.parent is None
        current = COMPOSER.get()
        current._begin_unit()
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

    def __init__(self, stagger: Duration = None):
        super().__init__()
        self.max_frame = 0
        self.stagger = stagger
        self._stagger_frames = duration_to_frames(stagger)
        self._unit_started = False

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

    def _begin_unit(self):
        if self._unit_started:
            self.frame += self._stagger_frames
        else:
            self._unit_started = True

    def __enter__(self):
        super().__enter__()
        self.max_frame = self.frame
        self._unit_started = False
        return self


class anim(Composer):
    """block-scoped `dur`/`ease` defaults"""

    def __init__(self, dur: Duration = None, *, ease: Easing = None):
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

    def _begin_unit(self):
        self.parent._begin_unit()

    def __enter__(self):
        assert self.parent is None
        self.parent = COMPOSER.get()
        COMPOSER.set(self)
        return self


class AnimProxy:
    """Lightweight proxy returned by `Node.anim()`."""

    def __init__(self, node, dur: Duration = None, ease: Easing = None):
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
