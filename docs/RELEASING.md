# GitHub binary releases

The `Release binaries` workflow builds Windows x86_64, Linux x86_64 and ARM64 (Ubuntu 24.04 /
glibc 2.39 baseline), macOS Apple Silicon and macOS Intel. Packages include the
license and third-party notices. The pipeline publishes a GitHub release only
after the desktop builds, native Arch build and release commit's CI checks pass,
with the complete artifact inventory checked by `packaging/verify.py` and
`SHA256SUMS`. New installers still require native qualification; pipeline code
alone is not evidence that installation or launch works on every target.
Releases are published as regular, non-prerelease releases and marked latest;
the software itself is still an experimental early preview.

To prepare the next version:

1. Update the workspace package and path-dependency versions in `Cargo.toml` and
   `fuzz/Cargo.toml`. Refresh `Cargo.lock` with `cargo check --workspace` and
   `fuzz/Cargo.lock` with `cargo metadata --manifest-path fuzz/Cargo.toml --format-version 1`.
2. Add `docs/releases/vVERSION.md` with downloads, changes and known limitations;
   update `CHANGELOG.md`. Do not claim unfinished roadmap gates are complete.
3. Run the checks in `AGENTS.md`, commit the release and push it to `main`.
4. Create and push an annotated tag matching the Cargo version, for example:

   ```sh
   git tag -a v0.1.0-alpha.2 -m 'MothRef 0.1.0-alpha.2'
   git push origin v0.1.0-alpha.2
   ```

The workflow checks the tag against the binary package version. It creates a
draft only once all jobs have passed, uploads every archive and the checksums,
then publishes. A failed build or CI check leaves the release unpublished. Re-run
a failed workflow after correcting an infrastructure failure; source changes
should get a new version and tag. Existing uploaded assets can be replaced during
a rerun of the same workflow.

Manual workflow dispatch builds downloadable Actions artifacts without creating
a release. This is the first qualification step after merging packaging changes.
Windows installers are unsigned. macOS bundles receive an ad-hoc signature but
are not Developer ID signed or notarized; Gatekeeper may block downloaded apps.
Trusted signing/notarization and publication to package-manager repositories
remain separate roadmap work. No credentials are needed to generate packages.

## Downloads and installation

| Platform | Artifacts | Installation |
| --- | --- | --- |
| Windows x86_64 | `.zip`, `-setup.exe` | Extract and launch, or use the per-user setup wizard (no admin rights). Desktop shortcut and `.moth` Open With registration are optional. |
| macOS Apple Silicon / Intel | `.tar.gz`, `.dmg` for each architecture | Open the DMG and drag `MothRef.app` to Applications, or extract the archive. |
| Linux x86_64 | `.tar.gz`, `.AppImage`, `.deb`, `.rpm` | Extract the archive; make the AppImage executable; or install with apt/dnf. |
| Linux ARM64 (aarch64) | `.tar.gz`, `.AppImage` | Extract the archive, or make `mothref-linux-arm64.AppImage` executable and launch it. |
| Arch Linux x86_64 | `.pkg.tar.zst`, `mothref-arch-build.tar.gz` | Install with pacman, or extract the recipe and build with makepkg. |
| All | Versioned `-source.tar.gz`, `SHA256SUMS` | Sources match the release commit, including the local purcore fork and license notices. |

Examples, from a directory containing the downloaded release files:

```sh
sudo apt install ./mothref-linux-x86_64.deb
sudo dnf install ./mothref-linux-x86_64.rpm
chmod +x mothref-linux-x86_64.AppImage
./mothref-linux-x86_64.AppImage
sudo pacman -U ./mothref-*-x86_64.pkg.tar.zst
```

For Arch source builds, install `base-devel`, extract `mothref-arch-build.tar.gz`
into an empty directory, then run `makepkg -si` as a regular user. It installs
build dependencies, including NASM, fetches the exact source archive and checks
its SHA-256. Cargo dependencies are fetched during `prepare()`; `build()` uses
`--frozen`. To build an unpublished workflow run, download its matching source
archive into the recipe directory first. Generate `.SRCINFO` with
`makepkg --printsrcinfo` when preparing a future AUR submission.

