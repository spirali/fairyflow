"""
Zensical plugin for ``ffpy`` custom fenced code blocks.

Image mode (single frame):
    ```ffpy frame=3
    with Scene():
        Rect().size(20, 20).fill("green")
    ```
    Renders frame N as PNG, embeds as base64 data-URI <img>.

Multi-frame mode (several frames, code written once):
    ```ffpy frames="0,1,2"
    with Scene():
        Rect().size(20, 20).fill("green")
        next_frame()
        Rect().size(20, 20).fill("red")
    ```
    Compiles the source once, renders each listed frame as a PNG, and
    displays them side-by-side with "Frame N" labels.

Video mode:
    ```ffpy video="mp4"
    with Scene():
        Rect().size(20, 20).fill("green")
    ```
    Renders all frames, runs ffmpeg, saves to docs/assets/ffpy/<hash>.mp4,
    embeds as <video>.

Standard pymdownx flags like hl_lines="2 3" are passed through to the
highlighter via fence_code_format.
"""

from __future__ import annotations

import base64
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from pymdownx.superfences import RE_HL_LINES, RE_LINENUMS


def _project_root() -> Path:
    """Walk up from this file to the directory containing Cargo.toml."""
    for parent in [Path(__file__).resolve(), *Path(__file__).resolve().parents]:
        if (parent / "Cargo.toml").exists():
            return parent
    return Path.cwd()


def _server_binary() -> Path | None:
    root = _project_root()
    # In development (Cargo.toml present), prefer the locally-built binary so
    # that Rust changes are picked up immediately instead of using a stale
    # bundled binary from a previous release.
    if (root / "Cargo.toml").exists():
        for candidate in [
            root / "target" / "release" / "server",
            root / "target" / "debug" / "server",
        ]:
            if candidate.exists():
                return candidate
    # Otherwise use the binary bundled inside the installed fairyflow package.
    try:
        import fairyflow

        pkg_bin = Path(fairyflow.__file__).parent / "_bin"
        for name in ("server", "server.exe"):
            candidate = pkg_bin / name
            if candidate.exists():
                return candidate
    except ImportError:
        pass
    # Last resort: local Cargo artifacts (cwd-rooted project without Cargo.toml
    # found via __file__ walk).
    for candidate in [
        root / "target" / "release" / "server",
        root / "target" / "debug" / "server",
    ]:
        if candidate.exists():
            return candidate
    return None


def _binary_cache_key() -> str:
    """Return a string that changes whenever the server binary is rebuilt.

    Uses mtime + size rather than a full file hash — cheap to call on every
    cache lookup while still catching any rebuild during a live docs server.
    Returns an empty string when no binary is found.
    """
    binary = _server_binary()
    if binary is None:
        return ""
    try:
        st = binary.stat()
        return f"{st.st_size}:{st.st_mtime_ns}"
    except OSError:
        return ""


def _content_hash(source: str, *extra: str) -> str:
    h = hashlib.sha256(source.encode())
    for part in extra:
        h.update(part.encode())
    return h.hexdigest()[:16]


def _cache_dir() -> Path:
    d = _project_root() / ".ffpy_cache"
    d.mkdir(parents=True, exist_ok=True)
    return d


def _video_asset_dir() -> Path:
    d = _project_root() / "docs" / "assets" / "ffpy"
    d.mkdir(parents=True, exist_ok=True)
    return d


def _error_html(highlighted: str, message: str) -> str:
    escaped = message.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    return (
        f"{highlighted}"
        f'<div class="ffpy-error">'
        f"<strong>ffpy render error:</strong><pre>{escaped}</pre>"
        f"</div>"
    )


def _run_fairyflow(source: str, tmp: Path) -> Path:
    """Write source + prologue, run python3 -m fairyflow, return json path."""
    prologue = tmp / "prologue.py"
    prologue.write_text("from fairyflow import *\n")
    src_file = tmp / "scene.py"
    src_file.write_text(source)
    json_file = tmp / "anim.json"

    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "fairyflow",
            "--prologue",
            str(prologue),
            str(src_file),
            str(json_file),
            "24",
        ],
        capture_output=True,
        text=True,
        cwd=str(_project_root()),
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"fairyflow failed:\n{result.stderr.strip()}")
    if not json_file.exists():
        raise RuntimeError("fairyflow produced no output JSON")
    return json_file


