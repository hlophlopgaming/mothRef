# M1 validation

M1's persistence implementation is present; **the reliability release is not yet
qualified**. Do not mark the whole milestone or publish 0.1 as reliability-tested
until all remaining gates below pass.

## Executed on Linux, 2026-10-16

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including property round-trips, every-byte
  final-record/checkpoint truncation, corrupted payload recovery, journal write
  failure, undo/redo/save/open and compaction failure/replacement tests.
- `cargo deny check`: passed (existing duplicate-version warnings remain).
- `cargo xtask corpus`: five image/board fixtures validated.
- `cargo xtask bench --quick`: 10,000 distinct images, AMD RX 5700 XT,
  p99 **5.062 ms**, textures **13.32 MiB**. Rendering was not changed by M1.
- `cargo xtask killtest 1000`: passed, **zero acknowledged commands lost**.
  Child processes are killed at varying points around split record writes and
  footer/fsync acknowledgment; recovery is checked after each kill.
- `cargo +nightly fuzz run moth_reader -- -max_total_time=300`: passed,
  **11,867,392 executions in 301 seconds**, seeded with the schema-one fixture.
- `cargo check --target aarch64-linux-android -p moth-scene -p moth-format -p moth-io`:
  passed.
- Windows platform type-check with `--features blake3/pure`: passed. The normal
  Linux-to-MSVC cross-check cannot run BLAKE3's assembly build without `ml64.exe`;
  native Windows CI uses the normal feature set.
- Native X11 smoke: opened `corpus/moth/v1.moth`, established a recovery journal,
  SIGTERM exited successfully without a dialog and retained the journal.

`bee_reader` and `pur_reader` fuzz commands were attempted but those targets do
not exist yet. Import readers and their fuzz targets belong to M2; no import code
or decoder path was changed for M1.

## Remaining release gates

- The locally started 30-minute soaks must finish: the original renderer run
  stayed at **188,284 KiB** during observation; the updated combined
  render/persistence workload also exercises journal writes, bounded undo/redo,
  saves, backups and compaction. A two-second combined smoke run passed. Logs are
  `/tmp/mothref-m1-soak.log` and `/tmp/mothref-m1-persistence-soak.log`.
- Run and record the full **eight-hour** combined soak (`cargo xtask soak 28800`). The manually triggered GitHub workflow defaults to
  30 minutes; hosted runner job limits preclude one uninterrupted eight-hour job.
- Runtime compressed texture upload/encoding (BC7 / ASTC / ETC2) is still open.
  Current previews use portable RGBA8; schema-one originals are format-neutral
  and contain no persistent GPU-specific LOD cache.
- Native manual save/recover/clipboard/close testing on Windows and macOS,
  including WM_QUERYENDSESSION and applicationShouldTerminate. All acknowledged
  mutations are already synced, but native desktop termination UX needs testing.

## Published experimental preview

[v0.1.0-alpha.1](https://github.com/hlophlopgaming/mothRef/releases/tag/v0.1.0-alpha.1)
was published by the [release workflow](https://github.com/hlophlopgaming/mothRef/actions/runs/38922605455).
The release commit's [CI](https://github.com/hlophlopgaming/mothRef/actions/runs/38922605451)
passed on native Windows, macOS and Linux runners, including workspace tests and
clippy, plus license and Android core checks.

All four published archives were downloaded and verified against `SHA256SUMS`;
binary architectures, Unix executable permissions and license notices were
checked. The GitHub-built Linux binary opened the board fixture under X11,
created its recovery journal, and exited successfully on SIGTERM while retaining
that journal. This does not replace native Windows/macOS manual UX testing or the
full M1 reliability gates above.

## Current product limits

Open uses an editable native path or recent/recovery buttons. Opening another
board requires an empty canvas; tabs remain M3. Undo/redo is capped at 256 session
commands and is not restored after reopening. The M0 10,000-item render/UI limit,
2048-pixel previews and native Wayland file-drop limitation remain in place.
