// SPDX-License-Identifier: GPL-3.0-or-later
//! Normalize and arrange for a selection: pure geometry that emits reversible commands.
//!
//! The first item (z-sorted, like the rest of the selection menu) is the reference:
//! normalize matches its frame along one measure; arrange uses the selection's minimum x/y.
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
    /// Compact mosaic with the same gutter on both axes.
    Grid,
    /// Packs unchanged displayed bounds tightly into a small square, without gaps.
    Optimal,
}

/// Preference for the unused space surrounding the next item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FreeRectChoice {
    /// `BestAreaFit`: least leftover area.
    BestArea,
    /// `BestShortSideFit`: least waste on the shorter leftover side.
    BestShortSide,
    /// `BestLongSideFit`: least waste on the longer leftover side.
    BestLongSide,
    /// `WorstAreaFit`: most leftover area.
    WorstArea,
    /// `WorstShortSideFit`: most waste on the shorter leftover side.
    WorstShortSide,
    /// `WorstLongSideFit`: most waste on the longer leftover side.
    WorstLongSide,
}

impl FreeRectChoice {
    /// Every heuristic, in display order.
    pub const ALL: [Self; 6] = [
        Self::BestArea,
        Self::BestShortSide,
        Self::BestLongSide,
        Self::WorstArea,
        Self::WorstShortSide,
        Self::WorstLongSide,
    ];
    /// The heuristic stored under `index`; out-of-range falls back to the default.
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::BestShortSide)
    }
}

/// Preference for dividing unused space below and beside a placed item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GuillotineSplit {
    ShorterLeftoverAxis,
    LongerLeftoverAxis,
    MinimizeArea,
    MaximizeArea,
    ShorterAxis,
    LongerAxis,
}

impl GuillotineSplit {
    /// Every heuristic, in display order.
    pub const ALL: [Self; 6] = [
        Self::ShorterLeftoverAxis,
        Self::LongerLeftoverAxis,
        Self::MinimizeArea,
        Self::MaximizeArea,
        Self::ShorterAxis,
        Self::LongerAxis,
    ];
    /// The heuristic stored under `index`; out-of-range falls back to the default.
    pub fn from_index(index: u8) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::ShorterLeftoverAxis)
    }
}

/// Scales every frame to match the reference along `mode`, keeping each top-left anchor.
pub fn normalize(items: &[&Item], mode: Normalize) -> Vec<Command> {
    let Some(reference) = items.first().map(|item| item.rect) else {
        return Vec::new();
    };
    items
        .iter()
        .flat_map(|item| {
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
            if !rect.valid() {
                return Vec::new();
            }
            let mut commands = vec![Command::SetRect { id: item.id, rect }];
            if item.note().is_some() {
                commands.push(Command::SetTransform {
                    id: item.id,
                    transform: item.scaled_note_transform(rect.height / item.rect.height),
                });
            }
            commands
        })
        .collect()
}

/// Repositions frames starting at the reference anchor, keeping each frame's size.
pub fn arrange(
    items: &[&Item],
    mode: Arrange,
    gap: f32,
    choice: FreeRectChoice,
    split: GuillotineSplit,
) -> Vec<Command> {
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
        Arrange::Grid => pack(items, start, gap, choice, split, &mut commands),
        Arrange::Optimal => pack(items, start, 0., choice, split, &mut commands),
    }
    commands
}

