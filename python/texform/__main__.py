"""Entry point of the `texform` command and of `python -m texform`."""

import signal
import sys

from ._native import _run_cli


def main() -> int:
    # Python's SIGINT handler only sets a flag that the native tool never
    # checks; restore the default so Ctrl-C ends the process as it does the
    # standalone binary.
    signal.signal(signal.SIGINT, signal.SIG_DFL)
    return _run_cli(["texform", *sys.argv[1:]])


if __name__ == "__main__":
    sys.exit(main())
