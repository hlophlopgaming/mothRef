# 0009 — Bounded adaptive GPU image-cache budget

## Context

The maintainer requested growth with image count and shrinkage when images are
removed. The original fixed 96 MiB ceiling (decision 0001) limits detailed previews
on larger boards. Budgets must remain explicit and finite, including on software
adapters. Scene, board format and journal semantics must remain unchanged.

## Decision

Count image items on the active board, excluding notes. Maintain the count on
successful acknowledged commands; recount only when replacing an opened scene.
Allow 1 MiB per image, with a 32 MiB floor for a nonempty board. Cap at 512 MiB on
discrete GPUs and 96 MiB on integrated, virtual, software and unknown adapters.
These are conservative policy ceilings, not measurements of available VRAM.
An empty board has a zero-byte image cache. Pending work temporarily retains the
32 MiB floor so decoded imports can upload before their commands are acknowledged.

The limit covers image array pages and all their mips; surfaces, egui and the
transient overlay attachment are separate allocations. Raising the ceiling reserves
nothing. Lowering it immediately evicts LRU detail pages before coarse fallback
pages until actual allocation fits. A zero ceiling clears all image pages and
residency bookkeeping. The status bar shows allocated/current-limit MiB.

## Consequences

This supersedes the fixed 96 MiB image-cache policy in decision 0001. Allocation
remains bounded independently of board size, while large discrete-GPU boards can
retain more detail. Whole-page eviction may release more than the limit reduction.
Undo may raise the limit again, but previously evicted previews reload on demand.
The policy uses adapter class, not OS-specific VRAM polling, and adds no timers,
background repaint loop or new dependencies. Idle-machine performance and native
VRAM-release observations remain manual qualification steps.
