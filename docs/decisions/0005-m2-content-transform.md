# M2: content transforms and schema 2

## Context

BeeRef parity needs scale, rotate, flip and crop, including "transform image content
inside its crop frame" (BeeRef issue #140). `.bee` files already carry crop, scale,
flip and rotation, and the importer currently reports them as skipped. The scene
holds no pixels, so a transform must be expressible without knowing an image's size,
and it must survive a crash: it reaches disk before the UI shows it.

## Decision

An item keeps its `rect` as the crop frame on the canvas and gains a five-field
`Transform`: `rotation`, `flip`, `content_offset`, `content_scale`,
`content_rotation`. Content is drawn as a quad mapped by the content fields and then
rotated by `rotation`; the frame clips it. Because content never leaves the frame, the
item's visible footprint is the rotated frame, so the R-tree entry stores `rect` plus
`rotation` and returns the rotated axis-aligned envelope. All gestures, including a
crop drag, emit one `Command::SetTransform` or `Command::SetRect`, so undo, the
journal and future sync keep one mutation path.

This is schema 2: tag `4` inserts carry a transform, tag `5` is `SetTransform`, and
the reader keeps schema 1 tags. The header version is bumped to 2 but readers accept
1 and 2, so existing boards and journals load with the identity transform. Details are
in [FORMAT.md](../FORMAT.md).

`content_scale` is uniform and 1.0 fills the frame without distortion, clipping what
does not fit; a frame larger than the content magnifies it rather than leaving gaps.
The renderer derives the content-to-frame mapping from the frame size and
the image's pixel size, both of which it already knows. That keeps the scene pixel-free
and reproduces BeeRef crops exactly, because the importer sizes the frame from the same
crop it stored.

## Consequences

A transform costs 21 bytes per item in records and checkpoints; identity transforms are
not elided, keeping one insert path in the writer. Crop, zoom and rotation inside the
frame change no indexed bounds, so those gestures skip the R-tree entirely. A future
need for non-uniform scaling or perspective would change the meaning of these five
numbers and therefore needs schema 3 with its own migration. Clipping is expected to be
a fragment-shader discard, which the software fallback adapter must support; verifying
that, the rotated-rectangle hit test and `.bee` mapping are tracked by the M2
parity item.
