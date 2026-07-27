from typing import Literal, SupportsFloat

from . import config


def time_to_frames(time: SupportsFloat) -> int:
    """
    Convert time in seconds to a frame number.
    """
    return round(config.FPS * time)


def frames_to_time(frames: int) -> float:
    """
    Convert a frame number to time in seconds.
    """
    return frames / config.FPS


class Frames:
    def __init__(self, frames):
        self.frames = frames


type Duration = SupportsFloat | Frames | None

type Easing = Literal["linear", "in", "out", "in_out", "out_back", "step"] | None


def duration_to_frames(dur: Duration):
    if dur is None:
        return 0
    if isinstance(dur, Frames):
        return dur.frames
    else:
        return time_to_frames(dur)
