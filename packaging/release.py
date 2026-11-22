# SPDX-License-Identifier: GPL-3.0-or-later
"""Stage native release artifacts. Run from the repository root on the target OS."""

import argparse
import hashlib
import os
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import tarfile
import tomllib


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def version():
    return tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["package"]["version"]


def notices(destination):
    for source in [Path("LICENSE"), Path("THIRD_PARTY.md"),
                   *Path("crates/moth-ui/fonts").glob("LICENSE-*.txt")]:
        shutil.copy2(source, destination)


def archive(path, directory):
    with tarfile.open(path, "w:gz") as output:
        for entry in sorted(directory.iterdir()):
            output.add(entry, arcname=entry.name)


def source_packages(dist):
    committed = subprocess.check_output(["git", "show", "HEAD:Cargo.toml"], text=True)
    release = tomllib.loads(committed)["workspace"]["package"]["version"]
    source = dist / f"mothref-{release}-source.tar.gz"
    run("git", "archive", "--format=tar.gz", f"--prefix=mothref-{release}/",
        "-o", source, "HEAD")
    recipe = Path("package-arch")
    recipe.mkdir()
    text = Path("packaging/arch/PKGBUILD.in").read_text()
    for key, value in {"VERSION": release, "ARCH_VERSION": release.replace("-", ""),
                       "SHA256": hashlib.sha256(source.read_bytes()).hexdigest()}.items():
        text = text.replace(f"@{key}@", value)
    (recipe / "PKGBUILD").write_text(text)
    archive(dist / "mothref-arch-build.tar.gz", recipe)


def stage(target, name, dist):
    package = Path("package")
    package.mkdir()
    notices(package)
    windows = "windows" in target
    executable = "mothref.exe" if windows else "mothref"
    binary = Path("target") / target / "release" / executable
    shutil.copy2(binary, package)
    if windows:
        pe = binary.read_bytes()
        offset = struct.unpack_from("<I", pe, 0x3c)[0]
        assert pe[offset:offset + 4] == b"PE\0\0"
        assert struct.unpack_from("<H", pe, offset + 24 + 68)[0] == 2, "GUI subsystem required"
        shutil.make_archive(str(dist / f"mothref-{name}"), "zip", package)
    elif "apple" in target:
        contents = package / "MothRef.app" / "Contents"
        (contents / "MacOS").mkdir(parents=True)
        (contents / "Resources").mkdir()
        shutil.move(package / executable, contents / "MacOS" / executable)
        shutil.copy2("assets/icons/moth.icns", contents / "Resources/MothRef.icns")
        notices(contents / "Resources")
        info = dict(CFBundleExecutable="mothref", CFBundleIdentifier="org.mothref.MothRef",
                    CFBundleName="MothRef", CFBundlePackageType="APPL", CFBundleIconFile="MothRef.icns",
                    CFBundleShortVersionString=version().split("-")[0], CFBundleVersion="1",
                    NSHighResolutionCapable=True,
                    CFBundleDocumentTypes=[dict(CFBundleTypeName="MothRef board",
                        CFBundleTypeExtensions=["moth"], CFBundleTypeRole="Editor",
                        CFBundleTypeIconFile="MothRef.icns")])
        (contents / "Info.plist").write_bytes(plistlib.dumps(info))
        assert (contents / "MacOS/mothref").stat().st_mode & 0o111
        assert (contents / "Resources/MothRef.icns").read_bytes().startswith(b"icns")
        run("plutil", "-lint", contents / "Info.plist")
        # Ad-hoc sign the complete bundle, including the cross-compiled Intel executable.
        run("codesign", "--force", "--deep", "--sign", "-", package / "MothRef.app")
        run("codesign", "--verify", "--deep", "--strict", package / "MothRef.app")
        archive(dist / f"mothref-{name}.tar.gz", package)
        (package / "Applications").symlink_to("/Applications", target_is_directory=True)
        run("hdiutil", "create", "-volname", "MothRef", "-srcfolder", package,
            "-ov", "-format", "UDZO", dist / f"mothref-{name}.dmg")
        run("hdiutil", "verify", dist / f"mothref-{name}.dmg")
    else:
        for source in Path("packaging/linux").glob("org.mothref.MothRef.*"):
            shutil.copy2(source, package)
        shutil.copy2("docs/assets/icon/mothref-icon-512.png", package / "org.mothref.MothRef.png")
        shutil.copy2("packaging/linux/README.md", package / "DESKTOP.md")
        archive(dist / f"mothref-{name}.tar.gz", package)
        # The native package metadata currently describes x86_64 only.
        if target == "x86_64-unknown-linux-gnu":
            os.environ["MOTHREF_VERSION"] = version()
            for kind in ("deb", "rpm"):
                run("nfpm", "package", "--config", "packaging/linux/nfpm.yaml", "--packager", kind,
                    "--target", dist / f"mothref-{name}.{kind}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=["source", "native"])
    parser.add_argument("--target")
    parser.add_argument("--name")
    args = parser.parse_args()
    if args.kind == "native" and (not args.target or not args.name):
        parser.error("native requires --target and --name")
    dist = Path("dist")
    dist.mkdir(exist_ok=True)
    if args.kind == "source":
        source_packages(dist)
    else:
        stage(args.target, args.name, dist)
