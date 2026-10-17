# Reference workspace validation — 2026-10-17

The five supplied UI states are wired to native application actions: library,
board, empty board, overlay and dirty-close confirmation. This is the initial
image-board implementation, not completion of every feature pictured in the
reference or the M2–M5 roadmap.

## Checks

- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --workspace`: passed, including deferred fitting, isolated tab
  journals, destination leases, relative file replacement and card label layout.
- `cargo deny check`: passed (advisories, bans, licenses, sources).
- `cargo xtask bench --quick`: passed; 10,000 distinct images, 100 frames,
  p99 3.195 ms, 13.32 MiB textures, AMD RX 5700 XT / RADV.
- `cargo xtask corpus`: passed, 21 image and board fixtures.
- `cargo xtask killtest`: passed, 1,000 process kills with no acknowledged
  commands lost.
- Headless golden checks cover software rendering, dot-grid layering and master
  opacity over overlapping images; registry clear releases tracked resources.
- A failing UI regression reproduced the card-label layout bug before the fix.

**At the maintainer's explicit request, the memory soak and both fuzz processes
were stopped. They are incomplete and are not qualification results.** Do not
restart them as part of this task. The importer-specific `bee_reader` and
`pur_reader` fuzz targets do not exist yet.

## Native inspection

Linux X11, RX 5700 XT, 1440×900: launched the release application with a separate
`XDG_DATA_HOME`, generated synthetic gradient fixtures and opened an actual
`.moth` board. Inspected library previews, the empty import prompt, image board,
fit, tool rail and minimap. Screenshots and command logs are in ignored
`target/ui-validation/`; no repository artwork was redistributed. The library
inspection found sideways/clipped card labels, now fixed with a regression test.

Earlier native overlay inspection covered the separate interactive controller.
Windows/macOS, actual touch devices and full cross-platform overlay behavior
remain unqualified. Wayland on-top is disabled through capability flags.

## Scope and limits

Four sessions; twenty recent plus twenty pinned catalog entries; four 128-pixel
previews per entry. The minimap samples at most 128 items. Arrangement commands
cover the whole selection and group into a single undo step. Folder
filters reflect parent folder names. Full folder management, tab tear-off,
index watchers, notes, hierarchy, crop and other later-milestone tools remain
roadmap work. The UI does not expose decorative controls for those features.
