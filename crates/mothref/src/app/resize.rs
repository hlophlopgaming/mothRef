// SPDX-License-Identifier: GPL-3.0-or-later
//! A corner drag scales a captured selection; only release journals the result.
use moth_scene::{Camera, Command, Item, Rect};

pub(super) struct Resize {
    items: Vec<Item>,
    anchor: [f32; 2],
    corner: [f32; 2],
    factor: f32,
    pub released: bool,
}
impl Resize {
    pub fn start(items: Vec<Item>, camera: Camera, pointer: [f32; 2], radius: f32) -> Option<Self> {
        let first = items.first()?;
        let corners = if items.len() == 1 {
            first.rect.corners(first.transform.rotation)
        } else {
            let bounds = items.iter().fold(egui::Rect::NOTHING, |bounds, item| {
                let e = item.rect.rotated_envelope(item.transform.rotation);
                bounds.union(egui::Rect::from_min_max(
                    egui::pos2(e.lower()[0], e.lower()[1]),
                    egui::pos2(e.upper()[0], e.upper()[1]),
                ))
            });
            [
                bounds.left_top(),
                bounds.right_top(),
                bounds.left_bottom(),
                bounds.right_bottom(),
            ]
            .map(|p| [p.x, p.y])
        };
        let index = corners.iter().position(|corner| {
            let p = camera.screen(*corner);
            (p[0] - pointer[0]).hypot(p[1] - pointer[1]) <= radius
        })?;
        Some(Self {
            items,
            anchor: corners[3 - index],
            corner: corners[index],
            factor: 1.,
            released: false,
        })
    }
    pub fn update(&mut self, point: [f32; 2]) {
        let diagonal = [
            self.corner[0] - self.anchor[0],
            self.corner[1] - self.anchor[1],
        ];
        let length = diagonal[0].powi(2) + diagonal[1].powi(2);
        let factor = ((point[0] - self.anchor[0]) * diagonal[0]
            + (point[1] - self.anchor[1]) * diagonal[1])
            / length;
        if factor.is_finite() {
            let factor = factor.clamp(0.001, 1000.);
            if self
                .items
                .iter()
                .all(|item| self.rect(item, factor).valid())
            {
                self.factor = factor;
            }
        }
    }
    fn rect(&self, item: &Item, factor: f32) -> Rect {
        Rect {
            x: self.anchor[0] + (item.rect.x - self.anchor[0]) * factor,
            y: self.anchor[1] + (item.rect.y - self.anchor[1]) * factor,
            width: item.rect.width * factor,
            height: item.rect.height * factor,
        }
    }
    pub fn previews(&self) -> Vec<Item> {
        self.items
            .iter()
            .map(|item| {
                let mut copy = item.clone();
                copy.rect = self.rect(item, self.factor);
                copy
            })
            .collect()
    }
    pub fn commands(&self) -> Vec<Command> {
        if (self.factor - 1.).abs() < f32::EPSILON {
            return Vec::new();
        }
        self.items
            .iter()
            .map(|item| Command::SetRect {
                id: item.id,
                rect: self.rect(item, self.factor),
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::{BlobId, Scene};
    fn item(x: f32, width: f32, rotation: f32) -> Item {
        let mut item = Item::image(
            BlobId::from_bytes([1; 32]),
            Rect {
                x,
                y: 20.,
                width,
                height: 80.,
            },
            Default::default(),
        );
        item.transform.rotation = rotation;
        item
    }
    #[test]
    fn rotated_corner_keeps_opposite_corner_fixed() {
        let item = item(10., 120., 0.7);
        let corners = item.rect.corners(item.transform.rotation);
        let camera = Camera::default();
        let mut drag =
            Resize::start(vec![item.clone()], camera, camera.screen(corners[3]), 12.).unwrap();
        drag.update([
            corners[0][0] + (corners[3][0] - corners[0][0]) * 2.,
            corners[0][1] + (corners[3][1] - corners[0][1]) * 2.,
        ]);
        let preview = drag.previews().remove(0);
        let fixed = preview.rect.corners(preview.transform.rotation)[0];
        assert!((fixed[0] - corners[0][0]).abs() < 0.001);
        assert!((fixed[1] - corners[0][1]).abs() < 0.001);
        assert!((preview.rect.width - item.rect.width * 2.).abs() < 0.001);
    }
    proptest::proptest! {
        #[test]
        fn multi_resize_undo_and_checkpoint_preserve_scene(factor in 0.05f32..10.) {
            let items = vec![item(10., 120., 0.), item(150., 60., 0.)];
            let mut scene = Scene::default();
            for item in &items { scene.apply(Command::Insert(item.clone())).unwrap(); }
            let mut drag = Resize::start(items.clone(), Camera::default(), [210., 100.], 12.).unwrap();
            drag.update([10. + 200. * factor, 20. + 80. * factor]);
            let mut inverse = Vec::new();
            for command in drag.commands() { inverse.push(scene.apply(command).unwrap()); }
            let payload = moth_format::board::snapshot(&scene).unwrap();
            let mut bytes = Vec::new();
            moth_format::board::initialize(&mut bytes).unwrap();
            let offset = bytes.len() as u64;
            moth_format::board::append(&mut bytes, offset, 3, &payload).unwrap();
            let loaded = moth_format::board::read(&mut std::io::Cursor::new(bytes)).unwrap();
            for preview in drag.previews() { proptest::prop_assert_eq!(loaded.scene.get(preview.id), Some(&preview)); }
            for command in inverse.into_iter().rev() { scene.apply(command).unwrap(); }
            for item in items { proptest::prop_assert_eq!(scene.get(item.id), Some(&item)); }
        }
    }
}
