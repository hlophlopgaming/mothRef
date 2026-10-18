# Bulk import and board-open performance — 2026-10-18

## Reproduction

Build first, then run each measurement alone, without builds, fuzzers or other
benchmarks. Use a disk-backed `TMPDIR`; `/tmp` on this workstation is tmpfs and
hides almost all durable-write latency.

```sh
cargo build --release -p xtask
mkdir -p target/load-profile
TMPDIR="$PWD/target/load-profile" target/release/xtask load /path/to/board.bee
TMPDIR="$PWD/target/load-profile" target/release/xtask load /path/to/board.moth
TMPDIR="$PWD/target/load-profile" target/release/xtask load /path/to/board.pur
TMPDIR="$PWD/target/load-profile" target/release/xtask load /path/to/image-directory
```

`load` optionally accepts a new save destination as its second argument. Saving
and verification happen after the measured load. Existing files are never replaced.
Its regression smoke test exercises a captured BeeRef board and GPU preview.

The harness uses the real storage worker, bounded decode pool (8 requests, 2
results, at most 4 consumers), and headless renderer with GPU completion. It warms
the renderer before starting the timer. A separate no-output parser probe warms
board input and is excluded from end-to-end timings. First image means the first
256-pixel RGBA preview drawn after its durable storage completion; full load means
all unique originals have been decoded and drawn once. Eviction stays enabled;
this does not require keeping every image resident at once. Decode wait is elapsed
time awaiting pool results, not the sum of parallel decoder CPU times. The worker
measurement includes parsing, source copies/hashing and journal syncs.

This isolates the pipeline from native dialogs, event-loop scheduling, BC7
compression and window presentation. It does not claim native first-frame latency,
cold-cache performance, full-resolution detail or the roadmap's hardware targets.
Bulk directory admission is bounded to the app's 1024-file waiting limit.

## Inputs

- Bulk: 200 distinct encoded JPEG originals, approximately 131 MiB.
- BeeRef: 1000 distinct encoded JPEG originals, 686,211,072-byte database.
- MothRef: the same 1000 originals converted through the storage worker,
  685,543,548 bytes.
- PureRef: private `FemaleFighterPosesPack.pur`, 208,180,100 bytes, 175 images.
  Artwork remains outside the repository; see M2-IMPORT-VALIDATION.md for identity
  and earlier round-trip qualification. The existing local file is read-only.

The bulk/BeeRef scaling fixture repeats the repository's
`images/foxes/fox-in-snow.jpg` with a four-byte little-endian counter appended to
each encoded source, producing distinct content hashes without re-encoding.
It represents repeated photographic decode/write cost, not diverse codecs or
artwork. The real PureRef board supplies diverse originals. Generated fixtures
are not redistributed.

To recreate the BeeRef fixture with Python's standard library, create the `items`
and `sqlar` tables using the SQL schemas from the local
`images/beerefTestFile.bee` (`sqlite_master`). For counter `i` in `0..1000`, insert
`items (i+1, 'pixmap', (i%40)*256, (i//40)*256, i, 1, 0, 1,
'{"crop":[0,0,1920,1280]}')` and
`sqlar (str(i), i+1, 0, 0, len(bytes), bytes)`. Write the first 200 originals into
the bulk directory. Use `load large.bee large.moth` to generate the MothRef input.

## Findings and change

An initial memory-backed serial probe spent 40.4 of 42.7 seconds decoding the
1000-image fixture. Switching the measurement to the actual bounded decode pool
avoids confusing serial decoder cost with application throughput. Disk-backed
runs instead expose synchronous durability as the dominant load cost.

Foreign import wrote and synced each original twice into `.source` working copies,
then synced separate blob and command journal records, despite returning only
one completion after the final snapshot. A regression test failed with two working
copies for one original before the fix. Foreign imports now reuse the first copy
and sync the journal once at the existing final snapshot. MothRef opening also
syncs its pending blobs with its final snapshot. Bulk individual inserts share
one journal sync between a blob and its command.

`.source` files are reconstructible working copies, not the recovery authority.
Their redundant file/directory syncs are removed; original bytes and checksummed
commands still reach the journal before any completion. Clipboard staging retains
its original durability boundary. Append/sync errors poison the worker. No format,
checksum, decoder, queue, history, catalog or memory budget changes.

