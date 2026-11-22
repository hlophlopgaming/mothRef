# 0029 — Automatic source groups

## Context

Grouping loose images by folder should preserve the existing hierarchy. Future
imports should find the same automatic group after the board closes or the
group is renamed. Schema 9 already records the file or archive source of an image.

## Decision

Derive folder identity from the stored image source. Save that identity on the
automatic group in schema 10, with a group insert envelope using tag 23.
The context menu, command palette, optional grouping on addition and native CLI
use the same command planner.

## Consequences

Schema 9 boards retain their image sources and can use grouping immediately.
Existing groups stay as they are until the user moves or ungroups their
members. A renamed automatic group still accepts images from its source folder.
The folder is stored as metadata and can be visible to anyone given the board.
