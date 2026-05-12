import inspect
import os

_debug_mode = False


def set_debug_mode(debug: bool):
    global _debug_mode
    _debug_mode = debug


def get_current_stack():
    stack = inspect.stack()
    return [
        {"file": frame.filename, "line": frame.lineno}
        for frame in stack
        if not os.path.isabs(frame.filename) and not frame.filename.startswith("<")
    ]


def get_info(node_id):
    info: dict = {"id": node_id}
    if _debug_mode:
        info["stack"] = get_current_stack()
    return info
