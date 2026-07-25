from .color import Color

DEFAULT_SCENE_CONFIG = {
    "width": 300,
    "height": 200,
    "background": Color("white"),
    "flow": False,
}

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


def set_fps(fps):
    global FPS
    FPS = fps
