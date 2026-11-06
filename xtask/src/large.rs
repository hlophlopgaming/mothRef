// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic large originals for the 100-megapixel measurements.
//!
//! `xtask large-inputs [dir]` writes three files whose content depends on nothing but
//! this code, so runs on different machines decode the same pixels:
//! a 12,000 × 8,334 PNG and JPEG (just over 100 megapixels, the largest MothRef keeps
//! at full detail) and a 9,000 × 9,000 lossless WebP (81 megapixels; WebP has no
//! row decoder, so it is the whole-frame case). Existing files are kept.
use std::path::Path;

/// A checkerboard over a two-axis gradient with faint grain: edges for tiles to keep
/// sharp, and noise like a photo's so the codecs work about as hard as on one (a
/// clean gradient compresses to almost nothing). The grain is a hash of the position:
/// no randomness, the same pixels everywhere.
fn pixel(x: u32, y: u32, w: u32, h: u32) -> [u8; 3] {
    let mut seed = x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B);
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x2C1B_3C6D);
    seed ^= seed >> 12;
    let grain = |shift: u32| ((seed >> shift) & 7) as i32 - 4;
    let channel = |base: u32, shift: u32| (base as i32 + grain(shift)).clamp(0, 255) as u8;
    let check = ((x / 200 + y / 200) % 2) * 255;
    [
        channel(x * 255 / w, 0),
        channel(y * 255 / h, 3),
        channel(check, 6),
    ]
}

pub fn run(directory: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(directory)?;
    let inputs: [(&str, u32, u32, image::ImageFormat); 3] = [
        ("100mp.png", 12_000, 8_334, image::ImageFormat::Png),
        ("100mp.jpg", 12_000, 8_334, image::ImageFormat::Jpeg),
        ("81mp.webp", 9_000, 9_000, image::ImageFormat::WebP),
    ];
    for (name, w, h, format) in inputs {
        let path = directory.join(name);
        if path.exists() {
            println!("{} (kept)", path.display());
            continue;
        }
        let image = image::RgbImage::from_fn(w, h, |x, y| image::Rgb(pixel(x, y, w, h)));
        // WebP's encoder in `image` takes RGBA.
        let image = if format == image::ImageFormat::WebP {
            image::DynamicImage::ImageRgba8(image::DynamicImage::ImageRgb8(image).into_rgba8())
        } else {
            image::DynamicImage::ImageRgb8(image)
        };
        let partial = path.with_extension("partial");
        image.save_with_format(&partial, format)?;
        std::fs::rename(&partial, &path)?;
        println!(
            "{} {}×{} {:.1} MB",
            path.display(),
            w,
            h,
            std::fs::metadata(&path)?.len() as f64 / 1e6
        );
    }
    Ok(())
}