/// Count means rows for horizontal flow or columns for vertical flow.
pub fn counted(items: &[&Item], mode: Arrange, count: usize, gap: f32) -> Vec<Command> {
    if !matches!(mode, Arrange::Horizontal | Arrange::Vertical)
        || items.is_empty()
        || !gap.is_finite()
        || !(0.0..=10000.).contains(&gap)
    {
        return Vec::new();
    }
    let start = items.iter().fold((f32::INFINITY, f32::INFINITY), |a, i| {
        (a.0.min(i.rect.x), a.1.min(i.rect.y))
    });
    let count = count.clamp(1, items.len());
    let vertical = matches!(mode, Arrange::Vertical);
    let line_length = |line: usize| items.len() / count + usize::from(line < items.len() % count);
    let mut line = 0;
    let mut next_line = line_length(line);
    let mut commands = Vec::with_capacity(items.len());
    let (mut x, mut y) = start;
    let mut extent = 0.0f32;
    for (index, item) in items.iter().enumerate() {
        if index == next_line {
            line += 1;
            next_line += line_length(line);
            if vertical {
                x += extent + gap;
                y = start.1;
            } else {
                y += extent + gap;
                x = start.0;
            }
            extent = 0.;
        }
        let rect = Rect { x, y, ..item.rect };
        if !rect.valid() {
            return Vec::new();
        }
        push(&mut commands, item, rect);
        if vertical {
            y += item.rect.height + gap;
            extent = extent.max(item.rect.width);
        } else {
            x += item.rect.width + gap;
            extent = extent.max(item.rect.height);
        }
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

/// Search finite square bins using all displayed sizes, independently of the viewport.
/// Geometry is calculated in f64 so area scores and accumulated positions stay finite.
fn pack(
    items: &[&Item],
    start: (f32, f32),
    gap: f32,
    choice: FreeRectChoice,
    split: GuillotineSplit,
    commands: &mut Vec<Command>,
) {
    if items
        .iter()
        .any(|item| !item.rect.valid() || !item.transform.valid())
    {
        return;
    }
    let gap = f64::from(gap);
    let sizes: Vec<_> = items
        .iter()
        .map(|item| {
            let (sin, cos) = f64::from(item.transform.rotation).sin_cos();
            let r = item.rect;
            [
                f64::from(r.width) * cos.abs() + f64::from(r.height) * sin.abs() + gap,
                f64::from(r.width) * sin.abs() + f64::from(r.height) * cos.abs() + gap,
            ]
        })
        .collect();
    let mut order: Vec<_> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        (sizes[b][0] * sizes[b][1])
            .total_cmp(&(sizes[a][0] * sizes[a][1]))
            .then(a.cmp(&b))
    });
    let area: f64 = sizes.iter().map(|s| s[0] * s[1]).sum();
    let mut low = sizes
        .iter()
        .fold(area.sqrt(), |side, s| side.max(s[0]).max(s[1]));
    // Leave slack so subtraction rounding cannot reject the final search bin.
    let limit = sizes.iter().map(|s| s[0].max(s[1])).sum::<f64>() * 2.;
    let mut high = low;
    let mut positions = loop {
        if let Some(positions) = pack_square(&sizes, &order, high, choice, split) {
            break positions;
        }
        if high >= limit {
            return;
        }
        low = high;
        high = (high * 1.25).min(limit);
    };
    // shortcut: bounded greedy search, not exhaustive packing; revisit if real boards leave large holes.
    for _ in 0..8 {
        if high - low <= high * 0.001 {
            break;
        }
        let side = (low + high) / 2.;
        if let Some(candidate) = pack_square(&sizes, &order, side, choice, split) {
            high = side;
            positions = candidate;
        } else {
            low = side;
        }
    }
    let result: Vec<_> = items
        .iter()
        .zip(sizes)
        .zip(positions)
        .map(|((item, size), position)| Rect {
            x: (f64::from(start.0)
                + position[0]
                + (size[0] - gap - f64::from(item.rect.width)) / 2.) as f32,
            y: (f64::from(start.1)
                + position[1]
                + (size[1] - gap - f64::from(item.rect.height)) / 2.) as f32,
            ..item.rect
        })
        .collect();
    // A layout outside scene coordinate limits must not move only part of the selection.
    if result.iter().all(|r| r.valid()) {
        commands.extend(
            items
                .iter()
                .zip(result)
                .map(|(item, rect)| Command::SetRect { id: item.id, rect }),
        );
    }
}

