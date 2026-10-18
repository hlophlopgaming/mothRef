# SPDX-License-Identifier: GPL-3.0-or-later
import subprocess
import sys
import unittest
from pathlib import Path

from annotate import annotation


class Annotations(unittest.TestCase):
    def test_rust_diagnostic(self):
        self.assertEqual(
            annotation("error: bad\n --> crates/a.rs:12:3\n"),
            "::error file=crates/a.rs,line=12::error: bad%0A --> crates/a.rs:12:3",
        )

    def test_windows_panic(self):
        result = annotation("thread 'test' panicked at C:\\src\\a.rs:8:2:\nbad%\r\n")
        self.assertIn("file=C%3A\\src\\a.rs,line=8", result)
        self.assertIn("bad%25", result)

    def test_exit_status_and_output(self):
        for status in (0, 7):
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("annotate.py")),
                 sys.executable, "-c", f"print('test output'); exit({status})"],
                capture_output=True, text=True,
            )
            self.assertEqual(result.returncode, status)
            self.assertIn("test output", result.stdout)
            self.assertEqual("::error" in result.stdout, status != 0)


if __name__ == "__main__":
    unittest.main()
