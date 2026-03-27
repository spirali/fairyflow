import sys
import argparse
import runpy
from .serializer import write_tree


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prologue", metavar="filename")
    parser.add_argument("source_filename")
    parser.add_argument("out_filename")
    args = parser.parse_args()

    context = {}

    if args.prologue:
        context = runpy.run_path(
            args.prologue, init_globals=context, run_name="__main__"
        )

    runpy.run_path(args.source_filename, init_globals=context, run_name="__main__")

    from .nodes import ROOT_OBJECT

    if ROOT_OBJECT is not None:
        write_tree(args.out_filename)


if __name__ == "__main__":
    main()
