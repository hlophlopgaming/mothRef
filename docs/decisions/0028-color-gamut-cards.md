# 0028 — Color gamut cards

## Context

The maintainer wants BeeRef's "Show color gamut", "pretty and above all fast",
viewable in a window and kept on the board as its own element with an arrow to the
image (2026-11-18). From mock-ups the maintainer chose a Gurney-style gamut mask and
asked for three views: gamut windows (5a), painter's cells (5d), and the mask with
hue shares and marked main colors (5c + 5e), with the main colors sortable (share,
warm to cold, light to dark, hue, saturation).

## Decision

- A new item kind, `Content::Gamut`, stored with schema 9 tag 22 (FORMAT.md). It
  holds no pixels: a 72 × 20 hue/saturation coverage grid, up to eight main colors
  with their shares, the view, the order and the id of the image it came from.
  The card is self-contained, so it keeps its colors when the image is removed; the
  image link is weak (not in `Item::references`) and only draws the arrow.
- Measuring decodes a preview of at most 128 px on the storage worker
  (`moth_io::gamut`), so it takes milliseconds for any image and never touches the
  UI thread. It samples what the frame shows, after crop, rotation and flip.
- The card records a basis: a fingerprint of the original, the frame's proportions
  and the content transform. When the image's basis differs (it was cropped,
  flipped or reframed), the card shows "Image changed"; "Update color gamut" (and
  `gamut --id CARD`) measures again, keeping the card's place, view and order.
  Moving, proportional resizing and opacity do not count as a change.
- The wheel is drawn by `moth_ui::gamut` with egui meshes, like notes in screen space
  above images (the same shortcut notes take). The Color gamut panel shows the views
  and orders and puts a card on the board; a selected card switches view and order
  from its selection bar, journaled as Remove + Insert.
- The CLI gains `gamut --id IMAGE [--view] [--sort] [--at]`.

## Consequences

Gamut cards are not drawn in PNG/SVG/PDF scene exports yet, and like notes they
always sit above images. The out-of-date badge is not clickable: updating runs from
the card's selection bar or context menu.
