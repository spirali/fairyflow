import json
import shutil
import socket
import subprocess
import time
import sys
import os
from alsie import scene
from alsie.serializer import create_export
import numpy as np
from PIL import Image

import pytest
from pathlib import Path

ROOT = Path(__file__).parent.parent
SERVER_BINARY = ROOT / "target" / "debug" / "server"
SERVER_STARTUP_TIMEOUT = 10  # seconds
CURRENT_DIR = ROOT / "tests" / "current"
CHECK_DIR = ROOT / "tests" / "check"

ALSIE_TEST_CREATE = int(os.environ.get("ALSIE_TEST_CREATE", False))

def pytest_sessionstart(session):
    if CURRENT_DIR.is_dir():
        for entry in CURRENT_DIR.iterdir():
            if entry.is_dir() and entry.name.startswith("test_"):
                shutil.rmtree(entry)


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


@pytest.fixture(scope="session")
def server_uri(server_port):
    """WebSocket URI for the running test server."""
    return f"ws://localhost:{server_port}/ws"


@pytest.fixture(scope="function")
def test_scene(request):
    s = scene(60, 40)
    s.select_frames = None
    s.target_resolution = None
    yield s
    exported = create_export(s)

    out_dir = CURRENT_DIR / request.node.name
    frames_dir = out_dir / "frames"
    frames_dir.mkdir(parents=True, exist_ok=True)

    def_path = out_dir / "def.json"
    def_path.write_text(json.dumps(exported))

    cmd = [str(SERVER_BINARY), "render-json", str(def_path), str(frames_dir)]
    if s.select_frames is not None:
        cmd.append("--frames=" + ",".join(str(f) for f in s.select_frames))
    if s.target_resolution is not None:
        w, h = s.target_resolution
        cmd.append(f"--target-resolution={w}x{h}")

    subprocess.run(cmd, check=True, cwd=ROOT)

    check_frames_dir = CHECK_DIR / request.node.name / "frames"
    if not check_frames_dir.is_dir():
        if ALSIE_TEST_CREATE:
            shutil.copytree(frames_dir, check_frames_dir)
            return
        else:
            raise Exception(f"Check directory '{check_frames_dir}' not found; run with ALSIE_TEST_CREATE=1 to create the snapshot")

    current_frames = sorted(frames_dir.glob("*.png"))
    check_frames = sorted(check_frames_dir.glob("*.png"))

    current_names = [f.name for f in current_frames]
    check_names = [f.name for f in check_frames]
    if current_names != check_names:
        pytest.fail(
            f"frame mismatch:\n"
            f"  current: {current_names}\n"
            f"  check:   {check_names}"
        )

    for current_png, check_png in zip(current_frames, check_frames):
        current_img = Image.open(current_png)
        check_img = Image.open(check_png)
        if current_img.size != check_img.size:
            pytest.fail(
                f"{current_png.name}: resolution mismatch: "
                f"{current_img.size} (current) vs {check_img.size} (check)"
            )
        current_arr = np.asarray(current_img)
        check_arr = np.asarray(check_img)
        diff_count = int(np.any(current_arr != check_arr, axis=-1).sum())
        if diff_count:
            pytest.fail(f"{current_png.name}: {diff_count} pixel(s) differ")
