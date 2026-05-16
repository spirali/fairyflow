from typing import SupportsFloat
from . import config


def time_to_frames(time: SupportsFloat) -> int:
    """
    Convert time in seconds to a frame number.
    """
    return int(round(config.FPS * time))


def frames_to_time(frames: int) -> float:
    """
    Convert a frame number to time in seconds.
    """
    return frames / config.FPS


class Frames:
    def __init__(self, frames):
        self.frames = frames


type Transition = SupportsFloat | Frames | None


def tr_to_frames(tr: Transition):
    if tr is None:
        return 0
    if isinstance(tr, Frames):
        return tr.frames
    else:
        return time_to_frames(tr)
