// SPDX-License-Identifier: GPL-3.0-or-later
//! A corner drag scales a captured selection; only release journals the result.
use moth_scene::{Camera, Command, Item, Rect};

pub(super) struct Resize {
    items: Vec<Item>,
    anchor: [f32; 2],
    corner: [f32; 2],
    factor: f32,
    corner_index: usize,
    note_height: Option<f32>,
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
            corner_index: index,
            note_height: None,
            released: false,
        })
    }
    pub fn update(&mut self, point: [f32; 2]) {
        let diagonal = [
            self.corner[0] - self.anchor[0],
            self.corner[1] - self.anchor[1],
        ];
        let length = diagonal[0].powi(2) + diagonal[1].powi(2);
        let factor = if let [item] = self.items.as_slice()
            && item.note().is_some()
        {
            let (sin, cos) = item.transform.rotation.sin_cos();
            let width = (point[0] - self.anchor[0]) * cos + (point[1] - self.anchor[1]) * sin;
            width / item.rect.width * if self.corner_index & 1 == 0 { -1. } else { 1. }
        } else {
            ((point[0] - self.anchor[0]) * diagonal[0] + (point[1] - self.anchor[1]) * diagonal[1])
                / length
        };
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
    pub fn wrap_note(&mut self, ctx: &egui::Context) {
        if let [item] = self.items.as_slice()
            && let Some(text) = item.note()
        {
            let previous = self.note_height;
            self.note_height = Some(
                super::canvas::measure_note_frame(
                    ctx,
                    text,
                    item.note_font_size(),
                    Some(item.rect.width * self.factor),
                )[1],
            );
            if !self.rect(item, self.factor).valid() {
                self.note_height = previous;
            }
        }
    }
    fn transform(&self, item: &Item) -> moth_scene::Transform {
        if self.note_height.is_some() {
            moth_scene::Transform {
                note_font_size: item.note_font_size(),
                note_wrap: true,
                ..item.transform
            }
        } else {
            item.scaled_note_transform(self.factor)
        }
    }
    fn rect(&self, item: &Item, factor: f32) -> Rect {
        if let Some(height) = self.note_height {
            let width = item.rect.width * factor;
            let delta = [
                if self.corner_index & 1 == 0 {
                    -width
                } else {
                    width
                },
                if self.corner_index & 2 == 0 {
                    -height
                } else {
                    height
                },
            ];
            let (sin, cos) = item.transform.rotation.sin_cos();
            let center = [
                self.anchor[0] + (delta[0] * cos - delta[1] * sin) / 2.,
                self.anchor[1] + (delta[0] * sin + delta[1] * cos) / 2.,
            ];
            return Rect {
                x: center[0] - width / 2.,
                y: center[1] - height / 2.,
                width,
                height,
            };
        }
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
                copy.transform = self.transform(item);
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
            .flat_map(|item| {
                let mut commands = vec![Command::SetRect {
                    id: item.id,
                    rect: self.rect(item, self.factor),
                }];
                if item.note().is_some() {
                    commands.push(Command::SetTransform {
                        id: item.id,
                        transform: self.transform(item),
                    });
                }
                commands
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
    #[test]
    fn a_single_note_wraps_on_resize_without_changing_its_font_or_anchor() {
        let ctx = egui::Context::default();
        moth_ui::theme(&ctx, &moth_ui::Tokens::LIGHT);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut note = Item::text(
                "one two three four five six",
                Rect {
                    x: 10.,
                    y: 20.,
                    width: 300.,
                    height: 32.,
                },
                None,
            );
            note.transform.note_font_size = 24.;
            let mut drag =
                Resize::start(vec![note.clone()], Camera::default(), [310., 52.], 12.).unwrap();
            drag.update([110., 31.]);
            drag.wrap_note(ui.ctx());
            let preview = drag.previews().remove(0);
            assert!(preview.rect.width < note.rect.width);
            assert!(preview.rect.height > note.rect.height);
            assert_eq!(preview.note_font_size(), 24.);
            assert!(preview.transform.note_wrap);
            assert_eq!(preview.rect.x, note.rect.x);
            assert_eq!(preview.rect.y, note.rect.y);
            let mut scene = Scene::default();
            scene.apply(Command::Insert(note.clone())).unwrap();
            let undo: Vec<_> = drag
                .commands()
                .into_iter()
                .map(|command| scene.apply(command).unwrap())
                .collect();
            assert_eq!(scene.get(note.id), Some(&preview));
            for command in undo.into_iter().rev() {
                scene.apply(command).unwrap();
            }
            assert_eq!(scene.get(note.id), Some(&note));
        });
        output.textures_delta.clear();
    }
    proptest::proptest! {
        #[test]
        fn multi_resize_undo_and_checkpoint_preserve_scene(factor in 0.05f32..10.) {
            let mut note = Item::text("wrapped note text", Rect{x:10., y:20., width:120., height:80.}, None);
            note.transform.note_font_size = 24.;
            note.transform.note_wrap = true;
            let mut image = item(150., 60., 0.);
            let original = include_bytes!("../../../../corpus/images/rows.png");
            image.content = moth_scene::Content::Image(moth_io::decode(original, 1).unwrap().hash);
            let items = vec![note, image];
            let mut scene = Scene::default();
            for item in &items { scene.apply(Command::Insert(item.clone())).unwrap(); }
            let mut drag = Resize::start(items.clone(), Camera::default(), [210., 100.], 12.).unwrap();
            drag.update([10. + 200. * factor, 20. + 80. * factor]);
            let mut inverse = Vec::new();
            for command in drag.commands() { inverse.push(scene.apply(command).unwrap()); }
            let payload = moth_format::board::snapshot(&scene).unwrap();
            let mut bytes = Vec::new();
            moth_format::board::initialize(&mut bytes).unwrap();
            moth_format::board::append(&mut bytes, 16, 1, original).unwrap();
            let offset = bytes.len() as u64;
            moth_format::board::append(&mut bytes, offset, 3, &payload).unwrap();
            let loaded = moth_format::board::read(&mut std::io::Cursor::new(bytes)).unwrap();
            for preview in drag.previews() { proptest::prop_assert_eq!(loaded.scene.get(preview.id), Some(&preview)); }
            for command in inverse.into_iter().rev() { scene.apply(command).unwrap(); }
            for item in items { proptest::prop_assert_eq!(scene.get(item.id), Some(&item)); }
        }
    }
}
