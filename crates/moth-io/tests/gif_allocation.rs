// SPDX-License-Identifier: GPL-3.0-or-later
//! Playback keeps a compositor and a preview while decoding successive frames.

use moth_format::{Handle, ReadSeek, Storage};
use moth_io::{ImageFormat, content_hash, gif_animation};
use std::{
    borrow::Cow,
    io::{self, Cursor, Write},
};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

struct Original(Vec<u8>);

impl Storage for Original {
    fn open(&self, _: Handle) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(Cursor::new(self.0.clone())))
    }

    fn append(&self, _: Handle) -> io::Result<Box<dyn Write + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn sync(&self, _: Handle) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[test]
fn gif_preview_memory_stays_bounded_across_frames() {
    let (width, height) = (320_u16, 192_u16);
    let frames = 96;
    let mut original = Vec::new();
    let mut pixels = vec![0; usize::from(width) * usize::from(height)];
    {
        let mut encoder =
            gif::Encoder::new(&mut original, width, height, &[255, 0, 0, 0, 0, 255]).unwrap();
        for index in 0..frames {
            pixels.fill((index % 2) as u8);
            encoder
                .write_frame(&gif::Frame {
                    width,
                    height,
                    delay: 4,
                    buffer: Cow::Borrowed(&pixels),
                    ..gif::Frame::default()
                })
                .unwrap();
        }
    }
    let original = Original(original);
    let key = content_hash(b"transient texture");
    let _profiler = dhat::Profiler::builder().testing().build();
    let mut reader = gif_animation::Reader::default();
    let mut preview = None;
    for index in 0..frames {
        let frame = reader.frame(&original, Handle(0), index).unwrap();
        preview = Some(gif_animation::preview(frame, key, ImageFormat::Rgba8));
    }
    std::hint::black_box(preview);
    let stats = dhat::HeapStats::get();
    dhat::assert!(stats.max_bytes < 4 * 1024 * 1024);
}
