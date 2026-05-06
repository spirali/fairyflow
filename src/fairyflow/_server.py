import os
import sys
from pathlib import Path


def main():
    package_dir = Path(__file__).parent
    binary = package_dir / "_bin" / "server"

    if not binary.exists():
        print(
            "error: bundled server binary not found\n"
            "       Run scripts/build_package.sh to build the package first.",
            file=sys.stderr,
        )
        sys.exit(1)

    binary.chmod(0o755)

    env = os.environ.copy()
    env["FAIRYFLOW_WEB_DIST"] = str(package_dir / "_web")

    os.execve(str(binary), [str(binary)] + sys.argv[1:], env)
