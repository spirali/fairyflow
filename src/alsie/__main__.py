import sys
import runpy
from .serializer import write_tree

def main():
    # Usage: <source_filename> <out_filename>
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} <source_filename> <out_filename>", file=sys.stderr)
        sys.exit(1)

    source_filename = sys.argv[1]
    out_filename = sys.argv[2]

    _pkg = sys.modules[__package__]
    init_globals = {k: getattr(_pkg, k) for k in _pkg.__all__}
    runpy.run_path(source_filename, init_globals=init_globals, run_name="__main__")

    from .items import ROOT_OBJECT
    if ROOT_OBJECT is not None:
        write_tree(out_filename)



if __name__ == "__main__":
    main()