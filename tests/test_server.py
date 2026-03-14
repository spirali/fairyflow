"""Integration tests: start the real server and communicate over WebSocket."""

import asyncio
import json

import pytest
import websockets

WS_TIMEOUT = 15  # seconds — any single test must finish within this time


async def _run_code_on_ws(ws, code: str) -> list[dict]:
    """Submit code on an existing WebSocket and collect messages until done."""
    await ws.send(json.dumps({"type": "run", "code": code}))
    messages = []
    async with asyncio.timeout(WS_TIMEOUT):
        async for raw in ws:
            msg = json.loads(raw)
            messages.append(msg)
            if msg["type"] == "done":
                break
    return messages


async def _run_code(uri: str, code: str) -> list[dict]:
    """Connect to the server, submit code, and collect all messages until done."""
    async with websockets.connect(uri) as ws:
        return await _run_code_on_ws(ws, code)


# ── helpers ──────────────────────────────────────────────────────────────────


def by_type(messages: list[dict], kind: str) -> list[dict]:
    return [m for m in messages if m["type"] == kind]


def output_text(messages: list[dict]) -> str:
    return "\n".join(m["text"] for m in by_type(messages, "output"))


# ── basic connectivity ────────────────────────────────────────────────────────


async def test_server_accepts_connection(server_uri):
    async with asyncio.timeout(WS_TIMEOUT):
        async with websockets.connect(server_uri):
            pass  # just connecting is enough


# ── code execution ────────────────────────────────────────────────────────────


async def test_print_output_is_forwarded(server_uri):
    msgs = await _run_code(server_uri, "print('hello world')")
    assert output_text(msgs) == "hello world"


async def test_multiple_print_lines(server_uri):
    msgs = await _run_code(server_uri, "print('a')\nprint('b')\nprint('c')")
    assert output_text(msgs) == "a\nb\nc"


async def test_successful_run_exits_with_code_zero(server_uri):
    msgs = await _run_code(server_uri, "x = 1 + 1")
    done = by_type(msgs, "done")
    assert len(done) == 1
    assert done[0]["exit_code"] == 0


async def test_syntax_error_exits_nonzero(server_uri):
    msgs = await _run_code(server_uri, "def (")
    done = by_type(msgs, "done")
    assert len(done) == 1
    assert done[0]["exit_code"] != 0


async def test_runtime_error_sends_error_messages(server_uri):
    msgs = await _run_code(server_uri, "raise ValueError('boom')")
    assert by_type(msgs, "error"), "expected at least one error message"
    done = by_type(msgs, "done")
    assert done[0]["exit_code"] != 0


async def test_done_message_always_sent(server_uri):
    """done must arrive even when the script crashes."""
    msgs = await _run_code(server_uri, "1 / 0")
    assert by_type(msgs, "done")


# ── scene tree ────────────────────────────────────────────────────────────────


async def test_tree_sent_on_success(server_uri):
    msgs = await _run_code(server_uri, """
with scene(100, 100):
    pass
    """)
    tree_msgs = by_type(msgs, "tree")
    assert len(tree_msgs) == 1


# ── repeated submissions on same connection ───────────────────────────────────


async def test_two_sequential_runs(server_uri):
    """Second run on the same connection returns correct output."""
    async with websockets.connect(server_uri) as ws:
        msgs1 = await _run_code_on_ws(ws, "print('first')")
        msgs2 = await _run_code_on_ws(ws, "print('second')")
    assert output_text(msgs1) == "first"
    assert output_text(msgs2) == "second"


async def test_many_sequential_runs(server_uri):
    """Ten runs in a row all complete and return correct results."""
    async with websockets.connect(server_uri) as ws:
        for i in range(10):
            msgs = await _run_code_on_ws(ws, f"print({i})")
            assert output_text(msgs) == str(i)


async def test_tree_updates_between_runs(server_uri):
    """Each run replaces the previous tree — second run produces a different tree."""
    async with websockets.connect(server_uri) as ws:
        msgs1 = await _run_code_on_ws(ws, """
with scene(100, 100):
    rect().width(10)
""")
        msgs2 = await _run_code_on_ws(ws, """
with scene(200, 150):
    pass
""")
    tree1 = by_type(msgs1, "tree")[0]
    tree2 = by_type(msgs2, "tree")[0]
    assert tree1["frames"][0]["children"][0]["kind"] == "rect"
    assert tree1["frames"][0]["children"][0]["width"] == 10
    assert tree2["frames"][0]["width"] == 200
    assert tree2["frames"][0]["height"] == 150


async def test_error_run_followed_by_successful_run(server_uri):
    """A run that crashes does not break the connection for the next run."""
    async with websockets.connect(server_uri) as ws:
        err_msgs = await _run_code_on_ws(ws, "raise RuntimeError('oops')")
        ok_msgs = await _run_code_on_ws(ws, "print('recovered')")
    assert by_type(err_msgs, "done")[0]["exit_code"] != 0
    assert output_text(ok_msgs) == "recovered"
    assert by_type(ok_msgs, "done")[0]["exit_code"] == 0


async def test_new_run_supersedes_previous(server_uri):
    """Sending a second run while the first is still running cancels the first."""
    async with websockets.connect(server_uri) as ws:
        # Send a slow script but don't wait for it to finish
        await ws.send(
            json.dumps({"type": "run", "code": "import time; time.sleep(30)"})
        )
        # Immediately send a fast second run
        await ws.send(json.dumps({"type": "run", "code": "print('superseded')"}))
        # Collect all messages; the killed first run sends done(exit_code=null),
        # the second run sends done(exit_code=0) — stop at the first non-null done.
        all_msgs = []
        async with asyncio.timeout(WS_TIMEOUT):
            async for raw in ws:
                msg = json.loads(raw)
                all_msgs.append(msg)
                if msg["type"] == "done" and msg.get("exit_code") is not None:
                    break
    assert any(m["type"] == "output" and m["text"] == "superseded" for m in all_msgs)
    done_msgs = by_type(all_msgs, "done")
    assert done_msgs[-1]["exit_code"] == 0


# ── timeout protection ────────────────────────────────────────────────────────


async def test_infinite_loop_times_out(server_uri):
    """An infinite loop must not block the test suite — asyncio.timeout fires."""
    with pytest.raises((TimeoutError, asyncio.TimeoutError)):
        await asyncio.wait_for(
            _run_code(server_uri, "while True: pass"),
            timeout=3,
        )
