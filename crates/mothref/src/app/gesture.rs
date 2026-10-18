// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns the active canvas pointer interaction so cancellation releases it atomically.
use moth_platform::input::Kind;
use moth_scene::{Rect, Uuid};

/// Keeps device kind in the identity because native devices can reuse contact ids.
type PointerId = (Kind, u64);

/// Owns the canvas gesture and its initial geometry until release or cancellation.
#[derive(Default)]
pub(super) enum Gesture {
    #[default]
    Idle,
    Panning {
        last: [f32; 2],
        pointer: PointerId,
    },
    Marquee {
        start: [f32; 2],
        now: [f32; 2],
        pointer: PointerId,
    },
    Moving {
        start: [f32; 2],
        now: [f32; 2],
        pointer: PointerId,
        origin: Vec<(Uuid, Rect)>,
        // Preserve the existing single-frame preview independently of selection order.
        preview: Uuid,
        duplicate: bool,
    },
}
impl Gesture {
    pub(super) fn cancel(&mut self) {
        *self = Self::Idle;
    }
    fn begin(
        p: moth_platform::input::Pointer,
        pan: bool,
        hit: Option<Uuid>,
        scene: &Scene,
        selection: &HashSet<Uuid>,
        duplicate: bool,
    ) -> Self {
        let pointer = (p.kind, p.id);
        if pan {
            Self::Panning {
                last: p.position,
                pointer,
            }
        } else if let Some(preview) = hit {
            let mut items: Vec<_> = selection.iter().filter_map(|id| scene.get(*id)).collect();
            items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
            Self::Moving {
                start: p.position,
                now: p.position,
                pointer,
                origin: items.iter().map(|item| (item.id, item.rect)).collect(),
                preview,
                duplicate,
            }
        } else {
            Self::Marquee {
                start: p.position,
                now: p.position,
                pointer,
            }
        }
    }
    fn update(&mut self, p: moth_platform::input::Pointer, camera: &mut Camera) -> Option<Self> {
        match (&mut *self, p.phase) {
            (Self::Panning { last, .. }, Phase::Move) => {
                camera.origin[0] -= (p.position[0] - last[0]) / camera.zoom;
                camera.origin[1] -= (p.position[1] - last[1]) / camera.zoom;
                *last = p.position;
            }
            (Self::Moving { now, .. } | Self::Marquee { now, .. }, Phase::Move) => {
                *now = p.position
            }
            (_, Phase::Up) => return Some(std::mem::take(self)),
            (_, Phase::Cancel) => self.cancel(),
            _ => {}
        }
        None
    }
    fn commands(
        &self,
        position: [f32; 2],
        camera: &Camera,
        scene: &Scene,
        locked: bool,
    ) -> Option<Vec<moth_scene::Command>> {
        let Self::Moving {
            start,
            origin,
            duplicate,
            ..
        } = self
        else {
            return None;
        };
        let dx = (position[0] - start[0]) / camera.zoom;
        let dy = (position[1] - start[1]) / camera.zoom;
        if !locked && dx.abs() + dy.abs() > 0.5 / camera.zoom {
            let items: Vec<_> = origin.iter().filter_map(|(id, _)| scene.get(*id)).collect();
            Some(arrange::translate(
                &items,
                [dx, dy],
                *duplicate,
                scene.top(),
            ))
        } else {
            None
        }
    }
    pub(super) fn pointer(&self) -> Option<PointerId> {
        match self {
            Self::Idle => None,
            Self::Panning { pointer, .. }
            | Self::Marquee { pointer, .. }
            | Self::Moving { pointer, .. } => Some(*pointer),
        }
    }
    pub(super) fn marquee(&self) -> Option<([f32; 2], [f32; 2])> {
        match self {
            Self::Marquee { start, now, .. } => Some((*start, *now)),
            _ => None,
        }
    }
}

