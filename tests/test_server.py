"""Integration tests: start the real server and communicate over WebSocket and HTTP."""

import asyncio
import json
import urllib.error
import urllib.request

import pytest
import websockets

WS_TIMEOUT = 15  # seconds
HTTP_TIMEOUT = 15  # seconds

# Hard per-test wall-clock cap so a stuck `await`/blocking call (server never
# responds, WebSocket never closes, ...) fails fast with a clear traceback
# instead of hanging the whole suite indefinitely. Generous relative to
# WS_TIMEOUT/FILE_CHANGED_TIMEOUT so it never fires under normal conditions —
# it's a backstop, not a tight budget.
pytestmark = pytest.mark.timeout(60)


async def _recv_json(ws) -> dict:
    """`ws.recv()` with a timeout — a bare `await ws.recv()` blocks forever
    if the server never sends anything, e.g. to consume the initial `config`
    message."""
    async with asyncio.timeout(WS_TIMEOUT):
        return json.loads(await ws.recv())


async def _collect_until_done(ws) -> list[dict]:
    """Collect messages from an open WebSocket until a 'done' message arrives."""
    messages = []
    async with asyncio.timeout(WS_TIMEOUT):
        async for raw in ws:
            msg = json.loads(raw)
            messages.append(msg)
            if msg["type"] == "done":
                break
    return messages


async def _run_scene(ws, path: str) -> list[dict]:
    """Send a run request for *path* on an existing WebSocket and collect messages."""
    await ws.send(json.dumps({"type": "run", "path": path}))
    return await _collect_until_done(ws)


# ── helpers ───────────────────────────────────────────────────────────────────


def by_type(messages: list[dict], kind: str) -> list[dict]:
    return [m for m in messages if m["type"] == kind]


def http_get(base_url: str, endpoint: str, token: str, **params) -> tuple[int, bytes]:
    qs = "&".join(f"{k}={v}" for k, v in {**params, "token": token}.items())
    url = f"{base_url}{endpoint}?{qs}"
    try:
        with urllib.request.urlopen(url, timeout=HTTP_TIMEOUT) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def http_put(base_url: str, endpoint: str, token: str, body: str, **params) -> int:
    qs = "&".join(f"{k}={v}" for k, v in {**params, "token": token}.items())
    url = f"{base_url}{endpoint}?{qs}"
    req = urllib.request.Request(url, data=body.encode(), method="PUT")
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT) as r:
            return r.status
    except urllib.error.HTTPError as e:
        return e.code


# ── connectivity ──────────────────────────────────────────────────────────────


async def test_server_accepts_connection(server_uri):
    async with asyncio.timeout(WS_TIMEOUT):
        async with websockets.connect(server_uri):
            pass


async def test_config_sent_on_connect(server_uri):
    async with websockets.connect(server_uri) as ws:
        msg = await _recv_json(ws)
    assert msg["type"] == "config"
    assert isinstance(msg["fps"], int)
    assert msg["fps"] > 0


# ── scene execution ───────────────────────────────────────────────────────────


async def test_run_valid_scene_exits_zero(server_uri):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/scene1.ffpy")
    done = by_type(msgs, "done")
    assert len(done) == 1
    assert done[0]["exit_code"] == 0


async def test_run_valid_scene_sends_tree(server_uri):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/scene1.ffpy")
    tree_msgs = by_type(msgs, "tree")
    assert len(tree_msgs) == 1
    tree = tree_msgs[0]
    assert isinstance(tree["frame_count"], int) and tree["frame_count"] > 0
    assert isinstance(tree["scenes"], list) and len(tree["scenes"]) >= 1


