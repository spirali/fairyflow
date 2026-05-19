import json
import shutil
import socket
import subprocess
import time
import os
import pymupdf
from fairyflow import Scene
from fairyflow.serializer import create_export
import numpy as np
from PIL import Image
import re as _re

import pytest
from pathlib import Path

ROOT = Path(__file__).parent.parent
SERVER_BINARY = ROOT / "target" / "debug" / "server"
SERVER_STARTUP_TIMEOUT = 10  # seconds
CURRENT_DIR = ROOT / "tests" / "current"
CURRENT_PDF_DIR = ROOT / "tests" / "current_pdf"
CHECK_DIR = ROOT / "tests" / "check"
FONTS_DIR = ROOT / "tests" / "assets" / "fonts"

FAIRYFLOW_TEST_CREATE = int(os.environ.get("FAIRYFLOW_TEST_CREATE", False))
FAIRYFLOW_TEST_UPDATE = int(os.environ.get("FAIRYFLOW_TEST_UPDATE", False))

DIFF_TOLERANCE = 3
PDF_DIFF_TOLERANCE = 30

_TESTS_DIR_RE = _re.compile(r".*/tests/")


def image_diff(
    current_arr: np.ndarray, check_arr: np.ndarray, label: str, tolerance: float
) -> str | None:
    """Return an error string if the two uint8 arrays differ beyond DIFF_TOLERANCE, else None.

    Arrays may have 3 or 4 channels. The score is the sum of per-pixel
    max-channel absolute differences (0-255 scale).
    """
    if current_arr.shape != check_arr.shape:
        return (
            f"{label}: size {current_arr.shape[1]}x{current_arr.shape[0]} "
            f"!= reference {check_arr.shape[1]}x{check_arr.shape[0]}"
        )
    diff_sum = (
        np.abs(current_arr.astype(np.int32) - check_arr.astype(np.int32))
        .max(axis=-1)
        .sum()
        / 255.0
    )
    if diff_sum > tolerance:
        return f"{label}: diff sum {diff_sum} exceeds tolerance of {tolerance}"
    return None


def normalize_tree(obj):
    """Replace the absolute tests-dir prefix in path strings with $TEST_DIR."""
    if isinstance(obj, dict):
        return {k: normalize_tree(v) for k, v in obj.items()}
    if isinstance(obj, list):
        return [normalize_tree(v) for v in obj]
    if isinstance(obj, str) and "/tests/" in obj:
        return _TESTS_DIR_RE.sub("$TEST_DIR/", obj, count=1)
    return obj


def pytest_sessionstart(session):
    if CURRENT_DIR.is_dir():
        for entry in CURRENT_DIR.iterdir():
            if entry.is_dir() and entry.name.startswith("test_"):
                shutil.rmtree(entry)
    if CURRENT_PDF_DIR.is_dir():
        for entry in CURRENT_PDF_DIR.iterdir():
            if entry.is_dir() and entry.name.startswith("test_"):
                shutil.rmtree(entry)


