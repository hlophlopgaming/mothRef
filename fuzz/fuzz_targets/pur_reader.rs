// SPDX-License-Identifier: GPL-3.0-or-later
#![no_main]
#![forbid(unsafe_code)]
libfuzzer_sys::fuzz_target!(|bytes: &[u8]| {
    let _ = moth_import::pur::read(std::io::Cursor::new(bytes), &mut |_| Ok(()));
    // Repair the outer checksum so mutations also exercise database/blob/legacy parsing.
    if bytes.len() >= 108 && bytes.len() <= 1024 * 1024 {
        let mut repaired = bytes.to_vec();
        let (field, offset) = if bytes.get(5) == Some(&b'2') {
            (40, 104)
        } else {
            (44, 108)
        };
        let digest = purcore::checksum::md5_hex_from(bytes, offset);
        for (i, c) in digest.bytes().enumerate() {
            repaired[field + i * 2] = 0;
            repaired[field + i * 2 + 1] = c;
        }
        let _ = moth_import::pur::read(std::io::Cursor::new(repaired), &mut |_| Ok(()));
    }
});
