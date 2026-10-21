// SPDX-License-Identifier: GPL-3.0-or-later
//! Affine tile coverage is shared by viewport requests and ordered drawing.
use super::{Content, LOCAL, content};
use moth_io::tiles::{CORE, Tile};
use moth_scene::{Camera, Item};

/// Which images are tiled and how much coarser than the screen their tiles are drawn.
#[derive(Clone, Copy)]
pub(super) struct Detail {
    /// Extra levels of coarsening when the tile working set exceeds video memory.
    pub bias: u32,
    /// Images whose longer side exceeds this are drawn from tiles.
    pub threshold: u32,
}
pub(super) fn visible(
    item: &Item,
    original: [u32; 2],
    camera: Camera,
    screen: [f32; 2],
    clip: [f32; 4],
    scale: f32,
    detail: Detail,
) -> Vec<Tile> {
    let Detail { bias, threshold } = detail;
    if original[0].max(original[1]) <= threshold.max(moth_io::tiles::SIDE) {
        return Vec::new();
    }
    let density = (item.rect.width / original[0] as f32).max(item.rect.height / original[1] as f32)
        * item.transform.content_scale
        * camera.zoom
        * scale;
    let level = if density >= 1. {
        0
    } else {
        (-density.log2()).floor().clamp(1., 15.) as u32
    };
    let level = level.saturating_add(bias).min(15);
    let size = original.map(|v| v.div_ceil(1 << level));
    if size[0].max(size[1]) <= moth_io::tiles::SIDE {
        return Vec::new();
    }
    let shape = content(item, original, camera, screen);
    let a = shape.corners[0];
    let u = [shape.corners[1][0] - a[0], shape.corners[1][1] - a[1]];
    let v = [shape.corners[2][0] - a[0], shape.corners[2][1] - a[1]];
    let det = u[0] * v[1] - u[1] * v[0];
    if !det.is_finite() || det.abs() < f32::EPSILON {
        return Vec::new();
    }
    let mut min = [1f32; 2];
    let mut max = [0f32; 2];
    for p in [
        [clip[0], clip[1]],
        [clip[2], clip[1]],
        [clip[0], clip[3]],
        [clip[2], clip[3]],
    ] {
        let delta = [
            p[0] / screen[0] * 2. - 1. - a[0],
            1. - p[1] / screen[1] * 2. - a[1],
        ];
        let mut local = [
            (delta[0] * v[1] - delta[1] * v[0]) / det,
            (u[0] * delta[1] - u[1] * delta[0]) / det,
        ];
        for axis in 0..2 {
            if item.transform.flip[axis] {
                local[axis] = 1. - local[axis];
            }
            min[axis] = min[axis].min(local[axis]);
            max[axis] = max[axis].max(local[axis]);
        }
    }
    let start = [0, 1].map(|axis| {
        ((min[axis].clamp(0., 1.) * original[axis] as f32 / (1 << level) as f32) / CORE as f32)
            .floor() as u32
    });
    let end = [0, 1].map(|axis| {
        ((max[axis].clamp(0., 1.) * original[axis] as f32 / (1 << level) as f32) / CORE as f32)
            .ceil() as u32
    });
    let mut tiles = Vec::new();
    for y in start[1]..end[1].min(size[1].div_ceil(CORE)) {
        for x in start[0]..end[0].min(size[0].div_ceil(CORE)) {
            let tile = Tile { level, x, y };
            let piece = piece(&shape, range(tile, original), item.transform.flip);
            if piece.frame.iter().all(|p| p[0] < 0.)
                || piece.frame.iter().all(|p| p[0] > 1.)
                || piece.frame.iter().all(|p| p[1] < 0.)
                || piece.frame.iter().all(|p| p[1] > 1.)
            {
                continue;
            }
            tiles.push(tile);
        }
    }
    tiles
}
pub(super) fn range(tile: Tile, original: [u32; 2]) -> [f32; 4] {
    let [x, y, w, h] = tile
        .bounds(original)
        .expect("visible tile was clipped to the source");
    let step = (1 << tile.level) as f32;
    [
        x as f32 * step / original[0] as f32,
        y as f32 * step / original[1] as f32,
        ((x + w) as f32 * step / original[0] as f32).min(1.),
        ((y + h) as f32 * step / original[1] as f32).min(1.),
    ]
}
pub(super) fn piece(shape: &Content, range: [f32; 4], flip: [bool; 2]) -> Content {
    let mut corners = [[0.; 2]; 4];
    let mut frame = corners;
    for (index, local) in LOCAL.iter().enumerate() {
        let mut uv = [
            range[0] + local[0] * (range[2] - range[0]),
            range[1] + local[1] * (range[3] - range[1]),
        ];
        for axis in 0..2 {
            if flip[axis] {
                uv[axis] = 1. - uv[axis];
            }
        }
        for axis in 0..2 {
            corners[index][axis] = shape.corners[0][axis]
                + uv[0] * (shape.corners[1][axis] - shape.corners[0][axis])
                + uv[1] * (shape.corners[2][axis] - shape.corners[0][axis]);
            frame[index][axis] = shape.frame[0][axis]
                + uv[0] * (shape.frame[1][axis] - shape.frame[0][axis])
                + uv[1] * (shape.frame[2][axis] - shape.frame[0][axis]);
        }
    }
    Content {
        corners,
        frame,
        region: [0.; 4],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::{BlobId, Rect};
    #[test]
    fn full_resolution_waits_for_one_to_one_and_other_levels_use_fixed_tiles() {
        let item = Item::image(
            BlobId::from_bytes([1; 32]),
            Rect {
                x: 0.,
                y: 0.,
                width: 4096.,
                height: 4096.,
            },
            None,
        );
        for (zoom, scale, level) in [(1., 1., 0), (0.75, 1., 1), (0.1, 1., 3), (0.75, 2., 0)] {
            let tiles = visible(
                &item,
                [4096; 2],
                Camera {
                    origin: [0.; 2],
                    zoom,
                },
                [256.; 2],
                [0., 0., 256., 256.],
                scale,
                0,
            );
            assert!(!tiles.is_empty());
            assert!(tiles.iter().all(|tile| tile.level == level));
            assert!(tiles.len() <= 9);
        }
        assert!(
            visible(
                &item,
                [4096; 2],
                Camera {
                    origin: [0.; 2],
                    zoom: 0.01
                },
                [256.; 2],
                [0., 0., 256., 256.],
                1.,
                0,
            )
            .is_empty(),
            "the small whole-image fallback needs no detail tiles"
        );
    }
}
