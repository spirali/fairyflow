import pytest
from pathlib import Path

PREAMBLE = (Path(__file__).parent.parent / "python" / "preamble.py").read_text()


@pytest.fixture
def scene():
    """Fresh DSL namespace for each test. Exposes all public API names and
    a helper `tree()` that returns the serialised scene dict."""
    ns = {}
    exec(PREAMBLE, ns)

    def tree():
        nodes = [r._to_dict() for r in ns["_roots"]]
        steps = max(ns["_all_steps"]) + 1
        return {"steps": steps, "nodes": nodes}

    ns["tree"] = tree
    return ns
