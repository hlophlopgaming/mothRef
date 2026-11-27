# 0032 Desktop updates

## Context

MothRef ships portable binaries, native installers and packages. A restart has to retain acknowledged board changes and leave each installation under the ownership of its distribution.

## Decision

Official standalone builds check public GitHub Releases after loading the user’s startup preferences. The checker caches release metadata and the day’s attempt in per-user state. It compares versions using SemVer and only offers packages matching the current architecture and distribution. Release descriptions feed the update preview and What’s new window. A manual What’s new request uses the installed tag.

After the user chooses Download, a worker verifies the package against the official release’s `SHA256SUMS` and stages it on the installation volume. A helper waits for the app to release the board workers and executable, then runs Inno Setup, exchanges the macOS bundle, or atomically replaces the portable executable or AppImage. Each platform keeps the previous executable or bundle during startup supervision. The new version confirms its identity after presenting its first frame. If startup fails, the helper restores the previous executable or bundle.

The restart plan records saved board paths and private journal identities for unsaved edits, grouped by window. The updater uses the existing storage worker and recovery rules to reopen them after startup. It keeps a saved board’s old journal until the file opens successfully. Package-managed copies leave installation to the package manager.

## Consequences

No updater request sends a board path or any other board data to GitHub. Release artifacts are trusted through GitHub TLS and the project’s published checksums. The checksum verifies that the download matches the release. OS signing and notarization remain separate. Windows rollback restores the executable, while Inno Setup still owns installation metadata. Native qualification is listed in the release checklist.
