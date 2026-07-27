import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SERVER_BINARY = ROOT / "target" / "debug" / "server"


def _ensure_binary():
    if not SERVER_BINARY.exists():
        subprocess.run(["cargo", "build"], cwd=ROOT, check=True, capture_output=True)


def test_init_project_structure(tmp_path):
    _ensure_binary()
    proj = tmp_path / "myproject"
    result = subprocess.run(
        [str(SERVER_BINARY), "init", str(proj)], capture_output=True, check=False
    )
    assert result.returncode == 0
    assert (proj / "fairyflow.toml").exists()
    assert (proj / "prologue.py").exists()
    assert (proj / "scenes" / "scene1.ffpy").exists()
    assert (proj / "sequences" / "sequence1.ffsq").exists()


def test_init_scene1_evaluates_without_error(tmp_path):
    _ensure_binary()
    proj = tmp_path / "myproject"
    subprocess.run(
        [str(SERVER_BINARY), "init", str(proj)], check=True, capture_output=True
    )
    out_json = tmp_path / "out.json"
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "fairyflow",
            "--prologue",
            str(proj / "prologue.py"),
            str(proj / "scenes" / "scene1.ffpy"),
            str(out_json),
            "24",
        ],
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, f"stderr:\n{result.stderr.decode()}"
