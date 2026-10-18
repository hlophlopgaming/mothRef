# PureRef import validation — 2026-10-18

## Scope and limits

Local purcore fork based on LazyRef commit
`e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654`, authorized by the maintainer.
The import boundary reads streams; no scratch database, filesystem path, C SQLite
library or implicit external-image/network access is needed in core crates.

Captured 1.10 and 2.1 redbar files preserve the exact 157-byte PNG
(MD5 `3f97a27e8cbfd743b3d6e3a675290a0b`) and its 1000×750 frame at (0,0).
The corpus command imports, writes a `.moth` representation and reloads it,
comparing item metadata. Storage-worker tests additionally read the actual
recovery journal, check original bytes and exercise undo/redo.

Unit/property tests cover rectangular 2.x crops and mirroring, uniform frame
rotation/scale, rejection of shear/nonuniform scale, plain versus rich notes,
linked-image skipping, callback errors, every truncation of both captured files,
corrupt checksums, bounds-checked blob decoding and legacy text-depth limits.
Fuzzing repairs checksums on a second pass so mutations reach the inner parser.

Budgets: 4 GiB per encoded board (both formats), 32 MiB per SQLite record or
legacy original, 8 MiB / 8192 retained metadata items, 64 parent levels and
32 levels of legacy text nesting. Both readers stream payloads; the former
16 MiB whole-file legacy buffer has been removed. The shared foreign-import
staging queue also caps metadata at 8 MiB / 8192 placements. These admission
limits can reject legitimate large files; the source is not modified.

Unsupported content is reported through the partial-import warning: linked
images, drawings, group decoration, rich/fixed-size notes, opacity, unsupported
flags, legacy crops and geometry the renderer cannot represent. Source names,
paths, comments and other fields absent from MothRef's scene model are not
persisted. Other PureRef versions are not yet qualified; the parent roadmap
checkbox remains open.

## Automated checks

- `cargo fmt --all --check` and workspace clippy with `-D warnings`: passed.
- `cargo test --workspace`: passed, followed by reruns after fixes.
- `cargo xtask corpus`: passed, 27 fixtures.
- `cargo deny check`: passed; existing duplicate-version warnings remain.
- `cargo check --target aarch64-linux-android -p moth-import`: passed.
- `pur_reader` fuzz, 30 seconds: 89,526 executions, no failure on final code.
- `bee_reader` fuzz, 30 seconds: 618,070 executions, no failure.
- `cargo xtask killtest 50`: passed, no acknowledged commands lost.
- `cargo xtask soak 300 5`: passed, 2850 frames; RSS 192,044 KiB at start,
  193,028 KiB after warm-up and stable through the final sample at 295 seconds.
- `cargo xtask bench --quick`: passed; 10,000 distinct images, 100 frames,
  p99 5.333 ms, 13.32 MiB textures, RX 5700 XT / RADV. This is a general render
  smoke benchmark, not a before/after measurement of import throughput.

The first fuzz attempt exposed upstream's panic-based checksum handling, which
aborts under the fuzz harness despite an outer unwind guard. The local primitive,
blob and legacy readers were converted to typed errors. Regression tests and
subsequent fuzz runs pass without disabling panic detection.

## Native/manual qualification

Linux: automated storage-worker integration and headless renderer tests passed;
the native file-picker/drop/CLI workflow was not manually exercised.
Windows/macOS: native import UI was not run in this Linux session.
No PureRef executable was inspected. Tier-3 reliability/release checks were not
run and remain the CI/maintainer gate. Coverage is limited to the verified samples and supported content described here;
qualifying every PureRef version is not an M2 requirement.

## 4 GiB admission and private battle board

The maintainer requested a 4 GiB limit for the entire board, retaining independent
payload and metadata budgets. Tests cover admission at exactly 4 GiB and rejection
above it using virtual file lengths, a legacy PNG enlarged past the former 16 MiB
limit with an in-memory ancillary chunk, and PNG markers crossing scan windows.
A real 4 GiB file has not been tested.

Private local input: `FemaleFighterPosesPack.pur`, 208,180,100 bytes (198.54 MiB),
format 2.1, application version 2.1.2. Source SHA-256:
`92500ddecb2377743e2e2ddc22db2841c5c86eae1f6416dbfe107eb804cb29f2`.
The artwork stays outside the committed corpus; no redistribution license was assumed.

Reproduce the storage-worker qualification with:

```sh
cargo xtask pur FemaleFighterPosesPack.pur
```

Results on this machine, release executable with build time excluded:

- 175 images, zero notes, zero skipped items.
- Durable import: 0.929 seconds; full import/save/reload/undo/redo process: 2.038 seconds
  (including the final source SHA-256 read in the measurement wrapper).
- Peak child-process RSS: 19,084 KiB (18.64 MiB), measured via `getrusage`.
- All 175 original blob identities and all item fields survive `.moth` save/reload.
- Undo removes the whole import in one step; redo restores every item field.
- Source SHA-256 before/after matches.

The first attempt skipped all 175 images: this board's SQLite columns have a
physical order different from the small 2.1.3 fixture. Import now resolves columns
from schema names, with a regression test reproducing the 2.1.2 layout. The check
compares scenes by UUID and complete item fields rather than nondeterministic
HashMap serialization order. It runs the actual storage worker, not the native UI,
and does not measure image decoding/GPU residency or qualify every PureRef version.

Follow-up automated checks: workspace fmt/clippy/tests, 27-file corpus conversion,
Android `moth-import` check and `killtest 50` passed. The final 30-second fuzz runs
completed 91,118 `pur_reader` and 566,742 `bee_reader` executions without failure.
The earlier five-minute soak remains baseline evidence for the unchanged runtime
scene/history/cache path; this follow-up changes transient import parsing and
the qualification harness, not a long-lived cache. Native UI and a physical
4 GiB input were not exercised.
