// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Original-resolution tiles must not allocate area-filter accumulators.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn original_resolution_tile_uses_less_than_one_mib_of_rust_heap() {
    let _profiler = dhat::Profiler::builder().testing().build();
    let image = moth_io::tiles::decode(
        include_bytes!("../../../corpus/images/4k.jpg"),
        moth_io::tiles::Tile {
            level: 0,
            x: 9,
            y: 2,
        },
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(image.mips[0].bytes.len(), 256 * 256 * 4);
    let stats = dhat::HeapStats::get();
    dhat::assert!(stats.max_bytes < 1024 * 1024);
}