fn pack_square(
    sizes: &[[f64; 2]],
    order: &[usize],
    side: f64,
    choice: FreeRectChoice,
    split: GuillotineSplit,
) -> Option<Vec<[f64; 2]>> {
    let mut free = vec![PackingRect {
        x: 0.,
        y: 0.,
        width: side,
        height: side,
    }];
    let mut positions = vec![[0.; 2]; sizes.len()];
    for &index in order {
        let [w, h] = sizes[index];
        let slot = free
            .iter()
            .enumerate()
            .filter(|(_, r)| r.width >= w && r.height >= h)
            .min_by(|(_, a), (_, b)| a.score(w, h, choice).total_cmp(&b.score(w, h, choice)))
            .map(|(index, _)| index)?;
        let space = free.remove(slot);
        positions[index] = [space.x, space.y];
        free.extend(
            space
                .remainder(w, h, split)
                .into_iter()
                .filter(|r| r.width > 0. && r.height > 0.),
        );
    }
    Some(positions)
}

#[derive(Clone, Copy)]
struct PackingRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl PackingRect {
    fn score(self, width: f64, height: f64, choice: FreeRectChoice) -> f64 {
        let across = self.width - width;
        let down = self.height - height;
        match choice {
            FreeRectChoice::BestArea => self.width * self.height - width * height,
            FreeRectChoice::WorstArea => width * height - self.width * self.height,
            FreeRectChoice::BestShortSide => across.min(down),
            FreeRectChoice::WorstShortSide => -across.min(down),
            FreeRectChoice::BestLongSide => across.max(down),
            FreeRectChoice::WorstLongSide => -across.max(down),
        }
    }

