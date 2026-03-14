import json as _json
import os as _os
import atexit as _atexit

_roots = []
_ctx = None
_all_steps = {0}

# Properties valid for each node type. _set() silently ignores anything not listed.
_TYPE_PROPS = {
    "node": {"width", "height", "color", "x", "y"},
    "rect": {"width", "height", "color", "x", "y"},
    "circle": {"radius", "color", "x", "y"},
}


class _Node:
    def __init__(self, ntype, start=0):
        self._type = ntype
        self._start = start
        self._end = None  # None = never removed
        self._t = start  # time cursor starts at creation step
        self._transition = "sharp"  # current transition mode
        self._keyframes = {}  # {int_t: {prop: {value, transition}}}
        self._children = []
        self._parent_ctx = None

    # ── internal ───────────────────────────────────────────────────────────
    def _set(self, key, val):
        if key not in _TYPE_PROPS.get(self._type, set()):
            return self  # silently ignore unsupported props
        _all_steps.add(self._t)
        self._keyframes.setdefault(self._t, {})[key] = {
            "value": val,
            "transition": self._transition,
        }
        return self

    def _resolve(self, key):
        """Last raw value of key in keyframes with t strictly less than self._t."""
        val = None
        for t in sorted(k for k in self._keyframes if k < self._t):
            entry = self._keyframes[t].get(key)
            if entry is not None:
                val = entry["value"]
        return val

    # ── time ───────────────────────────────────────────────────────────────
    def at(self, t):
        _all_steps.add(t)
        self._t = t
        return self

    # ── transition mode ────────────────────────────────────────────────────
    def sharp(self):
        self._transition = "sharp"
        return self

    def smooth(self):
        self._transition = "smooth"
        return self

    # ── lifetime ────────────────────────────────────────────────────────────
    def remove(self):
        _all_steps.add(self._t)
        self._end = self._t
        return self

    # ── chainable property setters ─────────────────────────────────────────
    def size(self, w, h):
        self._set("width", w)
        self._set("height", h)
        return self

    def color(self, c):
        return self._set("color", c)

    def radius(self, r):
        return self._set("radius", r)

    def position(self, x, y):
        self._set("x", x)
        self._set("y", y)
        return self

    def move(self, dx, dy):
        self._set("x", (self._resolve("x") or 0) + dx)
        self._set("y", (self._resolve("y") or 0) + dy)
        return self

    # ── context manager (nesting) ──────────────────────────────────────────
    def __enter__(self):
        global _ctx
        self._parent_ctx = _ctx
        _ctx = self
        return self

    def __exit__(self, *_):
        global _ctx
        _ctx = self._parent_ctx

    def _to_dict(self):
        kf = {str(t): props for t, props in sorted(self._keyframes.items())}
        d = {
            "type": self._type,
            "start": self._start,
            "keyframes": kf,
            "children": [c._to_dict() for c in self._children],
        }
        if self._end is not None:
            d["end"] = self._end
        return d


# ── group ──────────────────────────────────────────────────────────────────
class _Group:
    def __init__(self, nodes):
        self._nodes = list(nodes)

    def __getattr__(self, name):
        if name.startswith("_"):
            raise AttributeError(name)

        def _forward(*args, **kwargs):
            for n in self._nodes:
                fn = getattr(n, name, None)
                if fn is not None:
                    fn(*args, **kwargs)
            return self

        return _forward


# ── factory functions ──────────────────────────────────────────────────────
def _add(n):
    if _ctx is not None:
        _ctx._children.append(n)
    else:
        _roots.append(n)
    return n


def _make(ntype, at, kw):
    _all_steps.add(at)
    n = _Node(ntype, start=at)
    for k, v in kw.items():
        n._set(k, v)
    return _add(n)


def node(at=0, **kw):
    return _make("node", at, kw)


def rect(at=0, **kw):
    return _make("rect", at, kw)


def circle(at=0, **kw):
    return _make("circle", at, kw)


def group(nodes):
    return _Group(nodes)


# ── serialise on exit ──────────────────────────────────────────────────────
def _write_tree():
    path = _os.environ.get("ALSIE_TREE_PATH")
    if path:
        data = {
            "steps": max(_all_steps) + 1,
            "nodes": [r._to_dict() for r in _roots],
        }
        with open(path, "w") as _f:
            _json.dump(data, _f)


_atexit.register(_write_tree)

# ── end of preamble ────────────────────────────────────────────────────────
