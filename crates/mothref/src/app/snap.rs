// SPDX-License-Identifier: GPL-3.0-or-later
//! Rotation quantization, and snapping moved items to the grid and to other items.
use moth_scene::Rect;

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

/// A line drawn while snapping shows what the moved items lined up with: world
/// endpoints of a vertical or horizontal segment.
pub type Guide = [[f32; 2]; 2];

/// How a move of `bounds` by `delta` lines up: the delta adjusted so an edge or the
/// center of `bounds` lands on the nearest edge or center of a `target`, or on a grid
/// line, when one is within `threshold` (world units) on that axis; plus guides for
/// the item alignments. Each axis snaps on its own; items win ties with the grid.
pub fn moving(
    bounds: Rect,
    delta: [f32; 2],
    targets: &[Rect],
    grid: Option<f32>,
    threshold: f32,
) -> ([f32; 2], Vec<Guide>) {
    let mut out = delta;
    let mut guides = Vec::new();
    for axis in 0..2 {
        let (start, size) = if axis == 0 {
            (bounds.x, bounds.width)
        } else {
            (bounds.y, bounds.height)
        };
        let moved = start + delta[axis];
        let marks = [moved, moved + size / 2., moved + size];
        // (distance, adjustment, from an item)
        let mut best: Option<(f32, f32, bool)> = None;
        let mut consider = |adjust: f32, item: bool| {
            let distance = adjust.abs();
            if distance <= threshold
                && best.is_none_or(|(d, _, was_item)| {
                    distance < d - 1e-4 || (distance <= d + 1e-4 && item && !was_item)
                })
            {
                best = Some((distance, adjust, item));
            }
        };
        for target in targets {
            let (t, ts) = if axis == 0 {
                (target.x, target.width)
            } else {
                (target.y, target.height)
            };
            for line in [t, t + ts / 2., t + ts] {
                for mark in marks {
                    consider(line - mark, true);
                }
            }
        }
        if let Some(size) = grid.filter(|size| size.is_finite() && *size > 0.) {
            for mark in [marks[0], marks[2]] {
                consider((mark / size).round() * size - mark, false);
            }
        }
        let Some((_, adjust, item)) = best else {
            continue;
        };
        out[axis] = delta[axis] + adjust;
        if !item {
            continue;
        }
        // Guides along every target line the snapped marks now touch.
        let snapped = [marks[0] + adjust, marks[1] + adjust, marks[2] + adjust];
        let moved_span = if axis == 0 {
            (bounds.y + out[1], bounds.y + out[1] + bounds.height)
        } else {
            (bounds.x + out[0], bounds.x + out[0] + bounds.width)
        };
        for line in snapped {
            let mut span: Option<(f32, f32)> = None;
            for target in targets {
                let (t, ts, o, os) = if axis == 0 {
                    (target.x, target.width, target.y, target.height)
                } else {
                    (target.y, target.height, target.x, target.width)
                };
                if [t, t + ts / 2., t + ts]
                    .iter()
                    .any(|v| (v - line).abs() <= 1e-3 * (1. + line.abs()))
                {
                    let (a, b) = span.unwrap_or(moved_span);
                    span = Some((a.min(o).min(moved_span.0), b.max(o + os).max(moved_span.1)));
                }
            }
            if let Some((a, b)) = span {
                guides.push(if axis == 0 {
                    [[line, a], [line, b]]
                } else {
                    [[a, line], [b, line]]
                });
            }
        }
    }
    (out, guides)
}

/// Snap displayed bounds on both axes, preserving offsets between selected items.
pub(super) fn grid_delta(bounds: moth_scene::Rect, delta: [f32; 2], step: f32) -> [f32; 2] {
    let origin = [bounds.x, bounds.y];
    std::array::from_fn(|axis| snap_angle(origin[axis] + delta[axis], step) - origin[axis])
}

/// Preview and release use the same endpoint; a click leaves an off-grid item put.
pub(super) fn grid_point(
    bounds: moth_scene::Rect,
    start: [f32; 2],
    point: [f32; 2],
    zoom: f32,
    step: f32,
) -> [f32; 2] {
    if point == start {
        return point;
    }
    let delta = grid_delta(
        bounds,
        [(point[0] - start[0]) / zoom, (point[1] - start[1]) / zoom],
        step,
    );
    [start[0] + delta[0] * zoom, start[1] + delta[1] * zoom]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
    #[test]
    fn moves_snap_edges_and_centers_to_items_and_the_grid() {
        let wide = rect(0., 0., 40., 10.);
        let moving_rect = rect(0., 0., 10., 10.);
        let target = rect(100., 50., 20., 30.);
        // Left edge 3 units from the target's right edge (120): snaps, with a guide.
        let (delta, guides) = moving(wide, [117., 0.], &[target], None, 4.);
        assert_eq!(delta, [120., 0.]);
        assert_eq!(guides, vec![[[120., 0.], [120., 80.]]]);
        // Too far: nothing changes.
        let (delta, guides) = moving(moving_rect, [140., 200.], &[target], None, 4.);
        assert_eq!(delta, [140., 200.]);
        assert!(guides.is_empty());
        // Centers align: 5 + dy lands on 65, the target's center.
        let (delta, _) = moving(moving_rect, [300., 61.], &[target], None, 4.);
        assert_eq!(delta[1], 60.);
        // Grid: the nearest line for either edge, no guides.
        let (delta, guides) = moving(moving_rect, [33., 47.], &[], Some(16.), 4.);
        assert_eq!(delta, [32., 48.]);
        assert!(guides.is_empty());
        // An item at the same distance as a grid line wins.
        let (delta, guides) = moving(
            moving_rect,
            [1., 500.],
            &[rect(-2., 0., 4., 4.)],
            Some(10.),
            3.,
        );
        assert_eq!(delta[0], 0.);
        assert!(!guides.is_empty());
    }
    #[test]
    fn both_axes_match_the_commit_at_different_zooms_without_moving_on_click() {
        let mut item = moth_scene::Item::text(
            "reference",
            moth_scene::Rect {
                x: 13.,
                y: -27.,
                width: 105.,
                height: 97.,
            },
            None,
        );
        item.transform.rotation = 0.4;
        let bounds = moth_scene::footprint(&item);
        for zoom in [0.35, 1., 2.] {
            let start = [80., 90.];
            assert_eq!(grid_point(bounds, start, start, zoom, 100.), start);
            for movement in [[36., 0.], [0., -31.], [36., -31.]] {
                let point = [start[0] + movement[0] * zoom, start[1] + movement[1] * zoom];
                let preview = grid_point(bounds, start, point, zoom, 100.);
                let delta = grid_delta(bounds, movement, 100.);
                for axis in 0..2 {
                    assert!(((preview[axis] - start[axis]) / zoom - delta[axis]).abs() < 0.001);
                }
            }
        }
    }

    #[test]
    fn grid_snaps_both_axes_and_negative_coordinates_by_displayed_bounds() {
        let bounds = moth_scene::Rect {
            x: 13.,
            y: -27.,
            width: 105.,
            height: 97.,
        };
        let delta = grid_delta(bounds, [36., -31.], 100.);
        assert_eq!(delta, [-13., -73.]);
        assert_eq!([bounds.x + delta[0], bounds.y + delta[1]], [0., -100.]);
    }

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
