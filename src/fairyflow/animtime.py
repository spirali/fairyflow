from typing import Literal, SupportsFloat
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


type Duration = SupportsFloat | Frames | None

# The 5 standard CSS-equivalent cubic-bezier presets (api-v2-proposal.md §3.2).
# Sampled/custom-callable easing (`ease=lambda t: ...`) is deliberately not
# supported yet — deferred alongside the rest of proposal §9.
type Easing = Literal["linear", "in", "out", "in_out", "out_back"] | None


def duration_to_frames(dur: Duration):
    if dur is None:
        return 0
    if isinstance(dur, Frames):
        return dur.frames
    else:
        return time_to_frames(dur)
