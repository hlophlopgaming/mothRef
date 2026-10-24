# GitHub binary releases

The `Release binaries` workflow builds Windows x86_64, Linux x86_64 (Ubuntu 24.04
baseline), macOS Apple Silicon and macOS Intel archives. Archives include the
license and third-party notices. The pipeline publishes a GitHub release only
after all four builds and the release commit's CI checks pass, with `SHA256SUMS`.
Releases are published as regular, non-prerelease releases and marked latest;
the software itself is still an experimental early preview.

To prepare the next version:

1. Update the workspace package and path-dependency versions in `Cargo.toml`, and
   refresh `Cargo.lock` with `cargo check --workspace`.
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
a release. Published binaries are unsigned; signing/notarization, installers and
package-manager distribution follow the roadmap.

macOS archives contain `MothRef.app`; launch that bundle from Finder. The executable
for terminal diagnostics is `MothRef.app/Contents/MacOS/mothref`. Windows archives
contain a GUI-subsystem `mothref.exe`; GPU probes use no-console creation flags.
Both paths retain rotating logs in the application's recovery root under `logs/`.
Before release, complete the native launch and window-transition matrix in
[M3 window validation](M3-WINDOW-VALIDATION.md); packaging assertions alone do not
qualify Explorer/Finder launches or graphics-driver transitions.
