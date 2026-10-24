// SPDX-License-Identifier: GPL-3.0-or-later
//! Rotation quantization using arithmetic rounding.

/// Round to the nearest step in radians; halfway values round away from zero.
/// Invalid inputs or an unrepresentable result leave the angle unchanged.
pub fn snap_angle(angle: f32, step: f32) -> f32 {
    if !angle.is_finite() || !step.is_finite() || step <= 0. {
        return angle;
    }
    // Widen before division so a tiny positive step cannot overflow the quotient.
    let snapped = ((f64::from(angle) / f64::from(step)).round() * f64::from(step)) as f32;
    if snapped.is_finite() { snapped } else { angle }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_both_directions_and_half_steps_away_from_zero() {
        for (angle, expected) in [
            (0., 0.),
            (0.12, 0.),
            (0.13, 0.25),
            (0.125, 0.25),
            (-0.12, 0.),
            (-0.13, -0.25),
            (-0.125, -0.25),
            (7.125, 7.25),
        ] {
            assert_eq!(snap_angle(angle, 0.25), expected);
        }
    }

    #[test]
    fn invalid_inputs_and_extreme_ratios_do_not_create_infinity() {
        for step in [0., -1., f32::NAN, f32::INFINITY] {
            assert_eq!(snap_angle(1.25, step), 1.25);
        }
        assert!(snap_angle(f32::NAN, 0.25).is_nan());
        assert_eq!(snap_angle(f32::INFINITY, 0.25), f32::INFINITY);
        assert_eq!(snap_angle(f32::MAX, f32::MIN_POSITIVE), f32::MAX);
        assert_eq!(snap_angle(f32::MAX, f32::MAX * 0.6), f32::MAX);
    }
}