ZIP, app archives and AppImage run without installation; settings, caches and
recovery journals still live in the normal per-user directories. The roadmap's
`portable.txt` mode with settings next to the executable is not implemented.
Installers do not remove boards or user settings during uninstall.
AppImage bundles discoverable runtime libraries, but still needs a compatible
host glibc and graphics drivers. Without FUSE, use
`APPIMAGE_EXTRACT_AND_RUN=1 ./mothref-linux-x86_64.AppImage`.
Linux ARM32, native Windows ARM, universal macOS bundles, MSI, Flatpak and package
repository publication are not part of this matrix.

## Packaging locally and in CI

Build the release binary, then run from the repository root in a clean workspace:

```sh
python3 packaging/release.py native --target x86_64-unknown-linux-gnu --name linux-x86_64
```

Use the corresponding target/name pair from `release.yml` on Windows or macOS.
Linux ARM64 uses `aarch64-unknown-linux-gnu` / `linux-arm64` on a native
`ubuntu-24.04-arm` runner, including the ARM64 linuxdeploy tool and AppImage CLI
startup check. ARM64 currently produces an archive and AppImage; DEB/RPM and Arch
packages remain x86_64 only.
Python 3.11+ is required. Linux DEB/RPM packaging uses nFPM 2.43.4; AppImage uses
linuxdeploy 1-alpha-20251107-1. Windows uses the runner's Inno Setup 6; macOS uses
`codesign`, `plutil` and `hdiutil` from the native toolchain. See the workflow for
installation and invocation commands. `package/` and `package-arch/` must not
already exist; outputs go to `dist/`. The script deliberately does not delete
existing directories. No Rust dependency is added by packaging tools.

`python3 packaging/release.py source` archives committed `HEAD` and generates a
release-specific, checksummed PKGBUILD in `dist/mothref-arch-build.tar.gz`.
It does not include uncommitted edits. CI builds this recipe in Arch Linux as an
unprivileged user and installs the result. Linux CI inspects DEB/RPM metadata,
installs/removes DEB and Arch packages, checks CLI startup from DEB and AppImage,
and installs/runs/removes the RPM in a Fedora 42 container. Windows CI
silently installs and uninstalls the wizard; macOS validates the bundle signature
and disk image. GUI startup, file associations, desktop integration and macOS
drag-and-drop installation still need the native manual matrix; record the
results before release.

macOS archives contain `MothRef.app`; launch that bundle from Finder. The executable
for terminal diagnostics is `MothRef.app/Contents/MacOS/mothref`. Windows archives
contain a GUI-subsystem `mothref.exe`; GPU probes use no-console creation flags.
Both paths retain rotating logs in the application's recovery root under `logs/`.
Before release, complete the native launch and window-transition matrix in
[M3 window validation](M3-WINDOW-VALIDATION.md); packaging assertions alone do not
qualify Explorer/Finder launches or graphics-driver transitions.

Application marks are bundled from `assets/icons/`. Windows embeds `moth.ico`
in the executable; macOS bundles `moth.icns` for Finder, the Dock and `.moth`
documents. Linux archives include the desktop entry, PNG icon, MIME definition
and `DESKTOP.md` with per-user installation commands. x86 builds require NASM for
`rav1d`. Linux uses its distribution's NASM. The Windows and macOS workflows pin
NASM 2.16.03. On ARM, `avif-decode` uses the Rust implementation of `rav1d`.
Before release, run the AVIF regression/corpus and allocation checks, and the
image-reader fuzzer/IO benchmark under the policy in `AGENTS.md`. Manually check
icon switching and relaunch persistence on Windows, macOS and X11; Wayland's
taskbar follows the installed launcher icon, as explained in the setting.