    fn remainder(self, width: f64, height: f64, split: GuillotineSplit) -> [Self; 2] {
        let across = self.width - width;
        let down = self.height - height;
        let below_area = self.width * down;
        let beside_area = self.height * across;
        let horizontal = match split {
            GuillotineSplit::ShorterLeftoverAxis => across <= down,
            GuillotineSplit::LongerLeftoverAxis => across > down,
            GuillotineSplit::ShorterAxis => self.width <= self.height,
            GuillotineSplit::LongerAxis => self.width > self.height,
            GuillotineSplit::MinimizeArea => below_area < beside_area,
            GuillotineSplit::MaximizeArea => below_area >= beside_area,
        };
        [
            Self {
                x: self.x + width,
                y: self.y,
                width: across,
                height: if horizontal { height } else { self.height },
            },
            Self {
                x: self.x,
                y: self.y + height,
                width: if horizontal { self.width } else { width },
                height: down,
            },
        ]
    }
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
pub(super) fn scale(items: &[&moth_scene::Item], factor: f32) -> Vec<Command> {
    if !factor.is_finite() || factor <= 0. {
        return Vec::new();
    }
    let Some(first) = items.first() else {
        return Vec::new();
    };
    let commands: Vec<_> = items
        .iter()
        .map(|item| Command::SetRect {
            id: item.id,
            rect: Rect {
                x: first.rect.x + (item.rect.x - first.rect.x) * factor,
                y: first.rect.y + (item.rect.y - first.rect.y) * factor,
                width: item.rect.width * factor,
                height: item.rect.height * factor,
            },
        })
        .collect();
    let mut commands = commands;
    commands.extend(
        items
            .iter()
            .filter(|item| item.note().is_some())
            .map(|item| Command::SetTransform {
                id: item.id,
                transform: item.scaled_note_transform(factor),
            }),
    );
    if commands.iter().all(|c| match c {
        Command::SetRect { rect, .. } => rect.valid(),
        Command::SetTransform { transform, .. } => transform.valid(),
        _ => false,
    }) {
        commands
    } else {
        Vec::new()
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
    fn counted_layouts_use_exact_counts_and_preserve_sizes() {
        let items: Vec<_> = (0..5)
            .map(|n| item(n, 10. + f32::from(n), 20. + f32::from(n)))
            .collect();
        let refs: Vec<_> = items.iter().collect();
        for mode in [Arrange::Horizontal, Arrange::Vertical] {
            for requested in [0, 1, 2, 3, 4, 5, 100] {
                let out = rects(&counted(&refs, mode, requested, 7.));
                assert_eq!(out.len(), items.len());
                let mut lines: Vec<_> = out
                    .iter()
                    .map(|r| {
                        if matches!(mode, Arrange::Vertical) {
                            r.x
                        } else {
                            r.y
                        }
                    })
                    .collect();
                lines.dedup();
                assert_eq!(lines.len(), requested.clamp(1, items.len()));
                for (rect, item) in out.iter().zip(&items) {
                    assert_eq!(
                        (rect.width, rect.height),
                        (item.rect.width, item.rect.height)
                    );
                }
            }
        }
        let vertical = rects(&counted(&refs, Arrange::Vertical, 2, 7.));
        assert_eq!(vertical[0].x, vertical[2].x);
        assert!(vertical[1].y > vertical[0].y);
        assert!(vertical[3].x > vertical[0].x);
        let horizontal = rects(&counted(&refs, Arrange::Horizontal, 2, 7.));
        assert_eq!(horizontal[0].y, horizontal[2].y);
        assert!(horizontal[1].x > horizontal[0].x);
        assert!(horizontal[3].y > horizontal[0].y);
        assert!(counted(&refs, Arrange::Grid, 2, f32::NAN).is_empty());
        assert!(counted(&[], Arrange::Vertical, 0, 0.).is_empty());
    }
    fn arrange_default(items: &[&Item], mode: Arrange, gap: f32) -> Vec<Command> {
        arrange(
            items,
            mode,
            gap,
            FreeRectChoice::BestShortSide,
            GuillotineSplit::ShorterLeftoverAxis,
        )
    }

    proptest::proptest! {
        #[test]
        fn packing_keeps_displayed_bounds_disjoint(
            sizes in proptest::collection::vec((1f32..300., 1f32..300., -std::f32::consts::PI..std::f32::consts::PI), 1..40),
            gap in 0f32..30., choice in 0u8..6, split in 0u8..6
        ) {
            let items: Vec<_> = sizes.into_iter().enumerate().map(|(n,(w,h,rotation))| {
                let mut i=item(n as u8,w,h); i.transform.rotation=rotation; i
            }).collect();
            let refs: Vec<_> = items.iter().collect();
            for mode in [Arrange::Optimal, Arrange::Grid] {
                let commands=arrange(&refs,mode,gap,FreeRectChoice::from_index(choice),GuillotineSplit::from_index(split));
                proptest::prop_assert_eq!(commands.len(),items.len());
                let rects=rects(&commands);
                let envelopes: Vec<_> = rects.iter().zip(&items).map(|(r,i)| {
                    let e=r.rotated_envelope(i.transform.rotation);
                    [e.lower()[0],e.lower()[1],e.upper()[0],e.upper()[1]]
                }).collect();
                let spacing = if matches!(mode, Arrange::Grid) { gap } else { 0. };
                for (n,r) in rects.iter().enumerate() {
                    proptest::prop_assert_eq!((r.width,r.height),(items[n].rect.width,items[n].rect.height));
                    for b in &envelopes[n+1..] {
                        let a=envelopes[n];
                        proptest::prop_assert!(a[2]+spacing <= b[0]+0.001 || b[2]+spacing <= a[0]+0.001 || a[3]+spacing <= b[1]+0.001 || b[3]+spacing <= a[1]+0.001);
                    }
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
                commands.extend(arrange_default(&items,Arrange::Optimal,0.));
                commands.extend(arrange_default(&items,Arrange::Grid,7.));
                for mode in [Arrange::Horizontal, Arrange::Vertical] {
                    for count in [0, 1, n / 2, n, n + 1] {
                        commands.extend(counted(&items, mode, count, 7.));
                    }
                }
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
        let out = rects(&arrange_default(&refs, Arrange::Optimal, 0.));
        assert_eq!(out.iter().map(|r| r.y + r.height).fold(0., f32::max), 100.);
    }

    #[test]
    fn every_heuristic_pair_packs_without_overlap() {
        let sizes = [
            (100., 60.),
            (40., 90.),
            (70., 70.),
            (120., 30.),
            (50., 50.),
            (30., 120.),
            (90., 40.),
        ];
        let items: Vec<_> = sizes
            .into_iter()
            .enumerate()
            .map(|(n, (w, h))| item(n as u8, w, h))
            .collect();
        let refs: Vec<_> = items.iter().collect();
        for choice in FreeRectChoice::ALL {
            for split in GuillotineSplit::ALL {
                let mut commands = Vec::new();
                pack(&refs, (0., 0.), 4., choice, split, &mut commands);
                let rects = rects(&commands);
                assert_eq!(rects.len(), items.len(), "{choice:?} {split:?}");
                for (n, r) in rects.iter().enumerate() {
                    for o in rects.iter().skip(n + 1) {
                        assert!(
                            r.x + r.width <= o.x
                                || o.x + o.width <= r.x
                                || r.y + r.height <= o.y
                                || o.y + o.height <= r.y,
                            "{choice:?} {split:?} overlap {r:?} vs {o:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn packing_rotated_images_survives_rounding_at_the_search_limit() {
        let mut items = [item(1, 51.27531, 269.77512), item(2, 180.6287, 194.46127)];
        items[0].transform.rotation = -1.3824722;
        items[1].transform.rotation = 2.1351492;
        let refs: Vec<_> = items.iter().collect();
        for mode in [Arrange::Optimal, Arrange::Grid] {
            for choice in FreeRectChoice::ALL {
                for split in GuillotineSplit::ALL {
                    let out = rects(&arrange(&refs, mode, 0., choice, split));
                    assert_eq!(out.len(), items.len(), "{choice:?} {split:?}");
                    for (rect, item) in out.iter().zip(&items) {
                        assert!(rect.valid());
                        assert_eq!(
                            (rect.width, rect.height),
                            (item.rect.width, item.rect.height)
                        );
                    }
                    let a = out[0].rotated_envelope(items[0].transform.rotation);
                    let b = out[1].rotated_envelope(items[1].transform.rotation);
                    assert!(
                        a.upper()[0] <= b.lower()[0] + 0.001
                            || b.upper()[0] <= a.lower()[0] + 0.001
                            || a.upper()[1] <= b.lower()[1] + 0.001
                            || b.upper()[1] <= a.lower()[1] + 0.001,
                        "{choice:?} {split:?} overlap {a:?} vs {b:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn packing_handles_wide_items_and_preserves_command_order() {
        let items = [item(1, 30., 80.), item(2, 400., 10.), item(3, 20., 40.)];
        let refs: Vec<_> = items.iter().collect();
        for choice in FreeRectChoice::ALL {
            for split in GuillotineSplit::ALL {
                let commands = arrange(&refs, Arrange::Optimal, 7., choice, split);
                assert_eq!(commands.len(), items.len());
                for (command, item) in commands.iter().zip(&items) {
                    let Command::SetRect { id, rect } = command else {
                        panic!("unexpected command")
                    };
                    assert_eq!(*id, item.id);
                    assert!(rect.valid());
                    assert_eq!(
                        (rect.width, rect.height),
                        (item.rect.width, item.rect.height)
                    );
                }
            }
        }
    }

    #[test]
    fn packing_rejects_invalid_input_and_out_of_range_layout_atomically() {
        let a = item(1, 10., 10.);
        let mut b = item(2, 10., 10.);
        b.transform.rotation = f32::NAN;
        assert!(arrange_default(&[&a, &b], Arrange::Optimal, 0.).is_empty());
        b.transform.rotation = 0.;
        for gap in [-1., f32::NAN, f32::INFINITY] {
            assert!(arrange_default(&[&a, &b], Arrange::Optimal, gap).is_empty());
        }
        let mut a = item(1, 1e9, 1e9);
        let mut b = item(2, 1e9, 1e9);
        a.rect.x = 1e9;
        a.rect.y = 1e9;
        b.rect.x = 1e9;
        b.rect.y = 1e9;
        assert!(arrange_default(&[&a, &b], Arrange::Optimal, 0.).is_empty());
    }

    #[test]
    fn optimal_uses_all_images_for_a_tight_square_and_ignores_the_flow_gap() {
        let items: Vec<_> = (0..9).map(|n| item(n, 100., 100.)).collect();
        let refs: Vec<_> = items.iter().collect();
        let packed = rects(&arrange_default(&refs, Arrange::Optimal, 0.));
        assert_eq!(
            packed,
            rects(&arrange_default(&refs, Arrange::Optimal, 24.))
        );
        assert_eq!(
            packed.iter().map(|r| r.x + r.width).fold(0., f32::max),
            300.
        );
        assert_eq!(
            packed.iter().map(|r| r.y + r.height).fold(0., f32::max),
            300.
        );
        let mixed = [item(0, 120., 200.), item(1, 80., 120.), item(2, 80., 80.)];
        let packed = rects(&arrange_default(
            &mixed.iter().collect::<Vec<_>>(),
            Arrange::Optimal,
            24.,
        ));
        assert_eq!(
            packed.iter().map(|r| r.x + r.width).fold(0., f32::max),
            200.
        );
        assert_eq!(
            packed.iter().map(|r| r.y + r.height).fold(0., f32::max),
            200.
        );
    }
    #[test]
    fn identical_images_pack_into_a_tight_grid() {
        let items: Vec<_> = (0..4).map(|n| item(n, 50., 50.)).collect();
        let refs: Vec<_> = items.iter().collect();
        let out = rects(&arrange_default(&refs, Arrange::Optimal, 0.));
        let width = out.iter().map(|r| r.x + r.width).fold(0., f32::max);
        let height = out.iter().map(|r| r.y + r.height).fold(0., f32::max);
        assert_eq!(width, 100.);
        assert_eq!(height, 100.);
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
        let vertical = rects(&arrange_default(&items, Arrange::Vertical, 24.));
        assert_eq!(vertical[1].y, vertical[0].y + 50. + 24.);
        assert_eq!(vertical[1].x, vertical[0].x);
        let horizontal = rects(&arrange_default(&items, Arrange::Horizontal, 24.));
        assert_eq!(horizontal[1].x, horizontal[0].x + 100. + 24.);
        assert_eq!(horizontal[1].y, horizontal[0].y);
    }

    #[test]
    fn custom_gap_and_invalid_gap() {
        let a = item(1, 10., 20.);
        let b = item(2, 10., 20.);
        let out = rects(&arrange_default(&[&a, &b], Arrange::Horizontal, 7.));
        assert_eq!(out[1].x, 17.);
        assert!(arrange_default(&[&a, &b], Arrange::Horizontal, f32::NAN).is_empty());
        assert!(arrange_default(&[&a, &b], Arrange::Horizontal, -1.).is_empty());
    }
    #[test]
    fn grid_fills_a_square_with_equal_gutters() {
        let items: Vec<_> = (0..4).map(|n| item(n, 100., 100.)).collect();
        let out = rects(&arrange_default(
            &items.iter().collect::<Vec<_>>(),
            Arrange::Grid,
            24.,
        ));
        assert_eq!(out.iter().map(|r| r.x + r.width).fold(0., f32::max), 224.);
        assert_eq!(out.iter().map(|r| r.y + r.height).fold(0., f32::max), 224.);
        assert_eq!(out[1].x - out[0].x - 100., 24.);
        assert_eq!(out[2].y - out[0].y - 100., 24.);
    }
    #[test]
    fn grid_uses_the_same_edge_gap_vertically_and_horizontally() {
        let items = [
            item(0, 100., 200.),
            item(1, 100., 100.),
            item(2, 100., 100.),
        ];
        let out = rects(&arrange_default(
            &items.iter().collect::<Vec<_>>(),
            Arrange::Grid,
            24.,
        ));
        assert_eq!(out.len(), 3);
        assert_eq!(out[1].x, out[2].x);
        assert_eq!(out[1].y, out[0].y);
        assert_eq!(out[1].x - out[0].x - out[0].width, 24.);
        assert_eq!(out[2].y - out[1].y - out[1].height, 24.);
        for (rect, item) in out.iter().zip(&items) {
            assert_eq!(
                (rect.width, rect.height),
                (item.rect.width, item.rect.height)
            );
        }
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
        commands.extend(arrange_default(&refs, Arrange::Grid, 24.));
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
