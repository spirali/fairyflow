import inspect
import os


def get_current_stack():
    stack = inspect.stack()
    return [
        {"file": frame.filename, "line": frame.lineno}
        for frame in stack
        if not os.path.isabs(frame.filename) and not frame.filename.startswith("<")
    ]


def get_info(node_id):
    return {"id": node_id, "stack": get_current_stack()}
