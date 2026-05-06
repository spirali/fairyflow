import os
import platform
import subprocess
import sys
from pathlib import Path


def main():
    package_dir = Path(__file__).parent
    is_windows = platform.system() == "Windows"
    binary_name = "server.exe" if is_windows else "server"
    binary = package_dir / "_bin" / binary_name

    if not binary.exists():
        print(
            "error: bundled server binary not found\n"
            "       Run scripts/build_package.sh to build the package first.",
            file=sys.stderr,
        )
        sys.exit(1)

    env = os.environ.copy()
    env["FAIRYFLOW_WEB_DIST"] = str(package_dir / "_web")

    if is_windows:
        sys.exit(subprocess.run([str(binary)] + sys.argv[1:], env=env).returncode)
    else:
        binary.chmod(0o755)
        os.execve(str(binary), [str(binary)] + sys.argv[1:], env)