use super::{App, arrange};
use moth_platform::input::{Input, Phase};
use moth_scene::{Camera, Scene};
use std::collections::HashSet;
impl App {
    pub(super) fn pointer(&mut self, input: Input) {
        self.cursor = Some(match &input {
            Input::Zoom { position, .. } | Input::Pan { position, .. } => *position,
            Input::Pointer(p) => p.position,
        });
        if !self.modal.accepts_canvas(self.screen) {
            return;
        }
        match input {
            Input::Pan { position, delta } => {
                if self
                    .board
                    .view
                    .viewport
                    .contains(egui::pos2(position[0], position[1]))
                {
                    self.board.view.camera.origin[0] -= delta[0] / self.board.view.camera.zoom;
                    self.board.view.camera.origin[1] -= delta[1] / self.board.view.camera.zoom;
                }
            }
            Input::Zoom { position, factor } => {
                if self
                    .board
                    .view
                    .viewport
                    .contains(egui::pos2(position[0], position[1]))
                {
                    self.board.view.camera.zoom_at(position, factor);
                }
            }
            Input::Pointer(p) => {
                if p.phase == Phase::Down
                    && self
                        .ctx
                        .layer_id_at(egui::pos2(p.position[0], p.position[1]))
                        .is_some_and(|layer| layer.order != egui::Order::Background)
                {
                    return;
                }
                if p.phase == Phase::Down
                    && !self
                        .board
                        .view
                        .viewport
                        .contains(egui::pos2(p.position[0], p.position[1]))
                {
                    return;
                }
                if (self.tools.picking || self.tools.coordinates)
                    && p.phase == Phase::Down
                    && self.board.document.idle()
                {
                    let world = self.board.view.camera.world(p.position);
                    let id = self
                        .board
                        .document
                        .scene
                        .query(Rect {
                            x: world[0],
                            y: world[1],
                            width: 1. / self.board.view.camera.zoom,
                            height: 1. / self.board.view.camera.zoom,
                        })
                        .into_iter()
                        .rev()
                        .find(|item| {
                            item.blob().is_some()
                                && item.rect.contains_rotated(item.transform.rotation, world)
                        })
                        .map(|item| item.id);
                    if let Some(id) = id {
                        self.persist(moth_platform::boards::Request::Export(
                            if self.tools.coordinates {
                                moth_platform::export::Target::Info { id, point: world }
                            } else {
                                moth_platform::export::Target::Sample {
                                    id,
                                    point: world,
                                    format: match self.tools.color_format {
                                        1 => moth_platform::export::ColorFormat::Rgb,
                                        2 => moth_platform::export::ColorFormat::Hsv,
                                        _ => moth_platform::export::ColorFormat::Hex,
                                    },
                                }
                            },
                        ));
                    }
                    self.tools.picking = false;
                    self.tools.coordinates = false;
                    return;
                }
                if let Some(g) = self.gestures.update(p) {
                    self.gesture.cancel();
                    self.board.view.camera.zoom_at(g.anchor, g.zoom);
                    self.board.view.camera.origin[0] -= g.pan[0] / self.board.view.camera.zoom;
                    self.board.view.camera.origin[1] -= g.pan[1] / self.board.view.camera.zoom;
                    return;
                }
                if self.gestures.active() {
                    self.gesture.cancel();
                    return;
                }
                if self
                    .gesture
                    .pointer()
                    .is_some_and(|owner| owner != (p.kind, p.id))
                {
                    return;
                }
                match (&mut self.gesture, p.phase) {
                    (_, Phase::Down)
                        if self
                            .board
                            .view
                            .viewport
                            .contains(egui::pos2(p.position[0], p.position[1])) =>
                    {
                        let mut hit = None;
                        if !p.pan && !self.board.view.pan && self.board.document.idle() {
                            let world = self.board.view.camera.world(p.position);
                            if let Some(item) = self
                                .board
                                .document
                                .scene
                                .query(Rect {
                                    x: world[0],
                                    y: world[1],
                                    width: 1. / self.board.view.camera.zoom,
                                    height: 1. / self.board.view.camera.zoom,
                                })
                                .into_iter()
                                .rev()
                                // The index only guarantees the rotated envelope, so the
                                // click must still land inside the frame itself.
                                .find(|item| {
                                    item.rect.contains_rotated(item.transform.rotation, world)
                                })
                            {
                                hit = Some(item.id);
                                if !self.board.view.selection.contains(&item.id) {
                                    if !self.ctx.input(|i| i.modifiers.shift) {
                                        self.board.view.selection.clear();
                                    }
                                    self.board.view.selection.insert(item.id);
                                }
                            }
                        }
                        self.gesture = Gesture::begin(
                            p,
                            p.pan || self.board.view.pan,
                            hit,
                            &self.board.document.scene,
                            &self.board.view.selection,
                            self.ctx.input(|i| i.modifiers.alt),
                        );
                    }
                    _ => match self.gesture.update(p, &mut self.board.view.camera) {
                        Some(moving @ Gesture::Moving { .. }) => {
                            let commands = moving.commands(
                                p.position,
                                &self.board.view.camera,
                                &self.board.document.scene,
                                self.tools.locked,
                            );
                            if let Some(commands) = commands {
                                self.persist_batch(
                                    commands.into_iter().map(|c| (c, None)).collect(),
                                );
                            }
                        }
                        Some(Gesture::Marquee { start, .. }) => {
                            let a = self.board.view.camera.world(start);
                            let b = self.board.view.camera.world(p.position);
                            let rect = Rect {
                                x: a[0].min(b[0]),
                                y: a[1].min(b[1]),
                                width: (a[0] - b[0]).abs().max(1. / self.board.view.camera.zoom),
                                height: (a[1] - b[1]).abs().max(1. / self.board.view.camera.zoom),
                            };
                            self.board.view.selection = self
                                .board
                                .document
                                .scene
                                .query(rect)
                                .iter()
                                .map(|i| i.id)
                                .collect();
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_platform::input::Pointer;
    use moth_scene::{Command, Item};

    fn pointer(phase: Phase, position: [f32; 2]) -> Pointer {
        Pointer {
            id: 42,
            kind: Kind::Pen,
            position,
            pressure: 0.5,
            phase,
            pan: false,
        }
    }

    fn fixture() -> (Scene, HashSet<Uuid>, Uuid) {
        let mut scene = Scene::default();
        let mut selection = HashSet::new();
        for x in [0., 30.] {
            let item = Item::text(
                "note",
                Rect {
                    x,
                    y: 0.,
                    width: 20.,
                    height: 20.,
                },
                scene.top(),
            );
            selection.insert(item.id);
            scene.apply(Command::Insert(item)).unwrap();
        }
        let hit = scene
            .items()
            .max_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)))
            .unwrap()
            .id;
        (scene, selection, hit)
    }

    #[test]
    fn moving_captures_the_ordered_selection_and_releases_one_batch() {
        let (scene, selection, hit) = fixture();
        for duplicate in [false, true] {
            let mut camera = Camera::default();
            let mut gesture = Gesture::begin(
                pointer(Phase::Down, [5., 5.]),
                false,
                Some(hit),
                &scene,
                &selection,
                duplicate,
            );
            assert_eq!(gesture.pointer(), Some((Kind::Pen, 42)));
            assert!(
                gesture
                    .update(pointer(Phase::Move, [15., 25.]), &mut camera)
                    .is_none()
            );
            let released = gesture
                .update(pointer(Phase::Up, [15., 25.]), &mut camera)
                .unwrap();
            assert!(matches!(gesture, Gesture::Idle));
            let Gesture::Moving {
                start,
                now,
                origin,
                duplicate: copied,
                ..
            } = &released
            else {
                panic!("moving gesture must survive until release")
            };
            assert_eq!(*start, [5., 5.]);
            assert_eq!(*now, [15., 25.]);
            assert_eq!(*copied, duplicate);
            assert_eq!(origin.len(), 2);
            let items: Vec<_> = origin
                .iter()
                .map(|(id, rect)| {
                    let item = scene.get(*id).unwrap();
                    assert_eq!(item.rect, *rect);
                    item
                })
                .collect();
            assert!(items.windows(2).all(|pair| pair[0].z < pair[1].z));
            let commands = released
                .commands([15., 25.], &camera, &scene, false)
                .unwrap();
            assert!(
                released
                    .commands([15., 25.], &camera, &scene, true)
                    .is_none()
            );
            assert!(
                released
                    .commands([5.1, 5.1], &camera, &scene, false)
                    .is_none()
            );
            assert_eq!(commands.len(), selection.len());
            for (command, item) in commands.iter().zip(items) {
                let rect = match command {
                    Command::Insert(copy) if duplicate => {
                        assert_ne!(copy.id, item.id);
                        copy.rect
                    }
                    Command::SetRect { id, rect } if !duplicate => {
                        assert_eq!(*id, item.id);
                        *rect
                    }
                    _ => panic!("unexpected drag command"),
                };
                assert_eq!(rect.x, item.rect.x + 10.);
                assert_eq!(rect.y, item.rect.y + 20.);
            }
        }
    }

    #[test]
    fn cancel_and_focus_loss_release_every_state_without_committing() {
        let (scene, selection, hit) = fixture();
        for (pan, hit) in [(true, None), (false, None), (false, Some(hit))] {
            for focus_lost in [false, true] {
                let mut gesture = Gesture::begin(
                    pointer(Phase::Down, [5., 5.]),
                    pan,
                    hit,
                    &scene,
                    &selection,
                    false,
                );
                if focus_lost {
                    gesture.cancel();
                } else {
                    assert!(
                        gesture
                            .update(pointer(Phase::Cancel, [20., 20.]), &mut Camera::default())
                            .is_none()
                    );
                }
                assert!(matches!(gesture, Gesture::Idle));
                assert_eq!(gesture.pointer(), None);
                assert!(matches!(
                    gesture.update(pointer(Phase::Up, [20., 20.]), &mut Camera::default()),
                    Some(Gesture::Idle)
                ));
            }
        }
    }

    #[test]
    fn panning_uses_incremental_positions_and_marquee_keeps_its_anchor() {
        let (scene, selection, _) = fixture();
        let mut camera = Camera {
            zoom: 2.,
            ..Camera::default()
        };
        let mut pan = Gesture::begin(
            pointer(Phase::Down, [5., 5.]),
            true,
            None,
            &scene,
            &selection,
            false,
        );
        let origin = camera.origin;
        pan.update(pointer(Phase::Move, [15., 25.]), &mut camera);
        pan.update(pointer(Phase::Move, [25., 35.]), &mut camera);
        assert_eq!(camera.origin, [origin[0] - 10., origin[1] - 15.]);
        let mut marquee = Gesture::begin(
            pointer(Phase::Down, [5., 5.]),
            false,
            None,
            &scene,
            &selection,
            false,
        );
        marquee.update(pointer(Phase::Move, [15., 25.]), &mut camera);
        assert_eq!(marquee.marquee(), Some(([5., 5.], [15., 25.])));
    }
}
