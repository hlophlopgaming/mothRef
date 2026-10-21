// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Full-detail mip construction must not need a full-frame floating-point scratch buffer.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn jpeg_detail_keeps_mip_scratch_bounded() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    pool.install(|| {});
    let _profiler = dhat::Profiler::builder().testing().build();
    let preview = pool.install(|| {
        moth_io::decode(include_bytes!("../../../corpus/images/4k.jpg"), 2048).unwrap()
    });
    assert_eq!(
        [preview.mips[0].width, preview.mips[0].height],
        [1920, 1080]
    );
    let stats = dhat::HeapStats::get();
    dhat::assert!(stats.max_bytes < 3 * preview.mips[0].bytes.len());
}
