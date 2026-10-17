// SPDX-License-Identifier: GPL-3.0-or-later
//! Normalize and arrange for a selection: pure geometry that emits reversible commands.
//!
//! The first item (z-sorted, like the rest of the selection menu) is the reference:
//! normalize matches its frame along one measure, arrange anchors the layout at its top-left.
use moth_scene::{Command, Item, Rect};

/// Gap between arranged frames, in world units.
/// shortcut: fixed gap until a settings store lands (M2); the value matches the pre-existing menu.
pub const GAP: f32 = 24.0;

/// Which frame measure to match against the reference item.
#[derive(Clone, Copy)]
pub enum Normalize {
    /// Same width; height follows each item's aspect ratio.
    Width,
    /// Same height; width follows each item's aspect ratio.
    Height,
    /// Same width and height, stretching to match (distorts aspect ratio).
    Size,
    /// Same area, preserving each item's aspect ratio.
    Area,
}

/// How to lay items out relative to the reference item's anchor.
#[derive(Clone, Copy)]
pub enum Arrange {
    /// One row, left to right.
    Horizontal,
    /// One column, top to bottom.
    Vertical,
    /// Rows of roughly the square root of the item count.
    Grid,
    /// Shelf packing that wraps to fit the viewport width.
    Optimal,
}

/// Scales every frame to match the reference along `mode`, keeping each top-left anchor.
pub fn normalize(items: &[&Item], mode: Normalize) -> Vec<Command> {
    let Some(reference) = items.first().map(|item| item.rect) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let rect = match mode {
                Normalize::Width => Rect {
                    width: reference.width,
                    height: item.rect.height * reference.width / item.rect.width,
                    ..item.rect
                },
                Normalize::Height => Rect {
                    width: reference.height * item.rect.width / item.rect.height,
                    height: reference.height,
                    ..item.rect
                },
                Normalize::Size => Rect {
                    width: reference.width,
                    height: reference.height,
                    ..item.rect
                },
                Normalize::Area => {
                    let ratio = item.rect.width / item.rect.height;
                    let width = (reference.width * reference.height * ratio).sqrt();
                    Rect {
                        width,
                        height: width / ratio,
                        ..item.rect
                    }
                }
            };
            rect.valid()
                .then_some(Command::SetRect { id: item.id, rect })
        })
        .collect()
}

/// Repositions frames starting at the reference anchor, keeping each frame's size.
pub fn arrange(items: &[&Item], mode: Arrange, target_width: f32) -> Vec<Command> {
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let start = (first.rect.x, first.rect.y);
    let mut commands = Vec::with_capacity(items.len());
    match mode {
        Arrange::Horizontal => flow(items, usize::MAX, start, &mut commands),
        Arrange::Vertical => flow(items, 1, start, &mut commands),
        Arrange::Grid => flow(items, grid_columns(items.len()), start, &mut commands),
        Arrange::Optimal => shelf(items, target_width, start, &mut commands),
    }
    commands
}

/// Places items in rows of `columns`, advancing each row by its tallest frame.
fn flow(items: &[&Item], columns: usize, start: (f32, f32), commands: &mut Vec<Command>) {
    let (mut x, mut y) = start;
    let mut row_height = 0.0;
    for (index, item) in items.iter().enumerate() {
        if index % columns == 0 && index > 0 {
            x = start.0;
            y += row_height + GAP;
            row_height = 0.0;
        }
        push(commands, item, Rect { x, y, ..item.rect });
        x += item.rect.width + GAP;
        row_height = row_height.max(item.rect.height);
    }
}

/// Greedy shelf packing that wraps when a shelf would exceed `target_width`.
fn shelf(items: &[&Item], target_width: f32, start: (f32, f32), commands: &mut Vec<Command>) {
    let width = target_width.max(GAP);
    let (mut x, mut y) = start;
    let mut shelf_height = 0.0;
    for item in items {
        if x > start.0 && x + item.rect.width > start.0 + width {
            x = start.0;
            y += shelf_height + GAP;
            shelf_height = 0.0;
        }
        push(commands, item, Rect { x, y, ..item.rect });
        x += item.rect.width + GAP;
        shelf_height = shelf_height.max(item.rect.height);
    }
}

