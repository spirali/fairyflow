import json as json
import os as os
from .expr import EvalCtx


def create_export(node):
    frames = set()
    frames.add(0)
    node.key_frames(frames)
    n_frames = max(frames)
    return {
        "key_frames": sorted(frames),
        "frames": [node.build(EvalCtx(frame)) for frame in range(n_frames + 1)]
    }

def write_tree(path):
    from .items import ROOT_OBJECT
    export = create_export(ROOT_OBJECT)
    print(export)
    with open(path, "w") as f:
        json.dump(export, f)

