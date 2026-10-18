// SPDX-License-Identifier: GPL-3.0-or-later
//! Camera interpolation is presentation state; scene camera math stays timeless.
use super::*;
use moth_ui::motion::{Ease, Tween};

pub(super) fn zoom_anchor<'a>(
    camera: Camera,
    selected: impl Iterator<Item = &'a Item>,
    fallback: [f32; 2],
) -> [f32; 2] {
    let bounds = selected.fold(None, |bounds: Option<([f32; 2], [f32; 2])>, item| {
        let envelope = item.rect.rotated_envelope(item.transform.rotation);
        let lower = envelope.lower();
        let upper = envelope.upper();
        Some(bounds.map_or((lower, upper), |(min, max)| {
            (
                [min[0].min(lower[0]), min[1].min(lower[1])],
                [max[0].max(upper[0]), max[1].max(upper[1])],
            )
        }))
    });
    bounds.map_or(fallback, |(min, max)| {
        camera.screen([(min[0] + max[0]) / 2., (min[1] + max[1]) / 2.])
    })
}

#[derive(Default)]
pub(super) struct CameraMotion {
    transition: Option<(Camera, Camera, Option<[f32; 2]>, Tween)>,
    previous: Option<Instant>,
    samples: std::collections::VecDeque<(Instant, [f32; 2])>,
    velocity: [f32; 2],
}
impl CameraMotion {
    pub fn stop(&mut self) {
        self.transition = None;
        self.velocity = [0.; 2];
    }
    pub fn transition(&mut self, current: Camera, target: Camera, reduced: bool) {
        self.stop();
        if !reduced && (current.origin != target.origin || current.zoom != target.zoom) {
            self.transition = Some((
                current,
                target,
                None,
                Tween::new(
                    0.,
                    1.,
                    Instant::now(),
                    Duration::from_millis(250),
                    Ease::OutCubic,
                ),
            ));
        }
    }
    pub fn zoom(&mut self, current: &mut Camera, anchor: [f32; 2], factor: f32, reduced: bool) {
        let mut target = self
            .transition
            .as_ref()
            .map_or(*current, |(_, target, _, _)| *target);
        // Rebase the target at the new anchor, retaining accumulated wheel zoom.
        let factor = target.zoom / current.zoom * factor;
        target = *current;
        target.zoom_at(anchor, factor);
        self.stop();
        if reduced {
            *current = target;
        } else {
            self.transition = Some((
                *current,
                target,
                Some(anchor),
                Tween::new(
                    0.,
                    1.,
                    Instant::now(),
                    Duration::from_millis(180),
                    Ease::OutCubic,
                ),
            ));
        }
    }
    pub fn pan_sample(&mut self, delta: [f32; 2], now: Instant) {
        self.samples.push_back((now, delta));
        while self.samples.len() > 64
            || self
                .samples
                .front()
                .is_some_and(|(time, _)| now.duration_since(*time).as_millis() > 80)
        {
            self.samples.pop_front();
        }
    }
    pub fn release(&mut self, now: Instant, reduced: bool) {
        self.velocity = [0.; 2];
        if !reduced
            && self.samples.len() > 1
            && self
                .samples
                .back()
                .is_some_and(|(last, _)| now.duration_since(*last).as_millis() <= 60)
        {
            let dt = now
                .duration_since(
                    self.samples
                        .front()
                        .expect("nonempty samples checked above")
                        .0,
                )
                .as_secs_f32()
                .max(0.001);
            for (_, delta) in self.samples.iter().skip(1) {
                for (v, d) in self.velocity.iter_mut().zip(delta) {
                    *v += d / dt;
                }
            }
        }
        self.samples.clear();
    }
    pub fn clear_samples(&mut self) {
        self.samples.clear();
    }
    pub fn step(&mut self, current: &mut Camera, now: Instant, reduced: bool) -> bool {
        let dt = self
            .previous
            .replace(now)
            .map_or(0., |last| now.duration_since(last).as_secs_f32().min(0.05));
        if let Some((from, target, anchor, tween)) = &self.transition {
            *current = interpolate(
                *from,
                *target,
                *anchor,
                if reduced { 1. } else { tween.value(now) },
            );
            if reduced || tween.done(now) {
                *current = *target;
                self.transition = None;
            }
        }
        if reduced {
            self.velocity = [0.; 2];
        }
        let decay = (-12. * dt).exp();
        for (origin, velocity) in current.origin.iter_mut().zip(&mut self.velocity) {
            *origin -= *velocity * (1. - decay) / 12. / current.zoom;
            *velocity *= decay;
            if velocity.abs() < 1. {
                *velocity = 0.;
            }
        }
        self.transition.is_some() || self.velocity != [0.; 2]
    }
}
fn interpolate(from: Camera, to: Camera, anchor: Option<[f32; 2]>, t: f32) -> Camera {
    if t >= 1. {
        return to;
    }
    let zoom = (from.zoom.ln() + (to.zoom.ln() - from.zoom.ln()) * t).exp();
    let origin = if let Some(anchor) = anchor {
        let world = from.world(anchor);
        [world[0] - anchor[0] / zoom, world[1] - anchor[1] / zoom]
    } else {
        [
            from.origin[0] + (to.origin[0] - from.origin[0]) * t,
            from.origin[1] + (to.origin[1] - from.origin[1]) * t,
        ]
    };
    Camera { origin, zoom }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zoom_uses_the_whole_selection_and_keeps_its_center_fixed() {
        let camera = Camera {
            origin: [20., -10.],
            zoom: 2.,
        };
        let items: Vec<_> = [(0., 0.), (200., 0.), (0., 100.), (200., 100.)]
            .into_iter()
            .map(|(x, y)| {
                Item::image(
                    moth_scene::BlobId::from_bytes([1; 32]),
                    Rect {
                        x,
                        y,
                        width: 40.,
                        height: 60.,
                    },
                    None,
                )
            })
            .collect();
        let fallback = [999., 888.];
        assert_eq!(zoom_anchor(camera, std::iter::empty(), fallback), fallback);
        let anchor = zoom_anchor(camera, items.iter(), fallback);
        assert_eq!(anchor, camera.screen([120., 80.]));
        let mut target = camera;
        target.zoom_at(anchor, 2.);
        for frame in 0..=100 {
            let current = interpolate(camera, target, Some(anchor), frame as f32 / 100.);
            let center = current.screen([120., 80.]);
            assert!((center[0] - anchor[0]).abs() < 0.01);
            assert!((center[1] - anchor[1]).abs() < 0.01);
        }
        let mut rotated = items[0].clone();
        rotated.transform.rotation = std::f32::consts::FRAC_PI_2;
        assert_eq!(
            zoom_anchor(camera, std::iter::once(&rotated), fallback),
            camera.screen([20., 30.]),
        );
    }
    #[test]
    fn inertia_expires_and_stationary_release_does_not_glide() {
        let now = Instant::now();
        let mut motion = CameraMotion::default();
        let mut camera = Camera::default();
        motion.pan_sample([10., 0.], now);
        motion.pan_sample([10., 0.], now + Duration::from_millis(20));
        motion.release(now + Duration::from_millis(100), false);
        assert_eq!(motion.velocity, [0.; 2]);
        motion.pan_sample([10., 0.], now);
        motion.pan_sample([10., 0.], now + Duration::from_millis(20));
        motion.release(now + Duration::from_millis(21), false);
        assert!(motion.velocity[0] > 0.);
        for frame in 1..=240 {
            motion.step(
                &mut camera,
                now + Duration::from_secs_f32(frame as f32 / 120.),
                false,
            );
        }
        assert!(!motion.step(&mut camera, now + Duration::from_secs(3), false));
    }
    #[test]
    fn wheel_anchor_stays_fixed_and_fit_finishes_exactly() {
        let from = Camera {
            origin: [10., -40.],
            zoom: 0.02,
        };
        let anchor = [321., 153.];
        let world = from.world(anchor);
        let mut to = from;
        to.zoom_at(anchor, 100.);
        for n in 0..=100 {
            let camera = interpolate(from, to, Some(anchor), n as f32 / 100.);
            let screen = camera.screen(world);
            assert!((screen[0] - anchor[0]).abs() < 0.5 && (screen[1] - anchor[1]).abs() < 0.5);
        }
        let end = interpolate(from, to, None, 1.);
        assert_eq!(end.origin, to.origin);
        assert_eq!(end.zoom, to.zoom);
        let mut motion = CameraMotion::default();
        let mut current = from;
        motion.zoom(&mut current, anchor, 100., true);
        assert_eq!(current.origin, to.origin);
        assert!(!motion.step(&mut current, Instant::now(), true));
    }
}
