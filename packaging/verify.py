# SPDX-License-Identifier: GPL-3.0-or-later
"""Reject incomplete release uploads before publishing any assets."""

import argparse
from pathlib import Path
import tarfile
import zipfile


def verify(directory):
    expected = {
        "mothref-linux-x86_64.tar.gz", "mothref-linux-x86_64.deb",
        "mothref-linux-x86_64.rpm", "mothref-linux-x86_64.AppImage",
        "mothref-linux-arm64.tar.gz", "mothref-linux-arm64.AppImage",
        "mothref-windows-x86_64.zip", "mothref-windows-x86_64-setup.exe",
        "mothref-macos-arm64.tar.gz", "mothref-macos-arm64.dmg",
        "mothref-macos-x86_64.tar.gz", "mothref-macos-x86_64.dmg",
        "mothref-arch-build.tar.gz",
    }
    names = {path.name for path in directory.iterdir() if path.is_file()}
    sources = {name for name in names if name.startswith("mothref-") and name.endswith("-source.tar.gz")}
    arch = {name for name in names if name.startswith("mothref-") and name.endswith("-x86_64.pkg.tar.zst")}
    if len(sources) != 1 or len(arch) != 1 or names != expected | sources | arch:
        raise ValueError(f"Incomplete release: missing={sorted(expected - names)}, files={sorted(names)}")
    for path in directory.iterdir():
        if not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f"Empty or unexpected artifact: {path}")
    for platform in ("linux-x86_64", "linux-arm64", "macos-arm64", "macos-x86_64"):
        with tarfile.open(directory / f"mothref-{platform}.tar.gz") as archive:
            entries = set(archive.getnames())
            binary = "MothRef.app/Contents/MacOS/mothref" if platform.startswith("macos") else "mothref"
            if not {binary, "LICENSE", "THIRD_PARTY.md"} <= entries:
                raise ValueError(f"Missing executable or notices in {platform}")
    with zipfile.ZipFile(directory / "mothref-windows-x86_64.zip") as archive:
        if not {"mothref.exe", "LICENSE", "THIRD_PARTY.md"} <= set(archive.namelist()):
            raise ValueError("Missing Windows executable or notices")
        if "mothref.install" in archive.namelist():
            raise ValueError("Windows portable ZIP contains the setup marker")
        if archive.testzip() is not None:
            raise ValueError("Corrupt Windows ZIP")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    verify(parser.parse_args().directory)
