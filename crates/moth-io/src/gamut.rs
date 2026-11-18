// SPDX-License-Identifier: GPL-3.0-or-later
//! Measures which colors an image uses for a color gamut card, from a small preview.
use moth_scene::{GAMUT_COLORS, GAMUT_HUES, GAMUT_RINGS, Gamut, Uuid};

/// Longest side of the preview a gamut is measured on: enough to find every color
/// region, small enough to take a few milliseconds for any image.
pub const SIDE: u32 = 128;

pub use moth_scene::hue_saturation;

/// Measures straight RGBA pixels. Transparent pixels are ignored; an image with no
/// opaque pixel gives an empty grid and no colors.
pub fn measure(rgba: &[u8], image: Option<Uuid>) -> Gamut {
    tally(rgba.as_chunks::<4>().0.iter().copied(), image)
}
/// Measures what `item` shows of its image: only the pixels inside its frame, after
/// crop, rotation and flip. `rgba` is a straight-alpha preview of the original of any
/// size; the result records [`Gamut::basis_of`] the item.
pub fn measure_item(rgba: &[u8], width: u32, height: u32, item: &moth_scene::Item) -> Gamut {
    const SAMPLES: u32 = 160;
    let (w, h) = (width as usize, height as usize);
    let rect = item.rect;
    let (sin, cos) = item.transform.rotation.sin_cos();
    let center = [rect.x + rect.width / 2., rect.y + rect.height / 2.];
    let pixels = (0..SAMPLES * SAMPLES).filter_map(|i| {
        let u = ((i % SAMPLES) as f32 + 0.5) / SAMPLES as f32 - 0.5;
        let v = ((i / SAMPLES) as f32 + 0.5) / SAMPLES as f32 - 0.5;
        let local = [u * rect.width, v * rect.height];
        let world = [
            center[0] + local[0] * cos - local[1] * sin,
            center[1] + local[0] * sin + local[1] * cos,
        ];
        let uv = item.source_uv(world, [width, height])?;
        let x = ((uv[0] * w as f32) as usize).min(w.checked_sub(1)?);
        let y = ((uv[1] * h as f32) as usize).min(h.checked_sub(1)?);
        let at = (y * w + x) * 4;
        rgba.get(at..at + 4)?.try_into().ok()
    });
    let mut gamut = tally(pixels, Some(item.id));
    gamut.basis = Gamut::basis_of(item);
    gamut
}
fn tally(pixels: impl Iterator<Item = [u8; 4]>, image: Option<Uuid>) -> Gamut {
    let mut cells = vec![0u32; GAMUT_HUES * GAMUT_RINGS];
    // Main colors: 4 bits per channel, with the sum of the real colors per bucket.
    let mut buckets = vec![(0u32, [0u64; 3]); 4096];
    let mut total = 0u32;
    for pixel in pixels {
        if pixel[3] < 128 {
            continue;
        }
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let (hue, saturation) = hue_saturation(rgb);
        let sector = ((hue * GAMUT_HUES as f32) as usize).min(GAMUT_HUES - 1);
        let ring = ((saturation * GAMUT_RINGS as f32) as usize).min(GAMUT_RINGS - 1);
        cells[sector * GAMUT_RINGS + ring] += 1;
        let key = usize::from(rgb[0] >> 4) << 8
            | usize::from(rgb[1] >> 4) << 4
            | usize::from(rgb[2] >> 4);
        let bucket = &mut buckets[key];
        bucket.0 += 1;
        for (sum, value) in bucket.1.iter_mut().zip(rgb) {
            *sum += u64::from(value);
        }
        total += 1;
    }
    let fullest = cells.iter().copied().max().unwrap_or(0).max(1);
    // Rounded up, so a cell with any pixel never reads as empty.
    let grid = cells
        .iter()
        .map(|&count| ((u64::from(count) * 255).div_ceil(u64::from(fullest))) as u8)
        .collect();
    let mut ranked: Vec<_> = buckets
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, sum)| {
            let color = sum.map(|v| (v / u64::from(*count)) as u8);
            (*count, color)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut colors: Vec<([u8; 3], u32)> = Vec::new();
    for (count, color) in ranked {
        // Close shades add their pixels to the color they look like.
        if let Some(near) = colors
            .iter_mut()
            .find(|(other, _)| distance(*other, color) < 56.)
        {
            near.1 += count;
            continue;
        }
        if colors.len() < GAMUT_COLORS {
            colors.push((color, count));
        }
    }
    colors.sort_by_key(|color| std::cmp::Reverse(color.1));
    let colors = colors
        .into_iter()
        .map(|(color, count)| {
            let share = (u64::from(count) * 255 / u64::from(total.max(1))).clamp(1, 255) as u8;
            (color, share)
        })
        .collect();
    Gamut {
        image,
        view: 0,
        sort: 0,
        basis: 0,
        grid,
        colors,
    }
}

fn distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(&x, y)| (f32::from(x) - f32::from(y)).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_land_in_their_hue_and_saturation_cells_and_shares_add_up() {
        // Three quarters pure red, a quarter mid gray, one transparent blue pixel.
        let mut pixels = Vec::new();
        for _ in 0..30 {
            pixels.extend_from_slice(&[255, 0, 0, 255]);
        }
        for _ in 0..10 {
            pixels.extend_from_slice(&[128, 128, 128, 255]);
        }
        pixels.extend_from_slice(&[0, 0, 255, 0]);
        let gamut = measure(&pixels, None);
        assert!(gamut.valid());
        // Red: hue sector 0, outermost ring; gray: ring 0. Blue is transparent.
        assert_eq!(gamut.grid[GAMUT_RINGS - 1], 255);
        assert!(gamut.grid[0] > 0);
        let blue = (GAMUT_HUES * 2 / 3) * GAMUT_RINGS + GAMUT_RINGS - 1;
        assert_eq!(gamut.grid[blue], 0);
        assert_eq!(gamut.colors[0].0, [255, 0, 0]);
        assert_eq!(gamut.colors[1].0, [128, 128, 128]);
        let total: u32 = gamut.colors.iter().map(|c| u32::from(c.1)).sum();
        assert!((250..=255).contains(&total), "shares {total}");
        assert_eq!(hue_saturation([0, 0, 255]), (2. / 3., 1.));
    }
    #[test]
    fn a_cropped_frame_measures_only_what_it_shows_and_records_its_basis() {
        // Left half red, right half blue.
        let mut pixels = Vec::new();
        for _ in 0..4 {
            for x in 0..4 {
                pixels.extend_from_slice(&if x < 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                });
            }
        }
        let rect = moth_scene::Rect {
            x: 0.,
            y: 0.,
            width: 4.,
            height: 4.,
        };
        let mut item = moth_scene::Item::image(blake3::hash(b"original"), rect, None);
        let whole = measure_item(&pixels, 4, 4, &item);
        assert_eq!(whole.colors.len(), 2);
        assert!(!whole.stale(&item));
        // Show only the left half: zoom content 2x and shift it right by a quarter.
        item.transform.content_scale = 2.;
        item.transform.content_offset = [0.5, 0.];
        assert!(whole.stale(&item), "a new crop shows other pixels");
        let cropped = measure_item(&pixels, 4, 4, &item);
        assert_eq!(cropped.colors.len(), 1);
        assert_eq!(cropped.colors[0].0, [255, 0, 0]);
        // Moving the frame keeps the basis.
        let mut moved = item.clone();
        moved.rect.x += 100.;
        assert!(!cropped.stale(&moved));
    }
}
