# Library albums

Status: design agreed; implementation has not started.

## Accepted by the maintainer — 2026-11-10

- Show albums in the library grid as stacks of previews, with a name and board count. Opening an album shows its boards and a Back control.
- Clarified after questions 3–11: a board belongs to at most one album, or lives in the root gallery. Moving it to an album removes its previous membership without copying or moving the board file on disk. This replaces the earlier multi-album interpretation.
- Add a Select mode for moving one or several boards to the system trash, with a confirmation showing the number of boards.
- Deleting an album removes only the grouping and preserves its boards.
- Create albums with New album; move selected boards through Move to album or drag a board onto an album card. Dropping a board onto its current album does nothing and never creates another entry.
- The root gallery shows albums and boards without album membership. All boards remains available as a separate view.
- If any selected board is open, block the entire trash request before sending files to the worker, identify the open boards, and retain the selection so the user can close them and retry.
- Album covers automatically show a stack of board previews and update with membership; empty albums show a placeholder and their name.
- Search inside an album covers only that album's boards. Navigating to another album or library section clears the board selection.
- Multi-board trash requests may partially succeed: report successes and failures, remove successfully trashed boards from the library and their album, and retain failed boards in the selection for retry.
- The maintainer delegated the legacy multiple-folder case. Keep the first valid existing membership in its stored order, preserving board files. Before persisting the normalized library, preserve the original library index as a recovery copy; if that fails, leave the original index untouched and report the error. Do not ask the user to resolve old memberships individually.

Album is the user-facing term. Exclusive album membership revisits the multiple-folder membership in decision 0022. Legacy membership normalization affects the library index only; the board format is unchanged.

## Additional requested interaction

- Include creating an album by dropping one board onto another on Library home in the first version. Both boards move into the new album. Inside an existing album this action does not create another album or a duplicate board.

## Confirmation

The maintainer accepted questions 3–11, then confirmed exclusive album membership, Library home contents and board-on-board creation on Library home only. The maintainer delegated the legacy membership choice; the deterministic recovery-backed rule above completes the scope.

## Existing constraints

- Keep bounded preview requests and texture residency; the current cap is 64 cards with previews.
- LibraryScreen owns card and preview bookkeeping; App coordinates board opening and window lifecycle.
- Filesystem work stays on workers. Board deletion uses the system trash.
- Preserve library-file recovery protections and apply the legacy compatibility rule above on the library worker.
- UI commands use registered actions; controls live in the library header, album cards and card menus. No new setting is currently needed.
- Add regression coverage for state transitions. Execute tests only when explicitly requested or preparing a release, per AGENTS.md section 3.
