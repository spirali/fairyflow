import socket
import subprocess
import time
import sys

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


@pytest.fixture(scope="session")
def server_uri(server_port):
    """WebSocket URI for the running test server."""
    return f"ws://localhost:{server_port}/ws"