def _render_frame_png(json_file: Path, frames_dir: Path, frame: int) -> Path:
    server = _server_binary()
    if server is None:
        raise RuntimeError("server binary not found; run `cargo build` first")
    result = subprocess.run(
        [
            str(server),
            "render-png",
            str(json_file),
            str(frames_dir),
            "--frames",
            str(frame),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"render-png failed:\n{result.stderr.strip()}")
    png = frames_dir / f"frame{frame}.png"
    if not png.exists():
        raise RuntimeError(f"Expected frame PNG not written: {png}")
    return png


def _render_mp4(
    json_file: Path, output_mp4: Path, fps: int = 24, codec: str = "h264", crf: int = 23
) -> None:
    server = _server_binary()
    if server is None:
        raise RuntimeError("server binary not found; run `cargo build` first")
    result = subprocess.run(
        [
            str(server),
            "render-video",
            str(json_file),
            str(output_mp4),
            "--fps",
            str(fps),
            "--codec",
            codec,
            "--crf",
            str(crf),
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(f"render-video failed:\n{result.stderr.strip()}")


# ---------------------------------------------------------------------------
# Validator — registered in zensical.toml alongside the format handler
# ---------------------------------------------------------------------------


def ffpy_validator(language: str, inputs: dict, options: dict, attrs: dict, md) -> bool:
    """
    Validator for ``ffpy`` fences.

    Puts ``frame``, ``video``, and standard highlight options (``hl_lines``,
    ``linenums``, ``title``) into *options*, silently drops anything else.
    Keeping *attrs* empty is critical: superfences rejects the match if attrs
    is non-empty when using the default_validator, so we must not put any
    unrecognised keys there.
    """
    for k, v in inputs.items():
        if (
            k in ("frame", "frames", "video", "title", "position")
            or k == "hl_lines"
            and RE_HL_LINES.match(str(v))
            or k == "linenums"
            and RE_LINENUMS.match(str(v))
        ):
            options[k] = v
        # Unknown options are silently dropped so attrs stays empty.
    return True


# ---------------------------------------------------------------------------
# Public fence handler — registered in zensical.toml
# ---------------------------------------------------------------------------


def ffpy_fence(
    source: str, language: str, class_name: str, options: dict, md, **kwargs
) -> str:
    """pymdownx.superfences custom format handler for ``ffpy`` blocks."""
    # Pop our custom options; leave hl_lines etc. for the highlighter
    frame_opt = options.pop("frame", None)
    frames_opt = options.pop("frames", None)
    video_opt = options.pop("video", None)
    position = options.pop("position", "bottom")

    # Syntax-highlight as Python using the real pymdownx highlight pipeline.
    # fence_code_format only wraps in <pre><code> without Pygments highlighting,
    # so we call the preprocessor's highlight() method directly instead.
    highlighted = md.preprocessors["fenced_code_block"].highlight(
        src=source,
        language="python",
        options=options,
        md=md,
        **kwargs,
    )

    if video_opt is not None:
        return _do_video(source, highlighted, position)
    elif frames_opt is not None:
        try:
            frame_list = [int(x.strip()) for x in frames_opt.split(",")]
        except ValueError:
            return _error_html(highlighted, f"Invalid frames= value: {frames_opt!r}")
        return _do_frames(source, highlighted, frame_list, position)
    elif frame_opt is not None:
        try:
            frame_num = int(frame_opt)
        except ValueError:
            return _error_html(highlighted, f"Invalid frame= value: {frame_opt!r}")
        return _do_frame(source, highlighted, frame_num, position)
    return highlighted  # no render mode — just show highlighted code


def _do_frame(
    source: str, highlighted: str, frame: int, position: str = "bottom"
) -> str:
    cached_png = (
        _cache_dir() / f"{_content_hash(source, str(frame), _binary_cache_key())}.png"
    )
    if not cached_png.exists():
        try:
            with tempfile.TemporaryDirectory(prefix="ffpy-") as tmp:
                json_file = _run_fairyflow(source, Path(tmp))
                frames_dir = Path(tmp) / "frames"
                frames_dir.mkdir()
                png = _render_frame_png(json_file, frames_dir, frame)
                shutil.copy2(png, cached_png)
        except Exception as exc:  # noqa: BLE001 -- doc-build boundary: one bad ffpy block must not abort the whole docs build
            return _error_html(highlighted, str(exc))

    b64 = base64.b64encode(cached_png.read_bytes()).decode("ascii")
    label = (
        '<p style="margin:.5em 0 .2em;font-style:italic;color:#666">Output image</p>'
    )
    img = (
        f'<img src="data:image/png;base64,{b64}" '
        f'alt="fairyflow frame {frame}" '
        f'style="max-width:100%;display:block;margin:0 0 .5em;border:1px solid black" />'
    )
    output = f"{label}\n{img}"
    if position == "top":
        return f"{output}\n{highlighted}"
    return f"{highlighted}\n{output}"


def _do_frames(
    source: str, highlighted: str, frames: list[int], position: str = "bottom"
) -> str:
    """Render multiple frames from the same source and display them side by side."""
    cache = _cache_dir()
    cached: dict[int, Path] = {}
    uncached: list[int] = []
    for f in frames:
        path = cache / f"{_content_hash(source, str(f), _binary_cache_key())}.png"
        if path.exists():
            cached[f] = path
        else:
            uncached.append(f)

    if uncached:
        try:
            server = _server_binary()
            if server is None:
                raise RuntimeError("server binary not found; run `cargo build` first")
            with tempfile.TemporaryDirectory(prefix="ffpy-") as tmp:
                json_file = _run_fairyflow(source, Path(tmp))
                frames_dir = Path(tmp) / "frames"
                frames_dir.mkdir()
                result = subprocess.run(
                    [
                        str(server),
                        "render-png",
                        str(json_file),
                        str(frames_dir),
                        f"--frames={','.join(str(f) for f in uncached)}",
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if result.returncode != 0:
                    raise RuntimeError(f"render-png failed:\n{result.stderr.strip()}")
                for f in uncached:
                    png = frames_dir / f"frame{f}.png"
                    if not png.exists():
                        raise RuntimeError(f"Expected frame PNG not written: {png}")
                    dest = (
                        cache
                        / f"{_content_hash(source, str(f), _binary_cache_key())}.png"
                    )
                    shutil.copy2(png, dest)
                    cached[f] = dest
        except Exception as exc:  # noqa: BLE001 -- doc-build boundary: one bad ffpy block must not abort the whole docs build
            return _error_html(highlighted, str(exc))

    items = []
    for f in frames:
        b64 = base64.b64encode(cached[f].read_bytes()).decode("ascii")
        items.append(
            f'<div style="display:inline-block;margin-right:1em;vertical-align:top">'
            f'<p style="margin:.5em 0 .2em;font-style:italic;color:#666">Frame {f}</p>'
            f'<img src="data:image/png;base64,{b64}" '
            f'alt="fairyflow frame {f}" '
            f'style="display:block;border:1px solid black" />'
            f"</div>"
        )
    output = f'<div style="margin:.5em 0">{"".join(items)}</div>'
    if position == "top":
        return f"{output}\n{highlighted}"
    return f"{highlighted}\n{output}"


def _do_video(source: str, highlighted: str, position: str = "bottom") -> str:
    key = _content_hash(source, _binary_cache_key())
    cached_mp4 = _video_asset_dir() / f"{key}.mp4"
    if not cached_mp4.exists():
        try:
            with tempfile.TemporaryDirectory(prefix="ffpy-video-") as tmp:
                json_file = _run_fairyflow(source, Path(tmp))
                _render_mp4(json_file, cached_mp4)
        except Exception as exc:  # noqa: BLE001 -- doc-build boundary: one bad ffpy block must not abort the whole docs build
            return _error_html(highlighted, str(exc))

    # Zensical copies docs/ → site/ before processing markdown, so MP4s
    # rendered here must be pushed into site/ explicitly.
    site_dir = _project_root() / "site" / "assets" / "ffpy"
    site_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(cached_mp4, site_dir / f"{key}.mp4")

    # Build the asset URL. FAIRYFLOW_DOCS_BASE_URL lets CI set the path prefix
    # for deployments served under a subdirectory (e.g. /fairyflow on GitHub
    # Pages). Defaults to empty string for local / root-hosted builds.
    base = os.environ.get("FAIRYFLOW_DOCS_BASE_URL", "").rstrip("/")
    rel = cached_mp4.relative_to(_project_root() / "docs")
    url = base + "/" + rel.as_posix()
    label = (
        '<p style="margin:.5em 0 .2em;font-style:italic;color:#666">Output video</p>'
    )
    video = (
        f'<video controls style="max-width:100%;display:block;margin:0 0 .5em;border:1px solid black">'
        f'<source src="{url}" type="video/mp4">'
        f"Your browser does not support the video tag."
        f"</video>"
    )
    output = f"{label}\n{video}"
    if position == "top":
        return f"{output}\n{highlighted}"
    return f"{highlighted}\n{output}"
