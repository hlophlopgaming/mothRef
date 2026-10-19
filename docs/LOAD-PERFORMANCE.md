# Bulk import and board-open performance — 2026-10-18

The measurements below describe the October 18 working-copy implementation.
The [storage follow-up](#journal-range-follow-up--2026-10-19) removes those copies;
the [combined result](#combined-optimizations--2026-10-19) includes progressive
loading and footer-chain reading.

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

- Regression (then named `foreign_import_stages_each_original_once`)
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

## Journal-range follow-up — 2026-10-19

This subsection records the intermediate storage-only implementation and its
checks, before the progressive-loading/reader changes described below.

Originals now live only in the session journal after storage acknowledgment, with
bounded read-only storage handles into each payload (decision 0011). Opening
`.moth` reads live originals in source-offset order, verifies their hashes and
appends them directly to the new journal. Foreign import journals each unique
original during parsing, instead of creating a `.source` file and rereading it.
Individual inserts use the same journal-range path. Clipboard staging retains its
separate pre-insert durability boundary. The opened input file remains untouched.

Every range opens an independent reader. Its length and seek offsets are relative
to the original payload, reads cannot include adjacent records, and writable
storage operations reject range handles. Board saves and compaction do not replace
the session journal. The final completion still follows command/checkpoint sync;
there is no progressive scene delivery or weakened checksum verification.

### Comparison setup

Compare the pre-change release `xtask` from `b55d29c` with the changed release
binary, alternating before/after three times per input. Use the existing `load`
harness, a disk-backed `TMPDIR` under `target/load-profile`, and no concurrent
build, test, fuzzer or benchmark. Report medians of the three runs. Inputs are
warm; these are interactive-workstation samples, not calibrated release numbers.
As above, first image excludes the parse-only warm-up probe, dialogs and native
window presentation, and all previews does not mean full-resolution image detail.

- `hardPics.moth`: 76,942,327 bytes, 29 unique image originals. Read-only measurement
  of the maintainer's existing working file; no image or board is redistributed.
- Generated 1000-image `.bee`: 686,178,304 bytes, using the same JPEG/counter recipe
  above and table schemas from `images/beerefTestFile.bee`.
- Generated `.moth`: converted from that `.bee` with the pre-change storage worker;
  685,543,548 bytes. Both versions read the identical file. Fixture generation/save
  is outside timing.
- Per-process RSS and output-block counts come from `wait4`; output blocks are
  kernel-accounted 512-byte blocks, not physical SSD writes or Btrfs allocation.

These follow-up runs used Linux 6.18.49-2-lts, Rust 1.99.0 and the software
llvmpipe adapter (LLVM 22.1.8), including both builds' load comparisons. They are
not directly comparable to the earlier RX 5700 XT/RADV results above.

### Results

| Input | First image before → after | All previews before → after | Storage worker before → after |
| --- | --- | --- | --- |
| 29-image `hardPics.moth` | 0.622 → 0.521 s | 0.929 → 0.856 s | 0.570 → 0.481 s |
| 1000-image `.moth` | 13.543 → 7.998 s | 26.191 → 19.120 s | 13.486 → 7.947 s |
| 1000-image `.bee` | 8.778 → 7.066 s | 20.933 → 19.441 s | 8.717 → 7.001 s |

| Input | Median output blocks before → after | Median process peak RSS before → after |
| --- | --- | --- |
| `hardPics.moth` | 301,408 → 150,864 | 473,112 → 467,080 KiB |
| 1000-image `.moth` | 2,687,424 → 1,340,832 | 582,632 → 607,572 KiB |
| 1000-image `.bee` | 2,687,600 → 1,341,104 | 605,252 → 607,264 KiB |

Journal-range loading roughly halves kernel-accounted output. The first-image
median improves by about 41% for the large `.moth`, 20% for `.bee` and 16% for
`hardPics.moth`. Large-board decode waits remain roughly 8–9 seconds in this
harness; this change does not optimize the decoder or overlap it with board staging.
Peak RSS is the whole headless process, including the software renderer; these
samples do not establish a native-app memory reduction.

Timing spread is substantial. First-image ranges for before/after are
0.562–1.249 / 0.491–0.722 s (`hardPics.moth`),
13.243–14.893 / 3.499–10.630 s (large `.moth`), and
4.030–8.857 / 2.914–7.893 s (large `.bee`). Raw per-run measurements were saved
locally to `/tmp/mothref-load-comparison.json`; those temporary files are not
distributed or required to build/test the project.

### Safety and platform coverage

The new open regression first failed on the extra `.source` file. It now checks
journal-only storage, hash-identical originals after compacting and deleting the
opened test input, original export, save/reload, and opening the recovery journal.
Foreign-import regressions check journal-only originals and recovery for captured
BeeRef and PureRef files. Range tests check independent cursors, boundary reads,
invalid seeks, truncation, offset overflow and refusal of writable operations.

Workspace formatting, workspace Clippy with warnings denied, workspace tests
(including existing decoder/render allocation checks and GPU goldens), and all
28 corpus fixtures passed. After strengthening the input-deletion regression,
that test and workspace Clippy were rerun successfully. No dependency changed.
Disk-backed `killtest 50` and `load-killtest 50` passed without losing acknowledged
commands. BeeRef/PureRef fuzz runs completed 1,152,822 / 79,704 executions in
31 seconds each. The first sandboxed BeeRef run could not finish LeakSanitizer's
exit check because ptrace was denied; both successful runs used the normal
sanitizers outside that sandbox.

The foreground quick benchmark passed on llvmpipe: 10,000 distinct images,
100 frames, 13.32 MiB textures, p99 23.602 ms (pre-change sample: 27.371 ms).
These short software-adapter samples check for regression; they do not establish
a renderer speedup or qualify the native-GPU frame-time target.

Native Windows/macOS workflows, native first-window timing, physical power loss
and a large real PureRef board were not tested in this follow-up. Tier-3 release
qualification remains CI/maintainer work. At this intermediate stage, progressive
delivery and the reader were unchanged. The combined implementation below removes
the first-preview wait for the final journal sync.

## Combined optimizations — 2026-10-19

All three changes are implemented together:

1. Original bytes are decoded/exported/saved through bounded journal ranges, without
   separate `.source` copies.
2. The first durable item and subsequent bounded batches reach the UI before the
   final completion. Visible decoding overlaps the remaining storage work, for
   `.moth`, `.bee` and `.pur`. Editing/save are gated while loading; errors retain
   durable partial items under an unsaved destination. Foreign stacking order is
   corrected before completion, with stable UUIDs and one import undo step.
3. Healthy `.moth` records are located through their adjacent footer chain, avoiding
   payload byte scanning. All checksums and metadata validation remain; damaged,
   gapped or excessively long chains fall back to the original scanner.

Per the maintainer's subsequent instruction, stop further test/stress runs and
compare the combined implementation only on the existing `hardPics.moth`. The
in-progress soak was interrupted at 205 seconds (RSS 429,452 KiB); this is not a
completed five-minute soak. Earlier workspace/corpus/fuzz/kill checks above apply
only to the storage stage. The final combined app/tool release build passed;
the new progressive and footer-chain regressions were added but not executed.

One old-build run and one combined-build run, sequentially with disk-backed
`TMPDIR`, no other build/fuzzer/benchmark running, and the same llvmpipe adapter:

| Metric | Before | All three optimizations |
| --- | --- | --- |
| Parse-only probe (excluded from load timers) | 138.163 ms | 27.153 ms |
| First drawn image | 629.332 ms | 89.521 ms |
| Final durable completion observed | 563.028 ms | 433.070 ms |
| All 29 previews drawn | 1036.250 ms | 668.242 ms |

The first preview appears before final durable completion, demonstrating actual
overlap. In this single comparison, first-preview latency falls by about 86%
(7.0× faster) and all-preview latency by 36% (1.55× faster). This does not establish
multi-run statistics, native-window latency or large-board performance. The new
harness drains intermediate messages and starts decoding immediately; the old
binary has its original final-completion-only behavior. Both count first image
only after its original/command are durably stored and its preview is drawn.

The input remains 76,942,327 bytes with SHA-256
`ad72037d68c0cb71d3be14aba2cc1bbea4821dbd59f805ee1801527810156e14`.
No source-board writes, format/schema changes or new dependencies were introduced.
Windows/macOS manual workflows and further automated/release qualification remain
unverified for the combined changes; see decision 0012.

## Decoder follow-up — 2026-10-19

The 29 originals in `hardPics.moth` are all PNG. A temporary sequential probe read
each original through its recorded blob range, then timed `moth_io::decode` at
256 px separately from file reads. Before this follow-up, decoding totalled
1150.003 ms. A separate PNG row-only pass totalled 814.114 ms: roughly 71% of
decode time was PNG decompression, unfiltering and row transformation. The
remaining time includes area averaging, hashing and mip construction; these are
single-pass wall-clock observations, not CPU-profile attribution or statistics.
An initial end-to-end run with default temporary storage observed storage completion
at 133.634 ms and all previews at 611.317 ms. That run is diagnostic only; the
disk-backed comparison below has different storage conditions.

The follow-up specializes area accumulation once per row for grayscale, RGB and
alpha-bearing input, with constant channel counts and opaque alpha values in the
inner loop. It accumulates each bin locally and avoids calling `thumbnail` when
the decoded image already fits the requested size. All source pixels still
contribute with the same integer alpha weighting. Adam7 mapping, dimensions,
orientation, checksums, panic isolation and mip filters remain unchanged. No
cache, extra allocation, dependency, worker or queue capacity was added. This
shared path also serves clipboard RGBA, GIF, BMP and admitted WebP previews.

A single sequential decode-only follow-up totalled 1073.880 ms (about 6.6% less).
The dominant PNG row-decoding work remains. This small sample does not establish
a general decoder speedup, and is not a full-board or native-window timing.

One sequential disk-backed pair compared the saved release executable containing
the three storage optimizations (`/tmp/mothref-load-pre-decode`) with the new
release executable. Both used the same progressive harness, requested all 29
256 px previews and displayed them on llvmpipe. No build or other benchmark ran
alongside the pair:

| Metric | Three storage optimizations | Plus decoder follow-up |
| --- | --- | --- |
| Parse-only probe | 26.888 ms | 26.489 ms |
| First drawn image | 130.971 ms | 240.095 ms |
| Final durable completion observed | 466.093 ms | 1130.690 ms |
| All 29 previews drawn | 663.326 ms | 1330.339 ms |
| Whole-process peak RSS | 471560 KiB | 472800 KiB |

**This pair does not demonstrate faster full loading.** The second run was slower,
including substantially later observed storage completion, despite unchanged
storage code. The harness does not isolate fsync time from scheduling and result
observation, so the cause cannot be established from this pair. No repeated runs
were made to replace the unfavorable result. RSS includes the software renderer
and is not an allocation-budget test. The source SHA-256 was checked before and
after and remains the value recorded above.

Validation was deliberately limited per the maintainer: 18 `moth-io` library
tests passed in 0.56 s, including a new scalar-reference test across color types,
area sizes and interlace strides; formatting and `moth-io --all-targets` Clippy
passed. `cargo build --release -p mothref -p xtask` passed. No workspace test run,
allocation instrumentation, fuzz, soak, kill test or benchmark suite was run in
this follow-up. Earlier progressive-reader regression tests remain unexecuted.

This improves the existing requested-resolution preview path, not just the load
harness's 256 px overview. The application still chooses resolution from visible
screen size, capped at `MAX_SIDE` (2048); full-resolution visible tiles remain a
separate unimplemented path. Neither measurement represents full original detail.
