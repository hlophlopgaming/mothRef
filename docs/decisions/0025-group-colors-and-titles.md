# 0025 — Group RGB colors and nested titles

## Context

The maintainer requests automatic group colors, exact RGB choice and visible
preset swatches. Nested title chips overlap when world-space header gaps shrink
below the constant screen-space text height. Pack and fit ignores selection.

## Decision

- Extend decision 0021: group color is either one of six theme palette codes or
  a tagged exact sRGB value (`0x01RRGGBB`). Schema 8 adds tags 19/20; existing
  palette records stay readable. Saves migrate older files by atomic compaction.
- New groups cycle through the five colored presets, starting with sage.
- The group selection bar and context menu keep the ordered color action list,
  show swatch previews and add Custom RGB. That action opens an RGB picker with
  numeric channels, Apply and Cancel. Applying uses SetGroup and the journal;
  cancellation leaves the scene untouched. No new settings are needed.
- Title chips retain readable screen-space text. Place descendants first and
  move colliding chips upward; hit regions and the inline editor use the same
  placement. This changes presentation, never board geometry.
- Pack and fit uses selected non-group members, or all non-group items only
  when selection is empty. Fit runs after the layout batch leaves the journal
  queue. An empty selected group never expands the operation to the board.

## Consequences

Custom RGB follows the board across themes, undo, recovery and exports. Palette
colors still adapt to the theme. Applications supporting only schemas 1–7 cannot
open newly saved boards. At very small zoom levels title chips may extend above
their frames to keep the nested hierarchy readable.
