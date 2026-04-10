import json
import shutil
import socket
import subprocess
import time
import sys
import os
import fitz
from fairyflow import scene
from fairyflow.serializer import create_export
import numpy as np
from PIL import Image

import pytest
from pathlib import Path

ROOT = Path(__file__).parent.parent
SERVER_BINARY = ROOT / "target" / "debug" / "server"
SERVER_STARTUP_TIMEOUT = 10  # seconds
CURRENT_DIR = ROOT / "tests" / "current"
CHECK_DIR = ROOT / "tests" / "check"

FAIRYFLOW_TEST_CREATE = int(os.environ.get("FAIRYFLOW_TEST_CREATE", False))
FAIRYFLOW_TEST_UPDATE = int(os.environ.get("FAIRYFLOW_TEST_UPDATE", False))


def pytest_sessionstart(session):
    if CURRENT_DIR.is_dir():
        for entry in CURRENT_DIR.iterdir():
            if entry.is_dir() and entry.name.startswith("test_"):
                shutil.rmtree(entry)


@pytest.fixture(scope="session")
def server_port():
    """Build (if needed) and start the FairyFlow server on a free port.
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
    def_path.write_text(json.dumps(exported, indent=2))

    pdf_path = out_dir / "animation.pdf"

    png_cmd = [
        str(SERVER_BINARY),
        "render-png",
        str(def_path),
        str(frames_dir),
        "--write-tree",
    ]
    if s.select_frames is not None:
        png_cmd.append("--frames=" + ",".join(str(f) for f in s.select_frames))
    if s.target_resolution is not None:
        w, h = s.target_resolution
        png_cmd.append(f"--target-resolution={w}x{h}")
    subprocess.run(png_cmd, check=True, cwd=ROOT)

    pdf_cmd = [
        str(SERVER_BINARY),
        "render-pdf",
        str(def_path),
        str(pdf_path),
    ]
    if s.select_frames is not None:
        pdf_cmd.append("--frames=" + ",".join(str(f) for f in s.select_frames))
    subprocess.run(pdf_cmd, check=True, cwd=ROOT)

    check_frames_dir = CHECK_DIR / request.node.name / "frames"
    if FAIRYFLOW_TEST_UPDATE and check_frames_dir.is_dir():
        shutil.rmtree(check_frames_dir)
    if not check_frames_dir.is_dir():
        if FAIRYFLOW_TEST_CREATE or FAIRYFLOW_TEST_UPDATE:
            shutil.copytree(frames_dir, check_frames_dir)
            pngs = list(check_frames_dir.glob("*.png"))
            if pngs:
                subprocess.run(["oxipng", "--"] + [str(p) for p in pngs], check=True)
            return
        else:
            raise Exception(
                f"Check directory '{check_frames_dir}' not found; run with FAIRYFLOW_TEST_CREATE=1 to create the snapshot"
            )

    current_frames = sorted(frames_dir.glob("*.png"))
    check_frames = sorted(check_frames_dir.glob("*.png"))

    current_names = [f.name for f in current_frames]
    check_names = [f.name for f in check_frames]
    if current_names != check_names:
        pytest.fail(
            f"frame mismatch:\n  current: {current_names}\n  check:   {check_names}"
        )

    for current_png, check_png in zip(current_frames, check_frames):
        current_img = Image.open(current_png)
        check_img = Image.open(check_png)
        if current_img.size != check_img.size:
            pytest.fail(
                f"{current_png.name}: resolution mismatch: "
                f"{current_img.size} (current) vs {check_img.size} (check)"
            )
        current_arr = np.asarray(current_img.convert("RGBA"))
        check_arr = np.asarray(check_img.convert("RGBA"))
        diff_count = int(np.any(current_arr != check_arr, axis=-1).sum())
        if diff_count:
            pytest.fail(f"{current_png.name}: {diff_count} pixel(s) differ")

        stem = current_png.stem  # e.g. "frame0"
        current_json = frames_dir / f"{stem}.json"
        check_json = check_frames_dir / f"{stem}.json"
        if not check_json.exists():
            pytest.fail(f"{stem}.json missing from check directory")
        current_json = json.loads(current_json.read_text())
        check_json = json.loads(check_json.read_text())
        assert current_json == check_json

    # PDF rendering check: convert each PDF page to an image and compare with reference PNGs.
    # PageSettings uses scene dimensions as PDF points; rendering at 72 DPI (Matrix(1,1))
    # gives exactly scene.width × scene.height pixels, matching the PNG output.
    PDF_TOLERANCE = 2  # max per-channel absolute difference
    if pdf_path.exists():
        pdf_doc = fitz.open(str(pdf_path))
        if pdf_doc.page_count != len(check_frames):
            pytest.fail(
                f"PDF has {pdf_doc.page_count} page(s) but {len(check_frames)} reference frame(s)"
            )
        for page_idx, (page, check_png) in enumerate(zip(pdf_doc.pages(), check_frames)):
            pix = page.get_pixmap(matrix=fitz.Matrix(1, 1), colorspace=fitz.csRGB)
            pdf_arr = np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height, pix.width, 3)
            check_img = Image.open(check_png)
            check_arr = np.asarray(check_img.convert("RGB"))
            if pdf_arr.shape != check_arr.shape:
                pytest.fail(
                    f"PDF page {page_idx}: size {pdf_arr.shape[1]}x{pdf_arr.shape[0]} != "
                    f"reference {check_arr.shape[1]}x{check_arr.shape[0]}"
                )
            max_diff = int(np.abs(pdf_arr.astype(np.int32) - check_arr.astype(np.int32)).max())
            if max_diff > PDF_TOLERANCE:
                pytest.fail(
                    f"PDF page {page_idx} ({check_png.name}): "
                    f"max pixel diff {max_diff} exceeds tolerance of {PDF_TOLERANCE}"
                )
        pdf_doc.close()
