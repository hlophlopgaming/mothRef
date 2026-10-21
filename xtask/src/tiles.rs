// SPDX-License-Identifier: GPL-3.0-or-later
//! Separate cold codec work from cache-hit latency on one deterministic JPEG tile.
use moth_format::{Handle, ReadSeek, Storage};
use moth_io::{
    CacheMode, DecodePool, ImageFormat, LodCache, Request,
    tiles::{SIDE, Tile},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Source(Arc<AtomicUsize>);
impl Storage for Source {
    fn open(&self, _: Handle) -> io::Result<Box<dyn ReadSeek>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(io::Cursor::new(
            include_bytes!("../../corpus/images/4k.jpg").as_slice(),
        )))
    }
    fn append(&self, _: Handle) -> io::Result<Box<dyn io::Write + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn sync(&self, _: Handle) -> io::Result<()> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
pub fn run() -> anyhow::Result<()> {
    let reads = Arc::new(AtomicUsize::new(0));
    let cache = Arc::new(LodCache::new());
    let pool = DecodePool::with_cache(
        Arc::new(Source(reads.clone())),
        Arc::new(|| {}),
        cache.clone(),
    )?;
    let hash = moth_io::decode(include_bytes!("../../corpus/images/4k.jpg"), 16)?.hash;
    let request = Request {
        handle: Handle(0),
        blob: Some(hash),
        tile: Some(Tile {
            level: 0,
            x: 9,
            y: 2,
        }),
        side: SIDE,
        format: ImageFormat::Rgba8,
    };
    for mode in [CacheMode::NoCache, CacheMode::Memory] {
        cache.set_mode(mode);
        cache.set_memory_budget(4 * 1024 * 1024);
        let mut times = Vec::new();
        let before = reads.load(Ordering::Relaxed);
        for _ in 0..6 {
            pool.requests.schedule([]);
            let start = Instant::now();
            pool.requests.schedule([request]);
            let completion = pool.results.recv_timeout(Duration::from_secs(10))?;
            let image = completion.result?;
            times.push(start.elapsed());
            anyhow::ensure!(
                image.tile == request.tile
                    && image.original == [3840, 2160]
                    && image.mips[0].bytes.len() == (SIDE * SIDE * 4) as usize,
                "tile benchmark returned invalid detail"
            );
            pool.requests.acknowledge(request);
        }
        let opens = reads.load(Ordering::Relaxed) - before;
        anyhow::ensure!(
            opens == if mode == CacheMode::NoCache { 6 } else { 1 },
            "warm tiles must not open or decode the original again"
        );
        // The first memory-mode request warms the cache and is not a cache-hit sample.
        times.remove(0);
        times.sort();
        println!(
            "4K JPEG tile | {mode:?} | median {:.3} ms | source opens {opens} | RAM cache {} KiB",
            times[times.len() / 2].as_secs_f64() * 1000.,
            cache.memory_bytes() / 1024
        );
    }
    Ok(())
}

/// Exercises cancellation, cache-mode changes and tile eviction during the RSS soak.
pub struct Smoke {
    pool: DecodePool,
    cache: Arc<LodCache>,
    request: Request,
    received: usize,
}
impl Smoke {
    pub fn new() -> anyhow::Result<Self> {
        let cache = Arc::new(LodCache::new());
        let pool = DecodePool::with_cache(
            Arc::new(Source(Arc::default())),
            Arc::new(|| {}),
            cache.clone(),
        )?;
        let hash = moth_io::decode(include_bytes!("../../corpus/images/4k.jpg"), 16)?.hash;
        Ok(Self {
            pool,
            cache,
            request: Request {
                handle: Handle(0),
                blob: Some(hash),
                tile: Some(Tile {
                    level: 0,
                    x: 0,
                    y: 0,
                }),
                side: SIDE,
                format: ImageFormat::Rgba8,
            },
            received: 0,
        })
    }
    pub fn step(&mut self, frame: usize) -> anyhow::Result<()> {
        while let Ok(completion) = self.pool.results.try_recv() {
            self.pool.requests.acknowledge(completion.request);
            let image = completion.result?;
            anyhow::ensure!(
                image.tile == completion.request.tile
                    && image.mips[0].bytes.len() == (SIDE * SIDE * 4) as usize,
                "tile soak returned invalid detail"
            );
            self.received += 1;
        }
        if frame.is_multiple_of(200) {
            self.cache.set_mode(if (frame / 200).is_multiple_of(2) {
                CacheMode::Memory
            } else {
                CacheMode::NoCache
            });
            self.cache.set_memory_budget(8 * 1024 * 1024);
        }
        self.pool.requests.schedule([Request {
            tile: Some(Tile {
                level: 0,
                x: (frame / 8 % 12) as u32,
                y: (frame / 96 % 8) as u32,
            }),
            ..self.request
        }]);
        anyhow::ensure!(
            self.cache.memory_bytes() <= 8 * 1024 * 1024,
            "tile cache exceeded its soak budget"
        );
        Ok(())
    }
    pub fn finish(self) -> anyhow::Result<()> {
        anyhow::ensure!(self.received > 0, "tile soak did not complete any decode");
        Ok(())
    }
}
