// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! MothRef decoder allocation regression check.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;
#[test]
fn four_k_preview_does_not_allocate_a_full_resolution_image() {
    let source = image::RgbaImage::from_pixel(3840, 2160, image::Rgba([120, 90, 60, 255]));
    let mut encoded = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(source)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    pool.install(|| {});
    let _profiler = dhat::Profiler::builder().testing().build();
    let image = moth_io::decode(encoded.get_ref(), 256).unwrap();
    assert_eq!(image.mips[0].width, 256);
    let jpeg = moth_io::decode(include_bytes!("../../../corpus/images/4k.jpg"), 256).unwrap();
    assert_eq!(jpeg.original, [3840, 2160]);
    assert_eq!(jpeg.mips[0].width, 256);
    let large =
        moth_io::decode(include_bytes!("../../../corpus/images/large-dct.jpg"), 256).unwrap();
    assert_eq!(large.original, [8192, 4097]);
    // Exercise the app's Rayon context even when no other decode worker is free.
    pool.install(|| {
        let preview =
            moth_io::decode(include_bytes!("../../../corpus/images/large-dct.jpg"), 256).unwrap();
        assert_eq!(preview.original, [8192, 4097]);
        assert_eq!(preview.mips[0].width, 256);
    });
    for bytes in [
        include_bytes!("../../../corpus/images/4k.gif").as_slice(),
        include_bytes!("../../../corpus/images/4k.bmp").as_slice(),
    ] {
        let preview = moth_io::decode(bytes, 256).unwrap();
        assert_eq!(preview.original, [3840, 2160]);
        assert_eq!(preview.mips[0].width, 256);
    }
    let stats = dhat::HeapStats::get();
    dhat::assert!(
        stats.max_bytes < 8 * 1024 * 1024,
        "preview must stay below 8 MiB transient allocations"
    );
    let pixels = vec![127; 3840 * 2160 * 4];
    let before = dhat::HeapStats::get().curr_bytes;
    moth_io::write_png(std::io::sink(), 3840, 2160, &pixels).unwrap();
    let stats = dhat::HeapStats::get();
    dhat::assert!(
        stats.max_bytes < before + 8 * 1024 * 1024,
        "streaming clipboard compression must stay below 8 MiB overhead"
    );
    let webp = moth_io::decode(include_bytes!("../../../corpus/images/budget.webp"), 256).unwrap();
    assert_eq!(webp.original, [2048, 1024]);
    let stats = dhat::HeapStats::get();
    dhat::assert!(
        stats.max_bytes < before + 64 * 1024 * 1024,
        "admitted WebP must stay below 64 MiB additional peak allocations"
    );
}
