# SPDX-License-Identifier: GPL-3.0-or-later
"""Keep Cargo output and exit status while exposing failures in Actions annotations."""

from collections import deque
import os
import re
import subprocess
import sys


def escape(value, property=False):
    value = value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    return value.replace(":", "%3A").replace(",", "%2C") if property else value


def annotation(output):
    location = re.search(r"(?:-->\s+|panicked at )(.+?):(\d+):\d+", output)
    properties = ""
    if location:
        properties = f" file={escape(location[1], True)},line={location[2]}"
    return f"::error{properties}::{escape(output.strip())}"


def main(command):
    env = dict(os.environ, CARGO_TERM_COLOR="never", RUST_BACKTRACE="0")
    tail = deque(maxlen=30)
    diagnostic = []
    with subprocess.Popen(
        command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, encoding="utf-8", errors="replace", env=env,
    ) as process:
        for line in process.stdout:
            print(line, end="", flush=True)
            tail.append(line)
            if line.startswith("error") or re.search(r"thread .* panicked at ", line):
                if diagnostic:
                    print(annotation("".join(diagnostic)), flush=True)
                diagnostic = [line]
            elif diagnostic:
                diagnostic.append(line)
                if not line.strip() or len(diagnostic) >= 20:
                    print(annotation("".join(diagnostic)), flush=True)
                    diagnostic = []
        status = process.wait()
    if diagnostic:
        print(annotation("".join(diagnostic)))
    if status:
        print(annotation("Command failed: " + " ".join(command) + "\n" + "".join(tail)))
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
