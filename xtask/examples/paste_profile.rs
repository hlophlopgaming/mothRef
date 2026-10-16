// SPDX-License-Identifier: GPL-3.0-or-later
fn main() {
    let mut pixels = vec![0; 3840 * 2160 * 4];
    let mut seed = 42u32;
    for pixel in pixels.as_chunks_mut::<4>().0 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        pixel.copy_from_slice(&[seed as u8, (seed >> 8) as u8, (seed >> 16) as u8, 255]);
    }
    let start = std::time::Instant::now();
    let _ = moth_io::preview_rgba(3840, 2160, &pixels, 256).unwrap();
    println!("preview {:?}", start.elapsed());
    let start = std::time::Instant::now();
    moth_io::write_png(std::io::sink(), 3840, 2160, &pixels).unwrap();
    println!("png {:?}", start.elapsed());
    let scene = moth_render::headless::board().unwrap();
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        std::hint::black_box(scene.top());
    }
    println!("indexed top {:?}", start.elapsed());
    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        std::hint::black_box(scene.items().map(|i| &i.z).max());
    }
    println!("scanned top {:?}", start.elapsed());
}