async def test_run_error_scene_exits_nonzero(server_uri, error_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/error.ffpy")
    done = by_type(msgs, "done")
    assert len(done) == 1
    assert done[0]["exit_code"] != 0


async def test_run_error_scene_sends_error_messages(server_uri, error_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/error.ffpy")
    assert by_type(msgs, "error"), "expected at least one error message"


async def test_done_always_sent_on_error(server_uri, error_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/error.ffpy")
    assert by_type(msgs, "done"), "done message must always be sent"


async def test_run_invalid_font_scene_sends_font_error(server_uri, invalid_font_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/invalid_font.ffpy")
    errors = by_type(msgs, "error")
    assert any(
        "font" in e["text"].lower() and "not found" in e["text"].lower() for e in errors
    ), f"expected a font-not-found error, got: {msgs}"
    assert not by_type(msgs, "tree"), "invalid font scene must not send a tree message"


async def test_nonexistent_file_exits_nonzero(server_uri):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/does_not_exist.ffpy")
    done = by_type(msgs, "done")
    assert done[0]["exit_code"] != 0


# ── sequential runs on the same connection ────────────────────────────────────


async def test_two_sequential_runs(server_uri):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        msgs1 = await _run_scene(ws, "scenes/scene1.ffpy")
        msgs2 = await _run_scene(ws, "scenes/scene1.ffpy")
    assert by_type(msgs1, "done")[0]["exit_code"] == 0
    assert by_type(msgs2, "done")[0]["exit_code"] == 0


async def test_error_run_followed_by_successful_run(server_uri, error_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        err_msgs = await _run_scene(ws, "scenes/error.ffpy")
        ok_msgs = await _run_scene(ws, "scenes/scene1.ffpy")
    assert by_type(err_msgs, "done")[0]["exit_code"] != 0
    assert by_type(ok_msgs, "done")[0]["exit_code"] == 0


async def test_many_sequential_runs(server_uri):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        for _ in range(5):
            msgs = await _run_scene(ws, "scenes/scene1.ffpy")
            assert by_type(msgs, "done")[0]["exit_code"] == 0


# ── run cancellation ──────────────────────────────────────────────────────────


async def test_new_run_supersedes_previous(server_uri, slow_scene):
    """Sending a second run while the first is still running cancels the first."""
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        await ws.send(json.dumps({"type": "run", "path": "scenes/slow.ffpy"}))
        # Small delay lets the process start before we cancel it
        await asyncio.sleep(0.3)
        await ws.send(json.dumps({"type": "run", "path": "scenes/scene1.ffpy"}))

        all_msgs = []
        async with asyncio.timeout(WS_TIMEOUT):
            async for raw in ws:
                msg = json.loads(raw)
                all_msgs.append(msg)
                # Stop once the second run's done arrives (non-null exit_code)
                if msg["type"] == "done" and msg.get("exit_code") is not None:
                    break

    done_msgs = by_type(all_msgs, "done")
    assert done_msgs[-1]["exit_code"] == 0


async def test_terminate_stops_current_run(server_uri, slow_scene):
    async with websockets.connect(server_uri) as ws:
        await _recv_json(ws)  # consume config
        await ws.send(json.dumps({"type": "run", "path": "scenes/slow.ffpy"}))
        await asyncio.sleep(0.3)
        await ws.send(json.dumps({"type": "terminate"}))

        all_msgs = []
        async with asyncio.timeout(WS_TIMEOUT):
            async for raw in ws:
                msg = json.loads(raw)
                all_msgs.append(msg)
                if msg["type"] == "done":
                    break

    done = by_type(all_msgs, "done")
    assert len(done) == 1
    assert done[0]["exit_code"] is None


# ── HTTP REST endpoints ───────────────────────────────────────────────────────


def test_unauthorized_without_token(server_base_url):
    url = f"{server_base_url}/ls"
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req, timeout=HTTP_TIMEOUT):
            pytest.fail("expected 401")
    except urllib.error.HTTPError as e:
        assert e.code == 401


def test_ls_returns_entries(server_base_url, server_token):
    status, body = http_get(server_base_url, "/ls", server_token)
    assert status == 200
    entries = json.loads(body)
    assert isinstance(entries, list)
    names = {e["name"] for e in entries}
    assert "scenes" in names
    assert "fairyflow.toml" in names


def test_file_read_returns_content(server_base_url, server_token):
    status, body = http_get(server_base_url, "/file", server_token, path="prologue.py")
    assert status == 200
    assert b"fairyflow" in body


def test_file_write_and_read(server_base_url, server_token):
    content = "# test comment\nfrom fairyflow import *\n"
    status = http_put(
        server_base_url, "/file", server_token, content, path="prologue.py"
    )
    assert status == 204
    status2, body = http_get(server_base_url, "/file", server_token, path="prologue.py")
    assert status2 == 200
    assert body.decode() == content


def test_file_read_missing_returns_404(server_base_url, server_token):
    status, _ = http_get(server_base_url, "/file", server_token, path="nonexistent.py")
    assert status == 404


# ── frame/tree endpoints (require a loaded animation) ─────────────────────────


@pytest.fixture(scope="module")
def loaded_animation(server_uri):
    """Ensure an animation is loaded by running scene1 once. Sync wrapper."""
    asyncio.run(_load_scene(server_uri))


async def _load_scene(uri: str):
    async with websockets.connect(uri) as ws:
        await _recv_json(ws)  # consume config
        msgs = await _run_scene(ws, "scenes/scene1.ffpy")
    assert by_type(msgs, "done")[0]["exit_code"] == 0


def test_frame_endpoint_returns_png(loaded_animation, server_base_url, server_token):
    status, body = http_get(server_base_url, "/frame/0", server_token)
    assert status == 200
    assert body[:4] == b"\x89PNG"


def test_tree_endpoint_returns_json(loaded_animation, server_base_url, server_token):
    status, body = http_get(server_base_url, "/tree/0", server_token)
    assert status == 200
    data = json.loads(body)
    assert isinstance(data, dict)


def test_frames_range_returns_json_array(
    loaded_animation, server_base_url, server_token
):
    status, body = http_get(
        server_base_url, "/frames", server_token, **{"from": "0", "to": "2"}
    )
    assert status == 200
    data = json.loads(body)
    assert isinstance(data, list)


# ── file monitoring (inotify) ─────────────────────────────────────────────────

FILE_CHANGED_TIMEOUT = 5  # seconds


async def test_external_write_triggers_file_changed(server_uri, server_project_dir):
    """Externally writing a file delivers a file_changed WebSocket message with its relative path."""
    watch_file = server_project_dir / "scenes" / "_watch_test.ffpy"
    watch_file.write_text("from fairyflow import *\nwith Scene(): pass\n")
    try:
        async with websockets.connect(server_uri) as ws:
            await _recv_json(ws)  # consume config
            # Simulate an external editor saving the file
            watch_file.write_text("from fairyflow import *\nwith Scene(): pass\n# v2\n")
            async with asyncio.timeout(FILE_CHANGED_TIMEOUT):
                async for raw in ws:
                    msg = json.loads(raw)
                    if (
                        msg["type"] == "file_changed"
                        and msg["path"] == "scenes/_watch_test.ffpy"
                    ):
                        break
                else:
                    pytest.fail("WebSocket closed before file_changed arrived")
    finally:
        watch_file.unlink(missing_ok=True)


async def test_put_file_save_does_not_echo_file_changed(
    server_uri, server_base_url, server_token, server_project_dir
):
    """Saving a file via PUT /file must not echo back as a file_changed event."""
    suppress_file = server_project_dir / "scenes" / "_suppress_test.ffpy"
    suppress_file.write_text("from fairyflow import *\nwith Scene(): pass\n")
    try:
        async with websockets.connect(server_uri) as ws:
            await _recv_json(ws)  # consume config
            http_put(
                server_base_url,
                "/file",
                server_token,
                "from fairyflow import *\nwith Scene(): pass\n# via PUT\n",
                path="scenes/_suppress_test.ffpy",
            )
            received_echo = False
            try:
                # Wait within the 2-second suppression window; no echo should arrive
                async with asyncio.timeout(1.5):
                    async for raw in ws:
                        msg = json.loads(raw)
                        if (
                            msg["type"] == "file_changed"
                            and msg["path"] == "scenes/_suppress_test.ffpy"
                        ):
                            received_echo = True
                            break
            except TimeoutError:
                pass
    finally:
        suppress_file.unlink(missing_ok=True)
    assert not received_echo, "PUT /file must not send a file_changed echo"
