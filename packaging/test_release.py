# SPDX-License-Identifier: GPL-3.0-or-later
"""Packaging regressions; CI runs these without building the application."""

import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

from verify import verify


class ReleaseInventory(unittest.TestCase):
    def test_release_requires_every_format_and_architecture(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for name in ("mothref-linux-x86_64.deb", "mothref-linux-x86_64.rpm",
                         "mothref-linux-x86_64.AppImage", "mothref-windows-x86_64-setup.exe",
                         "mothref-linux-arm64.AppImage",
                         "mothref-macos-arm64.dmg", "mothref-macos-x86_64.dmg",
                         "mothref-arch-build.tar.gz", "mothref-0.1.0-alpha.27-source.tar.gz",
                         "mothref-0.1.0alpha.27-1-x86_64.pkg.tar.zst"):
                (directory / name).write_bytes(b"fixture")
            for platform in ("linux-x86_64", "linux-arm64", "macos-arm64", "macos-x86_64"):
                binary = "MothRef.app/Contents/MacOS/mothref" if platform.startswith("macos") else "mothref"
                with tarfile.open(directory / f"mothref-{platform}.tar.gz", "w:gz") as archive:
                    for name in (binary, "LICENSE", "THIRD_PARTY.md"):
                        info = tarfile.TarInfo(name)
                        info.size = 7
                        archive.addfile(info, io.BytesIO(b"fixture"))
            with zipfile.ZipFile(directory / "mothref-windows-x86_64.zip", "w") as archive:
                for name in ("mothref.exe", "LICENSE", "THIRD_PARTY.md"):
                    archive.writestr(name, b"fixture")
            verify(directory)
            for path in directory.iterdir():
                hidden = path.with_suffix(path.suffix + ".missing")
                path.rename(hidden)
                with self.assertRaises(ValueError):
                    verify(directory)
                hidden.rename(path)


if __name__ == "__main__":
    unittest.main()