@pytest.fixture(scope="session")
def _server(tmp_path_factory):
    """Build (if needed), initialise a project, and start the FairyFlow server.
    Yields (port, token, project_dir). Killed automatically after the session."""
    with socket.socket() as s:
        s.bind(("", 0))
        port = s.getsockname()[1]

    subprocess.run(
        ["cargo", "build"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )

    # The server resolves "../web/dist" relative to the project directory.
    # Place the project as a child of basetemp and symlink web/ there so that
    # ../web/dist from inside the project resolves to ROOT/web/dist.
    base = tmp_path_factory.getbasetemp()
    web_link = base / "web"
    if not web_link.exists():
        web_link.symlink_to(ROOT / "web")
    proj = base / "server_project"
    proj.mkdir(exist_ok=True)

    subprocess.run(
        [str(SERVER_BINARY), "init", str(proj)],
        cwd=ROOT,
        check=True,
        capture_output=True,
    )

    # Load DejaVu fonts and set aliases so server tests are independent of system fonts.
    with (proj / "fairyflow.toml").open("a") as f:
        f.write(f'\nfont_directories = ["{FONTS_DIR}"]\n')
        f.write("\n[font-aliases]\n")
        f.write('"sans-serif" = "DejaVu Sans"\n')
        f.write('"monospace" = "DejaVu Sans Mono"\n')

    # Extra scene files used by test_server.py
    (proj / "scenes" / "slow.ffpy").write_text("import time; time.sleep(30)\n")
    (proj / "scenes" / "error.ffpy").write_text(
        "raise ValueError('intentional error')\n"
    )

    token = "test-token-abc123"

    proc = subprocess.Popen(
        [str(SERVER_BINARY), "open", "--port", str(port), "--token", token, str(proj)],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )

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

    yield port, token, proj

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()


@pytest.fixture(scope="session")
def server_port(_server):
    return _server[0]


@pytest.fixture(scope="session")
def server_token(_server):
    return _server[1]


@pytest.fixture(scope="session")
def server_project_dir(_server):
    return _server[2]


@pytest.fixture(scope="session")
def server_uri(_server):
    """Authenticated WebSocket URI for the running test server."""
    port, token, _ = _server
    return f"ws://localhost:{port}/ws?token={token}"


@pytest.fixture(scope="session")
def server_base_url(_server):
    """Base HTTP URL (no trailing slash) for the running test server."""
    port, _, _ = _server
    return f"http://localhost:{port}"


@pytest.fixture(scope="function")
def test_scene(request):
    s = Scene(60, 40)
    s.select_frames = None
    s.target_resolution = None
    s.tolerance = DIFF_TOLERANCE
    s.pdf_tolerance = PDF_DIFF_TOLERANCE
    try:
        yield s
    except BaseException:
        raise
    exported = create_export(0, s)

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
        "--font-dir",
        str(FONTS_DIR),
        "--font-alias",
        "sans-serif=DejaVu Sans",
        "--font-alias",
        "monospace=DejaVu Sans Mono",
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
        "--font-dir",
        str(FONTS_DIR),
        "--font-alias",
        "sans-serif=DejaVu Sans",
        "--font-alias",
        "monospace=DejaVu Sans Mono",
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
                subprocess.run(
                    ["oxipng", "--nc", "--"] + [str(p) for p in pngs], check=True
                )
            return
        else:
            raise Exception(
                f"Check directory '{check_frames_dir}' not found; run with FAIRYFLOW_TEST_CREATE=1 to create the snapshot"
            )

    def frame_key(p: Path) -> int:
        m = _re.search(r"\d+", p.stem)
        return int(m.group()) if m else 0

    current_frames = sorted(frames_dir.glob("*.png"), key=frame_key)
    check_frames = sorted(check_frames_dir.glob("*.png"), key=frame_key)

    current_names = [f.name for f in current_frames]
    check_names = [f.name for f in check_frames]
    if current_names != check_names:
        pytest.fail(
            f"frame mismatch:\n  current: {current_names}\n  check:   {check_names}"
        )

    for current_png, check_png in zip(current_frames, check_frames):
        current_img = Image.open(current_png)
        check_img = Image.open(check_png)
        if err := image_diff(
            np.asarray(current_img.convert("RGBA")),
            np.asarray(check_img.convert("RGBA")),
            current_png.name,
            s.tolerance,
        ):
            pytest.fail(err)

        stem = current_png.stem  # e.g. "frame0"
        current_json = frames_dir / f"{stem}.json"
        check_json = check_frames_dir / f"{stem}.json"
        if not check_json.exists():
            pytest.fail(f"{stem}.json missing from check directory")
        current_json = normalize_tree(json.loads(current_json.read_text()))
        check_json = normalize_tree(json.loads(check_json.read_text()))
        assert current_json == check_json

    # PDF rendering check: convert each PDF page to an image and compare with reference PNGs.
    # PageSettings uses scene dimensions as PDF points; rendering at 72 DPI (Matrix(1,1))
    # gives exactly scene.width × scene.height pixels, matching the PNG output.
    if pdf_path.exists():
        pdf_doc = pymupdf.open(str(pdf_path))
        if pdf_doc.page_count != len(check_frames):
            pytest.fail(
                f"PDF has {pdf_doc.page_count} page(s) but {len(check_frames)} reference frame(s)"
            )
        pdf_pages = []
        for page in pdf_doc.pages():
            if s.target_resolution is not None:
                W, H = s.target_resolution
                scale = min(W / page.rect.width, H / page.rect.height)
                pix = page.get_pixmap(
                    matrix=pymupdf.Matrix(scale, scale), colorspace=pymupdf.csRGB
                )
                canvas = np.zeros((H, W, 3), dtype=np.uint8)
                x_off = (W - pix.width) // 2
                y_off = (H - pix.height) // 2
                canvas[y_off : y_off + pix.height, x_off : x_off + pix.width] = (
                    np.frombuffer(pix.samples, dtype=np.uint8).reshape(
                        pix.height, pix.width, 3
                    )
                )
                pdf_pages.append(canvas)
            else:
                pix = page.get_pixmap(
                    matrix=pymupdf.Matrix(1, 1), colorspace=pymupdf.csRGB
                )
                pdf_pages.append(
                    np.frombuffer(pix.samples, dtype=np.uint8)
                    .reshape(pix.height, pix.width, 3)
                    .copy()
                )
        pdf_doc.close()

        failure = None
        for page_idx, (pdf_arr, check_png) in enumerate(zip(pdf_pages, check_frames)):
            check_img = Image.open(check_png)
            check_arr = np.asarray(check_img.convert("RGB"))
            failure = image_diff(
                pdf_arr,
                check_arr,
                f"PDF page {page_idx} ({check_png.name})",
                s.pdf_tolerance,
            )
            if failure:
                break

        if failure is not None:
            dump_dir = CURRENT_PDF_DIR / request.node.name / "frames"
            dump_dir.mkdir(parents=True, exist_ok=True)
            for pdf_arr, check_png in zip(pdf_pages, check_frames):
                Image.fromarray(pdf_arr, mode="RGB").save(dump_dir / check_png.name)
            pytest.fail(failure)
