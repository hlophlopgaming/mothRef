# 0024 — Snapshot originals into the recovery journal when supported

## Context

On the measured 77 MB board, repeated full-resolution preparation was limited by
rewriting originals into a new journal, despite cached GPU-ready images. Four
loading-batch syncs consumed about 187–265 ms. A controlled comparison of one and
four sync barriers showed no reliable improvement: the same bytes still needed
to be written. Reading from the user's mutable file instead would weaken recovery
and invalidate original handles after external edits or compaction.

## Decision

For normal full-resolution opens, the desktop caller requests `OpenSnapshot`.
On Linux, try `FICLONE` through the existing rustix dependency to create a private
temporary file in the recovery directory. This is a copy-on-write snapshot, not a
hard link. The source and journal remain independent under changes to either.
The operation is optional; unsupported filesystems retain ordinary copying.

Validate the private snapshot with the existing reader before adopting it.
The normal source reader remains the fallback. Only healthy current-schema
files with no existing session journal qualify.
Keep the prefix through the last live original's valid footer, replace the old
layout tail with a current checkpoint, and reject a result larger than the normal
rebuilt journal. Sync the file, rename it to the private journal, and sync both
the session directory and its parent before publishing any loading result.

Register original handles only after adoption, using their unchanged offsets.
Scene construction still applies insertion-ordered Commands. Do not append those
inserts again: their complete scene already exists in the synced checkpoint.
The existing bounded progress messages and journal-first mutation path remain.

## Consequences

- No new board schema, cache, user preference or UI control. This is selected by
  the existing Settings › Performance mode; old formats still upgrade on save
  through the existing path.
- First-result latency can increase because validating a new snapshot may read
  uncached data. Only full-resolution mode opts in, since it waits for all image
  detail. Viewport modes retain their early progressive previews.
- Old/damaged files, backup/recovery opens, unsupported filesystems and files with
  excessive retained history fall back without weakening durability.
- Snapshot trimming happens before handles are registered; live session journals
  are never replaced, preserving decision 0011's range-handle invariant.
- The Linux branch is qualified on Btrfs. Windows, macOS and Android retain their
  existing copy path; native qualification on those systems remains separate.

Kernel contract: [FICLONE](https://www.man7.org/linux/man-pages/man2/FICLONE.2const.html)
documents independent copy-on-write contents, same-filesystem restrictions and
atomic cloning with respect to concurrent writes.

## Update 2026-11-23: copy where cloning is unavailable

Where `FICLONE` fails, and on every other platform, the private snapshot is now an
operating-system copy (`std::fs::copy`: `copy_file_range` on Linux, `clonefile` on
APFS, `CopyFileExW` on Windows, which clones on ReFS and Dev Drives). The copy is made
private and writable, then validated and trimmed exactly like a clone; any failure
falls back to staging originals. This replaces reading, hashing and appending every
original a second time. On a 173 MB board without reflinks, the storage stage of an
opening went from 490–530 ms to 270–430 ms; the first preview arrives after the copy
rather than with the first original, which the full-resolution mode hides behind its
progress bar.
