import socket
import subprocess
import time

import pytest
from pathlib import Path

ROOT = Path(__file__).parent.parent
SERVER_BINARY = ROOT / "target" / "debug" / "server"
SERVER_STARTUP_TIMEOUT = 10  # seconds


@pytest.fixture(scope="session")
def server_port():
    """Build (if needed) and start the Alsie server on a free port.
    Killed automatically after the test session."""
    # Pick a free port by binding briefly and releasing it
    with socket.socket() as s:
        s.bind(("", 0))
        port = s.getsockname()[1]

    subprocess.run(
        ["cargo", "build"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )

    proc = subprocess.Popen(
        [SERVER_BINARY, "--port", str(port)],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )

    # Wait until the server prints its ready line
    deadline = time.monotonic() + SERVER_STARTUP_TIMEOUT
    ready = False
    while time.monotonic() < deadline:
        line = proc.stdout.readline()
        if not line:
            break
        if "localhost" in line:
            ready = True
            break

    if not ready:
        proc.kill()
        raise RuntimeError("Server did not become ready in time")

    yield port

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()


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


# ── interpolation helper (mirrors resolvePropsAnimated in TreeView.jsx) ──────


def _resolve_props(node_dict, step):
    """Accumulate keyframe values up to and including `step`."""
    out = {}
    for t in range(step + 1):
        kf = node_dict["keyframes"].get(str(t), {})
        for key, entry in kf.items():
            out[key] = (
                entry["value"]
                if isinstance(entry, dict) and "value" in entry
                else entry
            )
    return out


def _lerp_color(a, b, t):
    """Linear interpolation between two '#rrggbb' hex colours."""

    def ch(s, i):
        return int(s[i : i + 2], 16)

    r = round(ch(a, 1) + (ch(b, 1) - ch(a, 1)) * t)
    g = round(ch(a, 3) + (ch(b, 3) - ch(a, 3)) * t)
    bl = round(ch(a, 5) + (ch(b, 5) - ch(a, 5)) * t)
    return f"#{r:02x}{g:02x}{bl:02x}"


def resolve_animated(node_dict, step, frame, frames_per_step):
    """Return interpolated props for (step, frame).

    Mirrors the JS resolvePropsAnimated function: for smooth-transition props
    in the *next* keyframe, linearly interpolate between the current value and
    the target value using t = frame / frames_per_step.
    """
    cur = _resolve_props(node_dict, step)
    if frame == 0:
        return cur

    next_kf = node_dict["keyframes"].get(str(step + 1), {})
    if not next_kf:
        return cur

    t = frame / frames_per_step
    out = dict(cur)
    for key, entry in next_kf.items():
        if not (isinstance(entry, dict) and entry.get("transition") == "smooth"):
            continue
        from_val = cur.get(key)
        to_val = entry.get("value")
        if from_val is None or to_val is None:
            continue
        if isinstance(from_val, (int, float)) and isinstance(to_val, (int, float)):
            out[key] = from_val + (to_val - from_val) * t
        elif (
            isinstance(from_val, str)
            and isinstance(to_val, str)
            and from_val.startswith("#")
            and to_val.startswith("#")
        ):
            out[key] = _lerp_color(from_val, to_val, t)
    return out
