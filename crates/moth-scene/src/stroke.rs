// SPDX-License-Identifier: GPL-3.0-or-later
use super::{Content, FractionalIndex, Item, Rect};

/// Bound both pointer capture and the encoded size of one annotation.
pub const MAX_STROKE_POINTS: usize = 2048;

/// How a drawing gesture produces an annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeStyle {
    /// An opaque freehand line.
    Pen,
    /// A broad, translucent freehand line.
    Marker,
    /// A straight shaft with an open head.
    Arrow,
}

impl StrokeStyle {
    /// Default width in screen points for the GUI, or board units for native operations.
    pub fn width(self) -> f32 {
        match self {
            Self::Pen | Self::Arrow => 4.,
            Self::Marker => 18.,
        }
    }
}

/// An editable polyline in frame-local coordinates. Resizing the frame scales the path.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// Coordinates normalized to the item's unrotated frame.
    pub points: Vec<[f32; 2]>,
    /// Line width as a fraction of the frame's shorter side.
    pub width: f32,
    /// Opaque sRGB swatch. Translucency is part of the item transform.
    pub color: [u8; 3],
}

impl Stroke {
    /// Rejects invalid data before it can enter the spatial index or painter.
    pub fn valid(&self) -> bool {
        (2..=MAX_STROKE_POINTS).contains(&self.points.len())
            && self.width > 0.
            && self.width <= 1.
            && self.points.iter().flatten().all(|v| (0. ..=1.).contains(v))
    }

    /// Width in board units after resizing.
    pub fn world_width(&self, frame: Rect) -> f32 {
        self.width * frame.width.min(frame.height)
    }

    /// Constructs an item with half a line-width of padding around the path.
    pub fn item(
        input: Vec<[f32; 2]>,
        style: StrokeStyle,
        width: f32,
        color: [u8; 3],
        above: Option<&FractionalIndex>,
    ) -> Option<Item> {
        if !(2..=MAX_STROKE_POINTS).contains(&input.len())
            || !width.is_finite()
            || width <= 0.
            || !input.iter().flatten().all(|v| v.is_finite())
        {
            return None;
        }
        let mut points = input;
        if style == StrokeStyle::Arrow {
            let start = points[0];
            let end = *points.last()?;
            let d = [end[0] - start[0], end[1] - start[1]];
            let length = d[0].hypot(d[1]);
            if length < width / 2. {
                return None;
            }
            let head = (width * 5.).min(length / 2.);
            let [x, y] = d.map(|v| v / length * head);
            points = vec![
                start,
                end,
                [end[0] - x - y * 0.55, end[1] - y + x * 0.55],
                end,
                [end[0] - x + y * 0.55, end[1] - y - x * 0.55],
            ];
        } else if points.iter().all(|p| *p == points[0]) {
            return None;
        }
        let mut lo = points[0];
        let mut hi = points[0];
        for point in &points {
            for axis in 0..2 {
                lo[axis] = lo[axis].min(point[axis]);
                hi[axis] = hi[axis].max(point[axis]);
            }
        }
        let rect = Rect {
            x: lo[0] - width / 2.,
            y: lo[1] - width / 2.,
            width: hi[0] - lo[0] + width,
            height: hi[1] - lo[1] + width,
        };
        if !rect.valid() {
            return None;
        }
        for [x, y] in &mut points {
            *x = (*x - rect.x) / rect.width;
            *y = (*y - rect.y) / rect.height;
        }
        let stroke = Self {
            points,
            width: width / rect.width.min(rect.height),
            color,
        };
        if !stroke.valid() {
            return None;
        }
        let mut item = Item::text("", rect, above);
        item.content = Content::Stroke(stroke);
        if style == StrokeStyle::Marker {
            item.transform.opacity = 0.35;
        }
        Some(item)
    }

    /// Tests the actual path with some picking tolerance, leaving empty corners clickable.
    pub fn hits(
        &self,
        frame: Rect,
        rotation: f32,
        flip: [bool; 2],
        point: [f32; 2],
        tolerance: f32,
    ) -> bool {
        let [dx, dy] = [
            point[0] - frame.x - frame.width / 2.,
            point[1] - frame.y - frame.height / 2.,
        ];
        let (sin, cos) = rotation.sin_cos();
        let [x, y] = [
            (dx * cos + dy * sin) * if flip[0] { -1. } else { 1. } + frame.width / 2.,
            (-dx * sin + dy * cos) * if flip[1] { -1. } else { 1. } + frame.height / 2.,
        ];
        let radius = self.world_width(frame) / 2. + tolerance;
        self.points.windows(2).any(|pair| {
            let [a, b] = [pair[0], pair[1]];
            let [ax, ay] = [a[0] * frame.width, a[1] * frame.height];
            let [vx, vy] = [(b[0] - a[0]) * frame.width, (b[1] - a[1]) * frame.height];
            let length = vx * vx + vy * vy;
            let along = if length > 0. {
                (((x - ax) * vx + (y - ay) * vy) / length).clamp(0., 1.)
            } else {
                0.
            };
            (x - ax - vx * along).hypot(y - ay - vy * along) <= radius
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn stroke_is_reversible(x in -1000f32..1000., y in -1000f32..1000.) {
            use crate::{Command, Scene};
            let item = Stroke::item(vec![[x,y],[x + 20., y + 40.]], StrokeStyle::Arrow, 4., [7;3], None).unwrap();
            let mut scene = Scene::default();
            let remove = scene.apply(Command::Insert(item.clone())).unwrap();
            let restore = scene.apply(remove).unwrap();
            prop_assert!(scene.is_empty());
            scene.apply(restore).unwrap();
            prop_assert_eq!(scene.get(item.id), Some(&item));
        }
    }
}
