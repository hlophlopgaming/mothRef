// SPDX-License-Identifier: GPL-3.0-or-later
//! View-only motion; callers request another frame only while motion is active.
use std::time::{Duration, Instant};

/// Frame-rate independent convergence without overshoot.
pub fn approach(current: f32, target: f32, rate: f32, dt: f32) -> f32 {
    current + (target - current) * -(-rate.max(0.) * dt.max(0.)).exp_m1()
}

/// Short transitions favor a quick response and a gentle stop.
#[derive(Clone, Copy)]
pub enum Ease {
    /// Constant speed.
    Linear,
    /// Gentle stop.
    OutCubic,
    /// Fast initial response.
    OutExpo,
    /// Optional overshoot, controlled by the supplied strength.
    OutBack(f32),
}
impl Ease {
    fn sample(self, t: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::OutCubic => 1. - (1. - t).powi(3),
            Self::OutExpo => {
                if t >= 1. {
                    1.
                } else {
                    1. - 2_f32.powf(-10. * t)
                }
            }
            Self::OutBack(s) => 1. + (s + 1.) * (t - 1.).powi(3) + s * (t - 1.).powi(2),
        }
    }
}

/// Scalar interpolation with exact endpoints, including zero duration.
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
    ease: Ease,
}
impl Tween {
    /// Creates a transition; use zero duration for reduced motion.
    pub fn new(from: f32, to: f32, start: Instant, duration: Duration, ease: Ease) -> Self {
        Self {
            from,
            to,
            start,
            duration,
            ease,
        }
    }
    /// Samples without extrapolating beyond the endpoint.
    pub fn value(&self, now: Instant) -> f32 {
        if self.done(now) {
            return self.to;
        }
        let t =
            now.saturating_duration_since(self.start).as_secs_f32() / self.duration.as_secs_f32();
        self.from + (self.to - self.from) * self.ease.sample(t)
    }
    /// Finished transitions no longer need repainting.
    pub fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= self.duration
    }
}

/// Analytic critically damped spring, stable across refresh rates.
pub struct Spring {
    /// Displayed value.
    pub value: f32,
    /// Units per second.
    pub velocity: f32,
    /// Destination.
    pub target: f32,
    /// Squared angular frequency, in seconds.
    pub stiffness: f32,
}
impl Spring {
    /// Advances using the exact solution rather than Euler integration.
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0., 0.05);
        let w = self.stiffness.max(0.).sqrt();
        let x = self.value - self.target;
        let c = self.velocity + w * x;
        let decay = (-w * dt).exp();
        self.value = self.target + (x + c * dt) * decay;
        self.velocity = (self.velocity - w * c * dt) * decay;
        if self.settled() {
            self.value = self.target;
            self.velocity = 0.;
        }
    }
    /// Exact underdamped solution for a card hanging from its grab point.
    pub fn step_damped(&mut self, dt: f32, damping: f32) {
        if damping >= 1. {
            self.step(dt);
            return;
        }
        let dt = dt.clamp(0., 0.05);
        let w = self.stiffness.max(0.).sqrt();
        let a = w * damping.clamp(0., 0.999);
        let b = (w * w - a * a).sqrt();
        if b <= f32::EPSILON {
            return;
        }
        let x = self.value - self.target;
        let c = (self.velocity + a * x) / b;
        let (sin, cos) = (b * dt).sin_cos();
        let decay = (-a * dt).exp();
        self.value = self.target + decay * (x * cos + c * sin);
        self.velocity = decay * ((-a * x + b * c) * cos + (-a * c - b * x) * sin);
        if self.settled() {
            self.value = self.target;
            self.velocity = 0.;
        }
    }
    /// Snapping negligible tails lets the event loop sleep.
    pub fn settled(&self) -> bool {
        (self.value - self.target).abs() < 0.0001 && self.velocity.abs() < 0.0001
    }
}

/// Frame-local repaint bookkeeping and shared reduced-motion policy.
#[derive(Default)]
pub struct Motion {
    reduced: bool,
    active: bool,
    previous: Option<Instant>,
}
impl Motion {
    /// Starts a frame; long pauses cannot produce a large integration step.
    pub fn begin(&mut self, now: Instant, reduced: bool) -> f32 {
        self.reduced = reduced;
        self.active = false;
        self.previous.replace(now).map_or(0., |last| {
            now.saturating_duration_since(last).as_secs_f32().min(0.05)
        })
    }
    /// Marks unfinished visual work, except when reduced motion is enabled.
    pub fn track(&mut self, unfinished: bool) {
        self.active |= unfinished && !self.reduced;
    }
    /// Whether the caller should request another repaint.
    pub fn active(&self) -> bool {
        self.active
    }
    /// Applies the accessibility policy to every transition duration.
    pub fn duration(&self, duration: Duration) -> Duration {
        if self.reduced {
            Duration::ZERO
        } else {
            duration
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn approach_is_monotonic_and_bounded() {
        let mut x = 0.;
        for _ in 0..500 {
            let next = approach(x, 1., 15., 1. / 60.);
            assert!(next >= x && next <= 1.);
            x = next;
        }
        assert!((x - 1.).abs() < 0.00001);
    }
    #[test]
    fn spring_settles_at_all_refresh_rates() {
        for hz in [30, 60, 144, 240] {
            let mut spring = Spring {
                value: 0.,
                velocity: 0.,
                target: 1.,
                stiffness: 400.,
            };
            for _ in 0..hz * 2 {
                spring.step(1. / hz as f32);
            }
            assert!(spring.settled());
            assert_eq!(spring.value, 1.);
        }
    }
    #[test]
    fn tween_finishes_exactly_and_reduced_motion_is_instant() {
        let now = Instant::now();
        let mut motion = Motion::default();
        motion.begin(now, true);
        motion.track(true);
        assert!(!motion.active());
        for ease in [
            Ease::Linear,
            Ease::OutCubic,
            Ease::OutExpo,
            Ease::OutBack(1.7),
        ] {
            let tween = Tween::new(
                2.,
                7.,
                now,
                motion.duration(Duration::from_millis(140)),
                ease,
            );
            assert!(tween.done(now));
            assert_eq!(tween.value(now), 7.);
            let tween = Tween::new(2., 7., now, Duration::from_millis(140), ease);
            assert_eq!(tween.value(now + Duration::from_secs(1)), 7.);
        }
    }
}
