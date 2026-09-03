#!/usr/bin/env python3

"""Measure one child process and record elapsed time and maximum RSS."""

from __future__ import annotations

import argparse
import resource
import subprocess
import sys
import time
from pathlib import Path


def maximum_rss_bytes() -> int:
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    value = int(usage.ru_maxrss)
    # Linux reports KiB; macOS reports bytes.
    return value if sys.platform == "darwin" else value * 1024


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", required=True, type=float)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()

    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("a command is required after --")

    args.log.parent.mkdir(parents=True, exist_ok=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    status = 0
    with args.log.open("wb") as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
        try:
            process.wait(timeout=args.timeout)
            status = process.returncode
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
            status = 124

    elapsed_ms = round((time.monotonic() - started) * 1000)
    args.output.write_text(
        f"{elapsed_ms}\t{maximum_rss_bytes()}\n",
        encoding="utf-8",
    )
    return status


if __name__ == "__main__":
    raise SystemExit(main())
