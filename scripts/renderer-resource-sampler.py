"""Linux process resource samples. No tracing, dependencies, or renderer hooks."""
import json
import os
import sys
import time

pid = int(sys.argv[1])
interval = float(sys.argv[2]) / 1000
clock_ticks = os.sysconf("SC_CLK_TCK")
page_bytes = os.sysconf("SC_PAGE_SIZE")
started = time.monotonic()
next_sample = started
while True:
    try:
        with open(f"/proc/{pid}/stat", encoding="utf8") as source:
            # comm can contain spaces and parentheses; fields start after its last ')'.
            fields = source.read().rsplit(")", 1)[1].split()
        sample = {
            "elapsedMs": (time.monotonic() - started) * 1000,
            "cpuSeconds": (int(fields[11]) + int(fields[12])) / clock_ticks,
            "rssBytes": int(fields[21]) * page_bytes,
        }
    except (OSError, ValueError, IndexError) as error:
        sample = {"elapsedMs": (time.monotonic() - started) * 1000,
                  "cpuSeconds": None, "rssBytes": None, "error": type(error).__name__}
    print(json.dumps(sample), flush=True)
    next_sample += interval
    time.sleep(max(0, next_sample - time.monotonic()))
