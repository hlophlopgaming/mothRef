// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, disposable drag presentation. Board commands never see these transforms.
use super::*;
use moth_ui::motion::{Ease, Motion, Spring, Tween};
const MAX_ITEMS: usize = 300;
const SWAY_GAIN: f32 = 0.012 * std::f32::consts::PI / 180.;
const SWAY_MAX: f32 = 6. * std::f32::consts::PI / 180.;
const SWAY_STIFFNESS: f32 = 400.;
const SWAY_DAMPING: f32 = 0.55;

pub(super) struct DragMotion {
    pub frames: Motion,
    spring: Spring,
    samples: std::collections::VecDeque<(Instant, f32)>,
    last_sample: Option<Instant>,
    pub anchor: [f32; 2],
    limit: f32,
    started: Instant,
    returning: Option<(Vec<Item>, [f32; 2], Tween)>,
}
impl Default for DragMotion {
    fn default() -> Self {
        Self {
            frames: Motion::default(),
            spring: Spring {
                value: 0.,
                velocity: 0.,
                target: 0.,
                stiffness: SWAY_STIFFNESS,
            },
            samples: Default::default(),
            last_sample: None,
            anchor: [0.; 2],
            limit: SWAY_MAX,
            started: Instant::now(),
            returning: None,
        }
    }
}
impl DragMotion {
    pub fn start(&mut self, anchor: [f32; 2], diag: f32) {
        self.spring.value = 0.;
        self.spring.velocity = 0.;
        self.spring.target = 0.;
        self.anchor = anchor;
        self.limit = SWAY_MAX * (320. / diag.max(1.)).clamp(0.25, 1.);
        self.started = Instant::now();
        self.samples.clear();
        self.last_sample = None;
        self.returning = None;
    }
    pub fn sample(&mut self, x: f32, now: Instant) {
        self.samples.push_back((now, x));
        while self.samples.len() > 2
            && (self.samples.len() > 64
                || now.duration_since(self.samples[1].0).as_secs_f32() > 0.05)
        {
            self.samples.pop_front();
        }
        if let Some(&(start, first)) = self.samples.front() {
            let dt = now.duration_since(start).as_secs_f32();
            if dt > 0.001 {
                self.spring.target = (-(x - first) / dt * SWAY_GAIN).clamp(-self.limit, self.limit);
            }
        }
        self.last_sample = Some(now);
    }
    pub fn step(&mut self, now: Instant, reduced: bool, moving: bool) {
        let dt = self.frames.begin(now, reduced);
        if reduced {
            self.spring.value = 0.;
            self.spring.velocity = 0.;
            self.spring.target = 0.;
            self.returning = None;
            return;
        }
        if !moving
            || self
                .last_sample
                .is_none_or(|last| now.duration_since(last).as_millis() > 60)
        {
            self.spring.target = 0.;
        }
        self.spring.step_damped(dt, SWAY_DAMPING);
        self.spring.value = self.spring.value.clamp(-self.limit, self.limit);
        if self.returning.as_ref().is_some_and(|(_, _, t)| t.done(now)) {
            self.returning = None;
        }
        self.frames.track(
            !self.spring.settled()
                || self.returning.is_some()
                || (moving && now.duration_since(self.started).as_millis() < 120),
        );
    }
    pub fn decorate(&self, item: &mut Item, anchor: [f32; 2], reduced: bool, moving: bool) {
        if reduced {
            return;
        }
        let angle = self.spring.value;
        let (sin, cos) = angle.sin_cos();
        let center = [
            item.rect.x + item.rect.width / 2.,
            item.rect.y + item.rect.height / 2.,
        ];
        let d = [center[0] - anchor[0], center[1] - anchor[1]];
        let lift = if moving {
            1. + 0.02 * (1. - (1. - (self.started.elapsed().as_secs_f32() / 0.12).min(1.)).powi(3))
        } else {
            1.
        };
        item.rect.width *= lift;
        item.rect.height *= lift;
        item.rect.x = anchor[0] + (d[0] * cos - d[1] * sin) * lift - item.rect.width / 2.;
        item.rect.y = anchor[1] + (d[0] * sin + d[1] * cos) * lift - item.rect.height / 2.;
        item.transform.rotation += angle;
    }
    pub fn cancel(&mut self, items: Vec<Item>, delta: [f32; 2], reduced: bool) {
        self.spring.target = 0.;
        if !reduced && items.len() <= MAX_ITEMS {
            self.returning = Some((
                items,
                delta,
                Tween::new(
                    1.,
                    0.,
                    Instant::now(),
                    Duration::from_millis(150),
                    Ease::OutCubic,
                ),
            ));
        }
    }
    pub fn release(&mut self, commands: &[Command], scene: &Scene, delta: [f32; 2], reduced: bool) {
        self.spring.target = 0.;
        if reduced || commands.len() > MAX_ITEMS {
            return;
        }
        let items = commands
            .iter()
            .filter_map(|command| match command {
                Command::Insert(item) => Some(item.clone()),
                Command::SetRect { id, rect } => scene.get(*id).map(|source| {
                    let mut item = source.clone();
                    item.rect = *rect;
                    item
                }),
                _ => None,
            })
            .collect();
        self.anchor[0] += delta[0];
        self.anchor[1] += delta[1];
        self.returning = Some((
            items,
            [0.; 2],
            Tween::new(
                1.,
                0.,
                Instant::now(),
                Duration::from_millis(400),
                Ease::OutCubic,
            ),
        ));
    }
    pub fn returning(&self, now: Instant, reduced: bool) -> Vec<Item> {
        let Some((items, delta, tween)) = &self.returning else {
            return Vec::new();
        };
        let t = tween.value(now);
        items
            .iter()
            .map(|source| {
                let mut item = source.clone();
                item.rect.x += delta[0] * t;
                item.rect.y += delta[1] * t;
                self.decorate(
                    &mut item,
                    [self.anchor[0] + delta[0] * t, self.anchor[1] + delta[1] * t],
                    reduced,
                    false,
                );
                item
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sway_is_bounded_settles_and_never_changes_source_rotation() {
        for hz in [60, 144, 240] {
            let mut motion = DragMotion::default();
            motion.spring.velocity = 20.;
            for _ in 0..hz * 2 {
                motion.spring.step_damped(1. / hz as f32, SWAY_DAMPING);
                motion.spring.value = motion.spring.value.clamp(-motion.limit, motion.limit);
                assert!(motion.spring.value.abs() <= motion.limit);
            }
            assert!(motion.spring.settled());
            let source = Item::text(
                "note",
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 10.,
                    height: 20.,
                },
                None,
            );
            let mut preview = source.clone();
            motion.spring.value = 0.05;
            motion.decorate(&mut preview, [0., 0.], false, false);
            assert_eq!(source.transform.rotation, 0.);
            let mut reduced = source.clone();
            motion.decorate(&mut reduced, [0., 0.], true, true);
            assert_eq!(reduced, source);
        }
    }
}
