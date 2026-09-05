"""Run one verification command and reap all its Linux descendants.

Native tests create detached process groups. A subreaper adopts those children
when a test exits or is cancelled, so cleanup never needs to scan or signal
unrelated host processes. No third-party Python packages are required.
"""

import ctypes
import os
from pathlib import Path
import signal
import subprocess
import sys
import time


cancelled = False


def cancel(_signal, _frame):
    global cancelled
    cancelled = True


def children():
    return [int(pid) for pid in Path(f"/proc/self/task/{os.getpid()}/children").read_text().split()]


def cleanup():
    deadline = time.monotonic() + 0.25
    while True:
        try:
            while os.waitpid(-1, os.WNOHANG)[0] != 0:
                pass
        except ChildProcessError:
            return
        termination = signal.SIGKILL if time.monotonic() >= deadline else signal.SIGTERM
        for pid in children():
            try:
                os.kill(pid, termination)
            except ProcessLookupError:
                pass
        time.sleep(0.01)


def main():
    libc = ctypes.CDLL(None, use_errno=True)
    # PR_SET_CHILD_SUBREAPER is supported on both workstation and Ubuntu CI.
    if libc.prctl(36, 1, 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), "cannot supervise verification descendants")
    signal.signal(signal.SIGINT, cancel)
    signal.signal(signal.SIGTERM, cancel)
    try:
        command = subprocess.Popen(sys.argv[1:])
        while command.poll() is None and not cancelled:
            time.sleep(0.025)
        result = command.returncode
    finally:
        cleanup()
    if cancelled:
        return 130
    return result if result >= 0 else 128 - result


if __name__ == "__main__":
    sys.exit(main())
