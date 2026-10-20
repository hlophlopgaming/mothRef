// SPDX-License-Identifier: GPL-3.0-or-later
//! Normalize and arrange for a selection: pure geometry that emits reversible commands.
//!
//! The first item (z-sorted, like the rest of the selection menu) is the reference:
//! normalize matches its frame along one measure, arrange anchors the layout at its top-left.
use moth_scene::{Command, Item, Rect};

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
    /// Packs displayed bounds into free rectangles within the viewport width.
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
pub fn arrange(items: &[&Item], mode: Arrange, target_width: f32, gap: f32) -> Vec<Command> {
    if !gap.is_finite() || !(0.0..=10000.0).contains(&gap) {
        return Vec::new();
    }
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let start = items.iter().fold((first.rect.x, first.rect.y), |a, i| {
        (a.0.min(i.rect.x), a.1.min(i.rect.y))
    });
    let mut commands = Vec::with_capacity(items.len());
    match mode {
        Arrange::Horizontal => flow(items, usize::MAX, start, gap, &mut commands),
        Arrange::Vertical => flow(items, 1, start, gap, &mut commands),
        Arrange::Grid => flow(items, grid_columns(items.len()), start, gap, &mut commands),
        Arrange::Optimal => pack(items, target_width, start, gap, &mut commands),
    }
    commands
}

/// Places items in rows of `columns`, advancing each row by its tallest frame.
fn flow(items: &[&Item], columns: usize, start: (f32, f32), gap: f32, commands: &mut Vec<Command>) {
    let (mut x, mut y) = start;
    let mut row_height = 0.0;
    for (index, item) in items.iter().enumerate() {
        if index % columns == 0 && index > 0 {
            x = start.0;
            y += row_height + gap;
            row_height = 0.0;
        }
        push(commands, item, Rect { x, y, ..item.rect });
        x += item.rect.width + gap;
        row_height = row_height.max(item.rect.height);
    }
}

