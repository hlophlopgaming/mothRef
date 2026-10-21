// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    let _ = moth_io::decode(bytes, 64);
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let _ = moth_io::tiles::decode(bytes, moth_io::tiles::Tile { level: 0, x: 0, y: 0 }, &cancel);
    let tile = moth_io::tiles::Tile {
        level: bytes.len() as u32 % 4,
        x: u32::from(bytes.last().copied().unwrap_or(0)) % 4,
        y: u32::from(bytes.get(bytes.len().saturating_sub(2)).copied().unwrap_or(0)) % 4,
    };
    let _ = moth_io::tiles::decode(bytes, tile, &cancel);
});
