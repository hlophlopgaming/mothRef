// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Repeated AVIF previews must release their Rust-owned RGBA frame buffers.
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[test]
fn small_avif_frames_are_released_after_each_decode() {
    let bytes = include_bytes!("../../../corpus/images/alpha.avif");
    let _profiler = dhat::Profiler::builder().testing().build();
    drop(moth_io::decode(bytes, 64).unwrap());
    let after_warmup = dhat::HeapStats::get().curr_bytes;
    for _ in 0..4 {
        drop(moth_io::decode(bytes, 64).unwrap());
    }
    let stats = dhat::HeapStats::get();
    dhat::assert!(stats.curr_bytes <= after_warmup + 1024 * 1024);
    dhat::assert!(stats.max_bytes < 64 * 1024 * 1024);
}
