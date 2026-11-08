# 0022 — Library folders are the app's own, not folders on disk

## Context

The library knew only the last twenty boards plus pins. Its "stacks" were the
names of the folders the board files happened to live in, and the roadmap
planned to make stacks real folders on disk (drop a board on a board to create
one). The maintainer asked instead (2026-11-08) for a library file that
remembers board paths, and for folders made in the app that group boards
while the files stay where they are: two ways of finding boards that do not
exclude each other. One level of folders is enough for now, and the stacks
taken from folders on disk go away.

## Decision

- `moth-platform::library` keeps `library.json` in the application data
  directory: every board the user opened or added (path, pin, tags, folder
  membership, when it was last seen opening) and the app folders (id, name,
  color index into the theme's group colors). It is read and written only by
  the library worker, through a temporary file and an atomic rename, with a
  4 MiB budget and at most 5,000 boards and 200 folders.
- The recent list (`recent.txt`, written by the board worker) stays as it is
  and is shown as its own "Recent" entry; each refresh brings its boards into
  the library file.
- A folder is a list of paths. A board can be in several folders; deleting a
  folder deletes only the grouping. Board files are never moved, copied or
  changed by the library.
- A board whose file is gone stays in the library marked "File not found";
  "Find…" points the record at the new path and keeps its pin, tags and folders.
- On first start the library file is built from `recent.txt`, `pins.txt` and
  the per-board tag files, which are then no longer written.
- A library file that cannot be read is never overwritten during that session,
  so folders the user made are not lost to a parsing bug.

## Consequences

- "Stacks are real folders on disk" in ROADMAP M5 is replaced by app folders.
  "Show in folder" still leads to where the file lives.
- Paths are stored as text; a path that is not valid UTF-8 or contains a line
  break is not remembered (as before with the recent list and pins).
- Cards are still inspected on each refresh, bounded to 300 boards with full
  previews for the 48 most recent. The cached index (metadata and previews kept
  between refreshes, the 1,000-board / 200 ms target) remains open in M5.
