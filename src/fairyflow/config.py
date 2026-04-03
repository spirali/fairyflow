from .color import Color

DEFAULT_SCENE_CONFIG = {
    "width": 300,
    "height": 200,
    "color": Color("white"),
    "cue_at_end": True,
}

FPS = 24

def set_default_scene(*, width=int | None, height=int | None, color=str | None, cue_at_end: bool | None = None):
    if width is not None:
        DEFAULT_SCENE_CONFIG["width"] = width
    if height is not None:
        DEFAULT_SCENE_CONFIG["height"] = height
    if color is not None:
        DEFAULT_SCENE_CONFIG["color"] = Color.parse(color)
    if cue_at_end is not None:
        DEFAULT_SCENE_CONFIG["cue_at_end"] = cue_at_end


def set_fps(fps):
    global FPS
    FPS = fps