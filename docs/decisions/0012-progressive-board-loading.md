# 0012 — Progressive durable loading and footer-chain reads

Date: 2026-10-19

## Context

Removing duplicate originals reduces I/O, but a single final completion still
keeps the first preview behind all storage work. The maintainer requests all three
load optimizations together and one simple before/after file measurement, without
further long test or benchmark runs.

## Decision

Publish a `Loading` completion after syncing the first item, then after 64 commands
or 32 MiB of originals. Send commands and bounded original handles, never pixels.
Keep the existing bounded completion channel and wake the UI for each batch.
The UI applies acknowledged commands and its existing visible-only decoder starts
while the storage worker continues. An open/import remains in flight until its
final checkpoint/completion; editing and saving are blocked meanwhile.

`.moth` loading uses complete bounds from its metadata. Foreign imports initially
publish in stream order, then journal the necessary remove/insert commands to
restore the foreign z-order before completion. Item UUIDs and source bytes stay
unchanged, and one undo removes the imported items. On load errors, valid pending
items are synced/published when possible; the UI keeps the durable partial scene
and clears its destination to require Save As. Write failures still poison the
worker and never acknowledge unsynced work.

For healthy `.moth` files, index adjacent footers backwards and process payloads
forwards with the same checksum, budget and command validation. Limit the index
to one million offsets; use the existing scanner for larger histories, torn tails,
gaps and corruption. No on-disk representation or schema changes.

## Consequences

Decoding overlaps durable loading, without speculative scene mutations. Extra
batch syncs trade some storage throughput for earlier images. Foreign layering
is provisional during loading and corrected before editing is enabled. Future
consumers must handle `Loading` without decrementing outstanding request counts;
the app, load harness and qualification helper do so.

The final app/tool release build and a one-file before/after measurement qualify
this implementation locally. Earlier automated checks cover only the storage
stage; progressive loading and the new reader were not rerun through the test,
fuzz or stress suites, per the maintainer's explicit instruction. CI/release
qualification remains necessary.