/// A square-ish layout uses the ceiling of the square root as the column count.
fn grid_columns(len: usize) -> usize {
    (len as f32).sqrt().ceil().max(1.0) as usize
}

fn push(commands: &mut Vec<Command>, item: &Item, rect: Rect) {
    if rect.valid() {
        commands.push(Command::SetRect { id: item.id, rect });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::Scene;

    fn blob(n: u8) -> moth_scene::BlobId {
        moth_scene::BlobId::from_bytes([n; 32])
    }
    fn item(n: u8, w: f32, h: f32) -> Item {
        Item::image(
            blob(n),
            Rect {
                x: 0.,
                y: 0.,
                width: w,
                height: h,
            },
            None,
        )
    }
    fn rects(commands: &[Command]) -> Vec<Rect> {
        commands
            .iter()
            .map(|c| match c {
                Command::SetRect { rect, .. } => *rect,
                _ => panic!("unexpected command"),
            })
            .collect()
    }

    #[test]
    fn normalize_width_equalizes_width_and_keeps_aspect() {
        let a = item(1, 100., 50.);
        let b = item(2, 200., 400.);
        let items = [&a, &b];
        let out = rects(&normalize(&items, Normalize::Width));
        assert_eq!(out[0].width, 100.);
        assert_eq!(out[1].width, 100.);
        // aspect ratio preserved: h = w * (orig_h / orig_w)
        assert!((out[1].height - 100. * 400. / 200.).abs() < 1e-3);
        assert!((out[0].height - 50.).abs() < 1e-3);
    }

    #[test]
    fn normalize_area_equalizes_area() {
        let a = item(1, 100., 50.);
        let b = item(2, 20., 20.);
        let items = [&a, &b];
        let out = rects(&normalize(&items, Normalize::Area));
        let target = 100. * 50.;
        for rect in &out {
            assert!((rect.width * rect.height - target).abs() < 1e-1);
        }
    }

    #[test]
    fn arrange_vertical_stacks_and_horizontal_lines_up() {
        let a = item(1, 100., 50.);
        let b = item(2, 80., 40.);
        let items = [&a, &b];
        let vertical = rects(&arrange(&items, Arrange::Vertical, 1000.));
        assert_eq!(vertical[1].y, vertical[0].y + 50. + GAP);
        assert_eq!(vertical[1].x, vertical[0].x);
        let horizontal = rects(&arrange(&items, Arrange::Horizontal, 1000.));
        assert_eq!(horizontal[1].x, horizontal[0].x + 100. + GAP);
        assert_eq!(horizontal[1].y, horizontal[0].y);
    }

    #[test]
    fn arrange_grid_wraps_after_sqrt_columns() {
        let items: Vec<_> = (0..5).map(|n| item(n, 10., 10.)).collect();
        let refs: Vec<&Item> = items.iter().collect();
        let out = rects(&arrange(&refs, Arrange::Grid, 1000.));
        assert_eq!(grid_columns(5), 3);
        // row 2 starts at the reference x, one row down.
        assert_eq!(out[3].x, out[0].x);
        assert_eq!(out[3].y, out[0].y + 10. + GAP);
    }

    #[test]
    fn commands_apply_and_undo_to_identity() {
        let items: Vec<_> = (0..4).map(|n| item(n, 10. * n as f32 + 10., 20.)).collect();
        let refs: Vec<&Item> = items.iter().collect();
        let mut scene = Scene::default();
        for item in &items {
            scene.apply(Command::Insert(item.clone())).unwrap();
        }
        let mut commands = normalize(&refs, Normalize::Area);
        commands.extend(arrange(&refs, Arrange::Grid, 1000.));
        let mut undo = Vec::new();
        for command in commands {
            undo.push(scene.apply(command).unwrap());
        }
        for command in undo.into_iter().rev() {
            scene.apply(command).unwrap();
        }
        for item in &items {
            assert_eq!(scene.get(item.id).unwrap().rect, item.rect);
        }
    }
}
