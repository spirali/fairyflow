#!/usr/bin/env python3
"""Check that every ffpy code block in docs/ compiles without error.

Usage:
    uv run scripts/check_docs_snippets.py [file ...]

Without arguments, checks all *.md files under docs/.
"""

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"

# Match opening fence line and capture everything up to the closing ```
FENCE_RE = re.compile(r"^(```ffpy[^\n]*)\n(.*?)^```", re.MULTILINE | re.DOTALL)


def check_snippet(source: str, label: str) -> bool:
    with tempfile.TemporaryDirectory(prefix="ffpy-check-") as tmp_str:
        tmp = Path(tmp_str)
        (tmp / "prologue.py").write_text("from fairyflow import *\n")
        (tmp / "scene.py").write_text(source)
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "fairyflow",
                "--prologue",
                str(tmp / "prologue.py"),
                str(tmp / "scene.py"),
                str(tmp / "anim.json"),
                "24",
            ],
            capture_output=True,
            text=True,
            cwd=str(ROOT),
        )
    if result.returncode != 0:
        print(f"FAIL  {label}")
        stderr = result.stderr.strip()
        for line in stderr.splitlines():
            print(f"      {line}")
        return False
    print(f"ok    {label}")
    return True


def collect_snippets(md_files: list[Path]) -> list[tuple[str, str]]:
    """Return list of (source, label) pairs."""
    snippets = []
    for md_file in md_files:
        text = md_file.read_text()
        for m in FENCE_RE.finditer(text):
            line_num = text[: m.start()].count("\n") + 1
            label = f"{md_file.relative_to(ROOT)}:{line_num}"
            snippets.append((m.group(2), label))
    return snippets


def main() -> None:
    if len(sys.argv) > 1:
        md_files = [Path(p).resolve() for p in sys.argv[1:]]
    else:
        md_files = sorted(DOCS.glob("*.md"))

    if not md_files:
        print("No markdown files found.", file=sys.stderr)
        sys.exit(1)

    snippets = collect_snippets(md_files)
    if not snippets:
        print("No ffpy blocks found.")
        sys.exit(0)

    failures = sum(1 for source, label in snippets if not check_snippet(source, label))
    total = len(snippets)
    passed = total - failures

    print(f"\n{passed}/{total} snippets passed")
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    main()
