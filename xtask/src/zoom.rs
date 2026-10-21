// SPDX-License-Identifier: GPL-3.0-or-later
//! Reproducible decode/cache zoom workload; excludes native UI and GPU upload.
use moth_format::{Handle, ReadSeek, Storage};
use moth_io::{
    CacheMode, DecodePool, ImageFormat, LodCache, Request,
    tiles::{CORE, SIDE, Tile},
};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct Source(Arc<AtomicUsize>, Arc<[u8]>);
impl Storage for Source {
    fn open(&self, _: Handle) -> io::Result<Box<dyn ReadSeek>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(io::Cursor::new(self.1.clone())))
    }
    fn append(&self, _: Handle) -> io::Result<Box<dyn io::Write + Send>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    fn sync(&self, _: Handle) -> io::Result<()> {
        Ok(())
    }
}
fn rss() -> u64 {
    moth_platform::memory::resident_bytes().unwrap_or(0)
}
fn complete(
    pool: &DecodePool,
    mut pending: Vec<Request>,
    original: [u32; 2],
) -> anyhow::Result<Duration> {
    let start = Instant::now();
    while !pending.is_empty() {
        pool.requests.schedule(pending.iter().copied());
        let completion = pool.results.recv_timeout(Duration::from_secs(10))?;
        let image = completion.result?;
        anyhow::ensure!(
            image.tile == completion.request.tile && image.original == original,
            "invalid zoom detail"
        );
        pool.requests.acknowledge(completion.request);
        pending.retain(|r| *r != completion.request);
    }
    pool.requests.schedule([]);
    Ok(start.elapsed())
}
pub fn run(code: &str, input: Option<&std::path::Path>) -> anyhow::Result<()> {
    let bytes: Arc<[u8]> = if let Some(input) = input {
        anyhow::ensure!(
            std::fs::metadata(input)?.len() <= moth_io::MAX_INPUT_BYTES,
            "input exceeds decode budget"
        );
        std::fs::read(input)?.into()
    } else {
        include_bytes!("../../corpus/images/4k.jpg")
            .as_slice()
            .into()
    };
    let header = moth_io::decode(&bytes, 1)?;
    let original = header.original;
    let hash = header.hash;
    let mode = CacheMode::from_code(code)
        .ok_or_else(|| anyhow::anyhow!("zoom mode: memory | memory_low | disk | none"))?;
    let directory =
        std::env::temp_dir().join(format!("mothref-zoom-{}-{}", std::process::id(), code));
    std::fs::create_dir(&directory)?;
    let disk = Arc::new(moth_platform::lod_store::DiskStore::in_directory(
        directory.clone(),
        64 * 1024 * 1024,
    ));
    let cache = Arc::new(LodCache::new());
    cache.set_store(disk.clone());
    cache.set_mode(mode);
    cache.set_memory_budget(32 * 1024 * 1024);
    let reads = Arc::new(AtomicUsize::new(0));
    let pool = DecodePool::with_cache(
        Arc::new(Source(reads.clone(), bytes)),
        Arc::new(|| {}),
        cache.clone(),
    )?;
    let base = Request {
        handle: Handle(0),
        blob: Some(hash),
        tile: None,
        side: 256,
        format: ImageFormat::Rgba8,
    };
    let initial = rss();
    let open = complete(&pool, vec![base], original)?;
    let density = 2048. / original[0].max(original[1]) as f32;
    let level = if density >= 1. {
        0
    } else {
        (-density.log2()).floor().max(1.) as u32
    };
    let size = original.map(|v| v.div_ceil(1 << level));
    let detail: Vec<_> = if mode == CacheMode::MemoryLowQuality
        || original[0].max(original[1]) <= moth_io::MAX_SIDE
    {
        vec![Request {
            side: 2048.min(cache.max_side()),
            ..base
        }]
    } else {
        (0..size[1].div_ceil(CORE))
            .flat_map(|y| {
                (0..size[0].div_ceil(CORE)).map(move |x| Request {
                    tile: Some(Tile { level, x, y }),
                    side: SIDE,
                    ..base
                })
            })
            .collect()
    };
    let before_cold = reads.load(Ordering::Relaxed);
    let cold = complete(&pool, detail.clone(), original)?;
    let cold_opens = reads.load(Ordering::Relaxed) - before_cold;
    let zoomed = rss();
    let before = reads.load(Ordering::Relaxed);
    let mut warm = Vec::new();
    for _ in 0..5 {
        warm.push(complete(&pool, detail.clone(), original)?);
    }
    let warm_opens = reads.load(Ordering::Relaxed) - before;
    anyhow::ensure!(
        mode == CacheMode::NoCache || warm_opens == 0,
        "warm zoom reopened the original {warm_opens} times"
    );
    warm.sort();
    complete(&pool, vec![base], original)?;
    let out = rss();
    let retained = cache.memory_bytes();
    anyhow::ensure!(retained <= 32 * 1024 * 1024, "zoom cache exceeded budget");
    cache.set_mode(CacheMode::NoCache);
    let released = rss();
    println!(
        "{mode:?} | open {:.3} ms | cold zoom {:.3} ms | warm median {:.3} ms | requests {} | cold opens {cold_opens} | warm opens {warm_opens} | cache {retained} bytes | disk {} bytes | RSS initial/in/out/cleared {initial}/{zoomed}/{out}/{released}",
        open.as_secs_f64() * 1000.,
        cold.as_secs_f64() * 1000.,
        warm[2].as_secs_f64() * 1000.,
        detail.len(),
        disk.usage()?
    );
    drop(pool);
    std::fs::remove_dir_all(directory)?;
    Ok(())
}
