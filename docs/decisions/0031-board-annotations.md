# 0031. Board annotations

## Context

Artists need to mark up references and keep short checklists beside them.
An annotation must survive recovery, move with groups, support undo and appear in exports. Existing boards can contain literal Markdown-like text.

## Decision

Persist a stroke as a bounded path in its own frame. The normal item commands
handle its lifetime, placement, opacity and orientation. The same path is drawn on
the canvas and exported as SVG geometry. The Pen, Marker and Arrow actions live
in the bottom toolbar. Color and thickness live in Settings › Canvas and navigation.

Keep a text item’s editable source, its presentation (label, note or list), and whether formatting is enabled.
New notes have formatting enabled. An existing plain note opts in when a user
chooses a formatting action in the inline editor or enables it through the CLI.
The regular text layout measures notes for both GUI and CLI edits. The canvas fits
text into older undersized rectangles without rewriting their geometry.

## Consequences

Schema 11 stores the stroke path and the note formatting bit. The reader keeps
the earlier schemas and upgrades on save through the existing atomic path.
Strokes drawn over reference images use the same UI layer that displays notes.
