// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    let _ = moth_format::board::read(&mut std::io::Cursor::new(bytes));
    let _ = moth_format::board::peek_preview(&mut std::io::Cursor::new(bytes));
    let _ = moth_format::board::decode_preview(bytes);
});
