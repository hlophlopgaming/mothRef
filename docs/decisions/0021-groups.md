# 0021 — Groups

## Context

M4 asks for PureRef-style groups (DESIGN.md screen 14): a tinted backdrop
with a title that hugs its content, a click that selects the whole group, a
double-click that steps inside, dropping an image on a backdrop to add it, and
a hierarchy tree. The maintainer chose nested groups from the start and a
backdrop that always fits its members.

The scene holds a flat set of items, and every change is a `Command` with an
exact inverse that the journal, undo and later sync share.

## Decision

- A group is an item: `Content::Group { title, color }`. It gets an id,
  z-order and a place in the spatial index like images and notes, so culling,
  hit testing, the journal and undo need no second structure.
- Every item has `parent: Option<Uuid>`. Two new commands: `SetParent` (with
  the old parent as inverse) and `SetGroup` (title and color). Nesting is
  limited to 16 levels and cycles are rejected, so a hostile file cannot make
  each edit walk a long chain.
- A group's frame is **derived inside the scene**: the union of its members'
  rotated footprints plus padding (24 world units, 36 on top for the title
  chip). An empty group shows a resting frame, the one stored in its insert
  and changed by `SetRect`. Because the frame is a pure function of the
  members and the resting frame, every inverse stays exact without commands
  carrying backdrop geometry. Hulls update incrementally; only a member that
  touched the hull edge forces a rescan of its siblings.
- A group can only be removed once empty. UI batches order commands so that
  holds: members before groups on delete, groups before members on insert and
  in checkpoints.
- Colors are palette codes (six), mapped to light and dark theme tokens. The
  file never stores a literal color.
- Selection stays a set of ids. Selecting a group adds its members, so moves,
  deletes, duplicates and resizes reuse the existing code paths; actions that
  make no sense on a frame (rotate, flip, arrange, align) skip groups.
- The renderer draws backdrops with a second, texture-less pipeline in the
  same z-ordered batches as images. The app moves each backdrop just before
  its first member in the draw list, so a member below its group in z-order is
  never tinted. Title chips are UI and drawn by egui.
- `.moth` schema 6 (see FORMAT.md "Migration 5 → 6"): new insert tags with a
  parent, a group insert, `SetParent` and `SetGroup`. Saving a schema 5 file
  compacts it to schema 6 first.

## Consequences

- Older MothRef versions reject schema 6 boards, including boards without
  groups once saved by this version.
- SVG/PNG/PDF exports draw backdrops and titles in the dark-theme swatches; the
  `.pur` exporter still writes members without their groups.
- JSON Canvas group nodes (M12) map directly onto this model.