/// Largest-first guillotine packing reuses holes instead of leaving whole shelves empty.
fn pack(
    items: &[&Item],
    target_width: f32,
    start: (f32, f32),
    gap: f32,
    commands: &mut Vec<Command>,
) {
    if !target_width.is_finite() || target_width <= 0. {
        return;
    }
    let bounds: Vec<_> = items
        .iter()
        .map(|i| {
            let e = i.rect.rotated_envelope(i.transform.rotation);
            Rect {
                x: e.lower()[0],
                y: e.lower()[1],
                width: e.upper()[0] - e.lower()[0],
                height: e.upper()[1] - e.lower()[1],
            }
        })
        .collect();
    let width = bounds.iter().fold(target_width, |w, r| w.max(r.width)) + gap;
    let mut free = vec![Rect {
        x: start.0,
        y: start.1,
        width,
        height: f32::INFINITY,
    }];
    let mut order: Vec<_> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        (bounds[b].width * bounds[b].height)
            .total_cmp(&(bounds[a].width * bounds[a].height))
            .then(a.cmp(&b))
    });
    let mut placed = vec![None; items.len()];
    for index in order {
        let b = bounds[index];
        let w = b.width + gap;
        let h = b.height + gap;
        let slot = free
            .iter()
            .enumerate()
            .filter(|(_, r)| r.width >= w && r.height >= h)
            .min_by(|(_, a), (_, b)| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)))
            .map(|(i, _)| i);
        let Some(slot) = slot else {
            return;
        };
        let r = free.swap_remove(slot);
        placed[index] = Some(Rect {
            x: items[index].rect.x + r.x - b.x,
            y: items[index].rect.y + r.y - b.y,
            ..items[index].rect
        });
        if r.width > w {
            free.push(Rect {
                x: r.x + w,
                y: r.y,
                width: r.width - w,
                height: h,
            });
        }
        if r.height > h {
            free.push(Rect {
                x: r.x,
                y: r.y + h,
                width: r.width,
                height: r.height - h,
            });
        }
    }
    if placed.iter().flatten().all(|r| r.valid()) {
        for (item, rect) in items.iter().zip(placed.into_iter().flatten()) {
            push(commands, item, rect);
        }
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

/// Frame alignment uses the visible rotated envelopes; stacking preserves z-order.
#[derive(Clone, Copy)]
pub enum Align {
    Left,
    Right,
    Top,
    Bottom,
    Horizontal,
    Vertical,
    Stack,
}
pub fn align(items: &[&Item], mode: Align) -> Vec<Command> {
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let envelopes: Vec<_> = items
        .iter()
        .map(|i| i.rect.rotated_envelope(i.transform.rotation))
        .collect();
    let min = envelopes.iter().fold([f32::INFINITY; 2], |a, r| {
        [a[0].min(r.lower()[0]), a[1].min(r.lower()[1])]
    });
    let max = envelopes.iter().fold([f32::NEG_INFINITY; 2], |a, r| {
        [a[0].max(r.upper()[0]), a[1].max(r.upper()[1])]
    });
    let axis = usize::from(matches!(mode, Align::Vertical));
    let mut order: Vec<_> = (0..items.len()).collect();
    order.sort_by(|a, b| {
        envelopes[*a].lower()[axis]
            .total_cmp(&envelopes[*b].lower()[axis])
            .then(items[*a].id.cmp(&items[*b].id))
    });
    let total: f32 = envelopes
        .iter()
        .map(|r| r.upper()[axis] - r.lower()[axis])
        .sum();
    let gap = (max[axis] - min[axis] - total) / items.len().saturating_sub(1).max(1) as f32;
    let mut cursor = min[axis];
    order
        .into_iter()
        .filter_map(|index| {
            let item = items[index];
            let e = envelopes[index];
            let mut r = item.rect;
            match mode {
                Align::Left => r.x += min[0] - e.lower()[0],
                Align::Right => r.x += max[0] - e.upper()[0],
                Align::Top => r.y += min[1] - e.lower()[1],
                Align::Bottom => r.y += max[1] - e.upper()[1],
                Align::Stack => {
                    r.x = first.rect.x + (first.rect.width - r.width) / 2.;
                    r.y = first.rect.y + (first.rect.height - r.height) / 2.;
                }
                Align::Horizontal | Align::Vertical => {
                    if axis == 0 {
                        r.x += cursor - e.lower()[0];
                    } else {
                        r.y += cursor - e.lower()[1];
                    }
                    cursor += e.upper()[axis] - e.lower()[axis] + gap;
                }
            }
            r.valid().then_some(Command::SetRect {
                id: item.id,
                rect: r,
            })
        })
        .collect()
}
/// Both pointer drags and nudges share the journaled geometry path.
pub fn translate(
    items: &[&Item],
    delta: [f32; 2],
    duplicate: bool,
    top: Option<&fractional_index::FractionalIndex>,
) -> Vec<Command> {
    if delta.iter().any(|v| !v.is_finite()) {
        return Vec::new();
    }
    let mut z = top.cloned();
    let mut out = Vec::new();
    for item in items {
        let rect = Rect {
            x: item.rect.x + delta[0],
            y: item.rect.y + delta[1],
            ..item.rect
        };
        if !rect.valid() {
            return Vec::new();
        }
        if duplicate {
            let mut copy = (*item).clone();
            copy.id = moth_scene::Uuid::now_v7();
            copy.rect = rect;
            copy.z = z
                .as_ref()
                .map(fractional_index::FractionalIndex::new_after)
                .unwrap_or_default();
            z = Some(copy.z.clone());
            out.push(Command::Insert(copy));
        } else {
            out.push(Command::SetRect { id: item.id, rect });
        }
    }
    out
}
/// Reuse remove/insert commands so layer changes need no new journal encoding.
pub fn reorder(
    scene: &moth_scene::Scene,
    selection: &std::collections::HashSet<moth_scene::Uuid>,
    front: bool,
    extreme: bool,
) -> Vec<Command> {
    use fractional_index::FractionalIndex as Z;
    let mut ordered: Vec<_> = scene.items().collect();
    ordered.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
    if extreme {
        ordered.sort_by_key(|i| selection.contains(&i.id) == front);
    } else if front {
        for index in (0..ordered.len().saturating_sub(1)).rev() {
            if selection.contains(&ordered[index].id) && !selection.contains(&ordered[index + 1].id)
            {
                ordered.swap(index, index + 1);
            }
        }
    } else {
        for index in 1..ordered.len() {
            if selection.contains(&ordered[index].id) && !selection.contains(&ordered[index - 1].id)
            {
                ordered.swap(index, index - 1);
            }
        }
    }
    let mut out = Vec::new();
    let mut previous: Option<Z> = None;
    let mut unselected = ordered
        .iter()
        .filter(|i| !selection.contains(&i.id))
        .peekable();
    for item in &ordered {
        if !selection.contains(&item.id) {
            previous = Some(item.z.clone());
            unselected.next();
            continue;
        }
        let next = unselected.peek().map(|i| &i.z);
        let z = match (previous.as_ref(), next) {
            (Some(a), Some(b)) => Z::new_between(a, b),
            (Some(a), None) => Some(Z::new_after(a)),
            (None, Some(b)) => Some(Z::new_before(b)),
            (None, None) => Some(Z::default()),
        };
        let Some(z) = z else {
            return Vec::new();
        };
        previous = Some(z.clone());
        if z != item.z {
            let mut changed = (*item).clone();
            changed.z = z;
            out.extend([Command::Remove(item.id), Command::Insert(changed)]);
        }
    }
    out
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

    proptest::proptest! {
        #[test]
        fn packing_keeps_displayed_bounds_disjoint(
            sizes in proptest::collection::vec((1f32..300., 1f32..300., -std::f32::consts::PI..std::f32::consts::PI), 1..40),
            gap in 0f32..30., width in 10f32..1000.
        ) {
            let items: Vec<_> = sizes.into_iter().enumerate().map(|(n,(w,h,rotation))| {
                let mut i=item(n as u8,w,h); i.transform.rotation=rotation; i
            }).collect();
            let refs: Vec<_> = items.iter().collect();
            let commands=arrange(&refs,Arrange::Optimal,width,gap);
            proptest::prop_assert_eq!(commands.len(),items.len());
            let rects=rects(&commands);
            let envelopes: Vec<_> = rects.iter().zip(&items).map(|(r,i)| {
                let e=r.rotated_envelope(i.transform.rotation);
                [e.lower()[0],e.lower()[1],e.upper()[0],e.upper()[1]]
            }).collect();
            for (n,r) in rects.iter().enumerate() {
                proptest::prop_assert_eq!((r.width,r.height),(items[n].rect.width,items[n].rect.height));
                for b in &envelopes[n+1..] {
                    let a=envelopes[n];
                    proptest::prop_assert!(a[2]+gap <= b[0]+0.001 || b[2]+gap <= a[0]+0.001 || a[3]+gap <= b[1]+0.001 || b[3]+gap <= a[1]+0.001);
                }
            }
        }
        #[test]
        fn editing_commands_undo_and_checkpoint_roundtrip(dx in -1000f32..1000.,dy in -1000f32..1000.,n in 2usize..20) {
            let mut scene=Scene::default();
            for i in 0..n {let item=Item::text(format!("note {i}"),Rect{x:i as f32*15.,y:i as f32*13.,width:10.+i as f32,height:20.},scene.top());scene.apply(Command::Insert(item)).unwrap();}
            let mut original:Vec<_>=scene.items().cloned().collect();original.sort_by_key(|i|i.id);
            for duplicate in [false,true] {
                let mut items:Vec<_>=scene.items().collect();items.sort_by(|a,b|a.z.cmp(&b.z));
                let mut commands=translate(&items,[dx,dy],duplicate,scene.top());
                commands.extend(arrange(&items,Arrange::Optimal,150.,0.));
                for mode in [Align::Left,Align::Right,Align::Top,Align::Bottom,Align::Horizontal,Align::Vertical,Align::Stack] {commands.extend(align(&items,mode));}
                let selected=items.iter().take(2).map(|i|i.id).collect();
                commands.extend(reorder(&scene,&selected,true,false));
                let mut inverse=Vec::new();for c in commands {inverse.push(scene.apply(c).unwrap());}
                let payload=moth_format::board::snapshot(&scene).unwrap();let mut bytes=Vec::new();moth_format::board::initialize(&mut bytes).unwrap();let offset=bytes.len() as u64;moth_format::board::append(&mut bytes,offset,3,&payload).unwrap();
                let loaded=moth_format::board::read(&mut std::io::Cursor::new(bytes)).unwrap();
                for item in scene.items() {proptest::prop_assert_eq!(loaded.scene.get(item.id),Some(item));}
                for c in inverse.into_iter().rev() {scene.apply(c).unwrap();}
                let mut after:Vec<_>=scene.items().cloned().collect();after.sort_by_key(|i|i.id);proptest::prop_assert_eq!(&after,&original);
            }
        }
    }
    #[test]
    fn layer_steps_keep_selected_order_and_cross_one_neighbor() {
        let mut scene = Scene::default();
        let mut ids = Vec::new();
        for n in 0..4 {
            let item = Item::text(
                n.to_string(),
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 10.,
                    height: 10.,
                },
                scene.top(),
            );
            ids.push(item.id);
            scene.apply(Command::Insert(item)).unwrap();
        }
        for (front, extreme, expected) in [
            (true, false, vec![0, 3, 1, 2]),
            (false, false, vec![1, 2, 0, 3]),
            (true, true, vec![0, 3, 1, 2]),
            (false, true, vec![1, 2, 0, 3]),
        ] {
            let commands = reorder(
                &scene,
                &[ids[1], ids[2]].into_iter().collect(),
                front,
                extreme,
            );
            let mut undo = Vec::new();
            for c in commands {
                undo.push(scene.apply(c).unwrap());
            }
            let order: Vec<_> = scene
                .query(Rect {
                    x: 0.,
                    y: 0.,
                    width: 10.,
                    height: 10.,
                })
                .iter()
                .map(|i| i.id)
                .collect();
            assert_eq!(
                order,
                expected.into_iter().map(|i| ids[i]).collect::<Vec<_>>()
            );
            for c in undo.into_iter().rev() {
                scene.apply(c).unwrap();
            }
        }
    }
    #[test]
    fn alignment_uses_rotated_edges_and_distribution_preserves_extremes() {
        let a = item(1, 20., 10.);
        let mut b = item(2, 10., 20.);
        b.rect.x = 100.;
        b.rect.y = 200.;
        b.transform.rotation = std::f32::consts::FRAC_PI_2;
        let out = rects(&align(&[&a, &b], Align::Left));
        assert!((out[1].x - 5.).abs() < 0.001);
        let out = rects(&align(&[&a, &b], Align::Horizontal));
        assert_eq!(out[0], a.rect);
        assert_eq!(out[1], b.rect);
    }
    #[test]
    fn optimal_fills_space_beside_tall_images() {
        let items = [item(1, 50., 100.), item(2, 50., 50.), item(3, 50., 50.)];
        let refs: Vec<_> = items.iter().collect();
        let out = rects(&arrange(&refs, Arrange::Optimal, 100., 0.));
        assert_eq!(out.iter().map(|r| r.y + r.height).fold(0., f32::max), 100.);
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
        let vertical = rects(&arrange(&items, Arrange::Vertical, 1000., 24.));
        assert_eq!(vertical[1].y, vertical[0].y + 50. + 24.);
        assert_eq!(vertical[1].x, vertical[0].x);
        let horizontal = rects(&arrange(&items, Arrange::Horizontal, 1000., 24.));
        assert_eq!(horizontal[1].x, horizontal[0].x + 100. + 24.);
        assert_eq!(horizontal[1].y, horizontal[0].y);
    }

    #[test]
    fn custom_gap_and_invalid_gap() {
        let a = item(1, 10., 20.);
        let b = item(2, 10., 20.);
        let out = rects(&arrange(&[&a, &b], Arrange::Horizontal, 100., 7.));
        assert_eq!(out[1].x, 17.);
        assert!(arrange(&[&a, &b], Arrange::Horizontal, 100., f32::NAN).is_empty());
        assert!(arrange(&[&a, &b], Arrange::Horizontal, 100., -1.).is_empty());
    }
    #[test]
    fn arrange_grid_wraps_after_sqrt_columns() {
        let items: Vec<_> = (0..5).map(|n| item(n, 10., 10.)).collect();
        let refs: Vec<&Item> = items.iter().collect();
        let out = rects(&arrange(&refs, Arrange::Grid, 1000., 24.));
        assert_eq!(grid_columns(5), 3);
        // row 2 starts at the reference x, one row down.
        assert_eq!(out[3].x, out[0].x);
        assert_eq!(out[3].y, out[0].y + 10. + 24.);
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
        commands.extend(arrange(&refs, Arrange::Grid, 1000., 24.));
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
