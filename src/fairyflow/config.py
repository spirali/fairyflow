from .color import Color, Gradient

DEFAULT_SCENE_CONFIG = {
    "width": 300,
    "height": 200,
    "background": Color("white"),
    "flow": False,
}

# Starts empty (not pre-seeded with "sans-serif"/16) so "configured" and "left
# at the engine default" stay distinguishable — only keys the user actually
# sets here get materialized onto Text nodes (Text.__init__, text.py).
DEFAULT_FONT = {}

FPS = 24


def set_default_scene(
    *,
    width=int | None,
    height=int | None,
    background=str | None,
    flow: bool | None = None,
):
    if width is not None:
        DEFAULT_SCENE_CONFIG["width"] = width
    if height is not None:
        DEFAULT_SCENE_CONFIG["height"] = height
    if background is not None:
        DEFAULT_SCENE_CONFIG["background"] = Color.parse(background)
    if flow is not None:
        DEFAULT_SCENE_CONFIG["flow"] = flow


def normalize_font_style(family, weight, *, bold, mono, caller="font"):
    """Shared `bold=`/`mono=` sugar + mutual-exclusion checks for `font()`
    (text.py) and `set_default_font()` below, so the two TypeErrors stay
    defined in exactly one place."""
    if bold is not None:
        if weight is not None:
            raise TypeError(f"{caller}(): pass either weight= or bold=, not both")
        weight = 800 if bold else 400
    if mono is not None:
        if family is not None:
            raise TypeError(
                f"{caller}(): pass either family (positional) or mono=, not both"
            )
        family = "monospace" if mono else "sans-serif"
    return family, weight


def set_default_font(
    family: str | None = None,
    size: float | None = None,
    *,
    weight: float | None = None,
    italic: bool | None = None,
    bold: bool | None = None,
    mono: bool | None = None,
    fill=None,
):
    family, weight = normalize_font_style(
        family, weight, bold=bold, mono=mono, caller="set_default_font"
    )
    if family is not None:
        DEFAULT_FONT["font"] = family
    if size is not None:
        DEFAULT_FONT["font_size"] = size
    if weight is not None:
        DEFAULT_FONT["font_weight"] = weight
    if italic is not None:
        DEFAULT_FONT["italic"] = italic
    if fill is not None:
        # Normalized here (not at Text.__init__ use time), matching how
        # set_default_scene(background=) normalizes eagerly (line 30 above) —
        # a bad color fails in the prologue, not at the first Text().
        DEFAULT_FONT["fill"] = fill if isinstance(fill, Gradient) else Color.parse(fill)


DEFAULT_CODE = {"family": "monospace"}


def set_default_code(
    language: str | None = None,
    *,
    theme: str | None = None,
    family: str | None = None,
    size: float | None = None,
):
    if language is not None:
        DEFAULT_CODE["language"] = language
    if theme is not None:
        DEFAULT_CODE["theme"] = theme
    if family is not None:
        DEFAULT_CODE["family"] = family
    if size is not None:
        DEFAULT_CODE["size"] = size


def set_fps(fps):
    global FPS
    FPS = fps
