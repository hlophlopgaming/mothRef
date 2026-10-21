// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Inspecting a preview at 1:1 needs pixels, not per-pixel averaging sums.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn unscaled_png_preview_does_not_allocate_area_sums() {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        2048,
        1024,
        image::Rgba([30, 60, 90, 255]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    let _profiler = dhat::Profiler::builder().testing().build();
    let preview = moth_io::decode(bytes.get_ref(), 2048).unwrap();
    assert_eq!(preview.mips[0].bytes.len(), 2048 * 1024 * 4);
    let stats = dhat::HeapStats::get();
    // Mip generation must fit beside the pixels without a full-frame f32 scratch buffer.
    dhat::assert!(stats.max_bytes < 2 * preview.mips[0].bytes.len());
}
