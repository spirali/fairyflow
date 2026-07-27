from .avalue import AnimatedValue
from .ctxvars import COMPOSER, _flush_pending_cue_advance, get_frame
from .sentinels import INHERITED_VALUE


def _resolve_dur_ease(dur, ease):
    if dur is None:
        composer = COMPOSER.get()
        while composer is not None and composer.dur is None:
            composer = composer.parent
        if composer is not None:
            dur = composer.dur
    if ease is None:
        composer = COMPOSER.get()
        while composer is not None and composer.ease is None:
            composer = composer.parent
        if composer is not None:
            ease = composer.ease
    return dur, ease


class AnimatedObject:
    """Attributes are created lazily: `_attrs` starts empty, and an entry only
    comes into existence the first time it's read or written (`_ensure_attr`).
    This is what makes "every node can rotate/scale/alpha/z" free — nothing is
    paid for until it's actually used — and it's what makes the wire format
    sparse (`Node.serialize`, nodes.py, just iterates whatever ended up in
    `_attrs`, which is naturally only the touched ones)."""

    def __init__(self):
        _flush_pending_cue_advance()
        self._start = get_frame()
        self._end = None
        self._attrs = {}

    def _default_for(self, name):
        for klass in type(self).__mro__:
            defaults = klass.__dict__.get("_ATTR_DEFAULTS")
            if defaults is None or name not in defaults:
                continue
            factory = defaults[name]
            if factory is INHERITED_VALUE:
                return self._parent._get_attr(name)
            if callable(factory):
                return factory(self)
            return factory
        raise AttributeError(
            f"attribute {name!r} has no default and must be set explicitly"
        )

    def _add_attr(self, name, value):
        """Seed a required attribute that has no meaningful default (text
        content, image path, control-point offsets that are always passed
        explicitly, ...). Unlike `_set_attr`, this never consults
        `_default_for` — there's nothing to fall back to, and the attribute
        is always present in the wire output."""
        self._attrs[name] = AnimatedValue(value, self._start)

    def _ensure_attr(self, name):
        if name not in self._attrs:
            self._attrs[name] = AnimatedValue(
                self._default_for(name), self._start, is_default=True
            )
        return self._attrs[name]

    def _set_attr(self, name, value, dur=None, ease=None):
        _flush_pending_cue_advance()
        COMPOSER.get()._begin_unit()
        dur, ease = _resolve_dur_ease(dur, ease)
        self._ensure_attr(name).set(value, dur=dur, ease=ease)

    def _get_attr(self, name):
        return self._ensure_attr(name)

    def _has_attr(self, name):
        return name in self._attrs

    def remove(self):
        _flush_pending_cue_advance()
        self._end = get_frame()

    def _move_attr(self, name, delta, dur=None, ease=None):
        _flush_pending_cue_advance()
        COMPOSER.get()._begin_unit()
        dur, ease = _resolve_dur_ease(dur, ease)
        self._ensure_attr(name).move(delta, dur=dur, ease=ease)