The only repeated recovery-directory enumeration found is clipboard admission in
`DesktopStorage::staging_directory`. File-drop imports, `.moth` opening and foreign
board loading do not call it. Startup discovery already runs only for the first
tab. No directory-scan cache was added: it would not speed these paths and would
need to account for both clipboard and storage writes to preserve the 8 GiB limit.

Tracing spans distinguish `open_board`, `parse_moth`, `import_board`,
`stage_original`, `read_original`, `durable_record`, and the existing `decode` span.

## Measurements

Linux 6.18.49, Ryzen 5 3600 (12 logical CPUs), RX 5700 XT/RADV, Rust 1.99,
release build. Recovery writes use `/home` on Btrfs (`compress=zstd:3`),
not tmpfs. Input fixtures are warm. These are single-run comparisons on an
interactive workstation, not a calibrated release benchmark or confidence interval.
All runs were foreground and sequential, without a concurrent build or fuzzer.

| Input | First image before → after | All previews before → after | Full-load speedup |
| --- | --- | --- | --- |
| 200-file bulk | 0.520 → 0.069 s | 15.366 → 3.180 s | 4.8× |
| 1000-image `.bee` | 171.596 → 16.056 s | 183.319 → 27.034 s | 6.8× |
| 1000-image `.moth` | 135.021 → 30.109 s | 145.357 → 41.431 s | 3.5× |
| 175-image `.pur` | 32.584 → 11.291 s | 33.857 → 12.763 s | 2.7× |

| Input | Parse-only probe before → after | Storage worker before → after | After decode-result waits | After peak RSS |
| --- | --- | --- | --- | --- |
| Bulk | n/a | 15.177 → 3.018 s | 0.049 s | 227,052 KiB |
| `.bee` | 0.231 → 0.229 s | 171.545 → 16.002 s | 10.453 s | 229,284 KiB |
| `.moth` | 1.062 → 0.707 s | 134.971 → 30.059 s | 10.857 s | 228,528 KiB |
| `.pur` | 0.731 → 0.427 s | 32.541 → 11.249 s | 1.380 s | 225,112 KiB |

Parser code did not change; its variation reflects cache/workstation noise.
Storage results include source copies and hashing, not only time inside `fsync`.
Bulk decode overlaps storage writes; its short waits do not imply negligible
CPU decode work. Peak RSS is from `wait4`, measured only for after runs.

Baseline `.moth` and `.pur` processes were stopped after their metric line while
performing an excluded, unnecessary temporary save. The measured load had already
completed. The final harness saves only on explicit request.

The remaining cost is substantial on this filesystem: all board metadata is
returned in one durable completion, so the first preview still waits for board
staging. These results do not close the <300 ms first-frame / <2 s full-detail
performance targets. Progressive visible-first loading and journal-backed source
ranges would need separate design and crash-recovery qualification.

The private PureRef input still hashes to
`92500ddecb2377743e2e2ddc22db2841c5c86eae1f6416dbfe107eb804cb29f2`,
matching the pre-existing qualification record. No source board was modified.

## Validation

- Regression: `cargo test -p moth-platform foreign_import_stages_each_original_once`
  failed before the fix (two working copies for one original), then passed. It
  deletes all working copies and reopens the journal, validating recovered bytes.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including the existing `dhat` allocation
  assertions, scene properties, truncation tests and headless golden images.
- `cargo xtask corpus`: passed, 28 image/board fixtures.
- Disk-backed `cargo xtask killtest 50`: passed, no acknowledged commands lost.
- Disk-backed `cargo xtask load-killtest 50`: passed against the real storage
  worker while importing distinct originals; it checks every acknowledged image
  and its recovered original, not only the recovered count.
- `bee_reader` / `pur_reader` fuzz, 30 seconds each: passed, 558,021 / 74,321
  executions. Parser implementations and on-disk formats are unchanged.

- `cargo xtask bench --quick`: passed, 10,000 distinct images / 100 frames,
  p99 4.785 ms, 13.32 MiB textures (RX 5700 XT / RADV).
- Disk-backed `cargo xtask soak 300 5`: passed, 1972 frames. RSS started at
  195,384 KiB and held at 195,860 KiB from 70 through 295 seconds.

Linux validation exercises storage/decode workers and headless rendering. Native
window presentation/file-picker timing was not manually measured. Windows/macOS
native workflows and physical power-loss testing were not run. Tier-3 release
checks remain with CI/the maintainer; none ran in this session.
