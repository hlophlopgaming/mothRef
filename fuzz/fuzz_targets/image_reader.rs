// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    let _ = moth_io::decode(bytes, 64);
});
