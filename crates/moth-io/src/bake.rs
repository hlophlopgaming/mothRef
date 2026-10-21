// SPDX-License-Identifier: GPL-3.0-or-later
//! Background tile baking.
//!
//! The first time a large image is shown, one low-priority pass decodes its original
//! once and writes every detail tile of every level into a tile pack. Later zoom steps
//! read GPU-ready tiles from the pack instead of decoding the original for each batch,
//! which for sequential JPEG means re-reading every row above the tile.
use crate::{DecodeError, ImageFormat, LodCache, apply_format, pack, read_original, tiles};
use moth_format::{Handle, Storage};
use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

/// Queued bakes; a full queue drops offers, which later requests make again.
const QUEUE: usize = 64;

struct Job {
    handle: Handle,
    hash: blake3::Hash,
    format: ImageFormat,
}

/// One bake thread per decode pool; work is queued once per image and format.
pub(crate) struct Baker {
    jobs: crossbeam_channel::Sender<Job>,
    offered: Mutex<HashSet<(blake3::Hash, ImageFormat)>>,
    stop: Arc<AtomicBool>,
}
impl Baker {
    pub(crate) fn new(storage: Arc<dyn Storage>, cache: Arc<LodCache>) -> std::io::Result<Self> {
        let (jobs, rx) = crossbeam_channel::bounded::<Job>(QUEUE);
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        std::thread::Builder::new()
            .name("lod-bake".into())
            .spawn(move || {
                while let Ok(job) = rx.recv() {
                    if flag.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Err(error) = run(storage.as_ref(), &cache, &job, &flag) {
                        tracing::debug!(%error, "tile bake skipped");
                    }
                }
            })?;
        Ok(Self {
            jobs,
            offered: Mutex::default(),
            stop,
        })
    }
    /// Queues a bake for an image the renderer draws from tiles.
    pub(crate) fn offer(
        &self,
        cache: &LodCache,
        handle: Handle,
        hash: blake3::Hash,
        format: ImageFormat,
        original: [u32; 2],
    ) {
        if tiles::tiled_levels(original).is_empty() || !cache.bakes() {
            return;
        }
        let mut offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        if offered.contains(&(hash, format)) {
            return;
        }
        if self
            .jobs
            .try_send(Job {
                handle,
                hash,
                format,
            })
            .is_ok()
        {
            offered.insert((hash, format));
        }
    }
}
impl Drop for Baker {
    fn drop(&mut self) {
        // The running bake checks this per row; the thread exits once the queue closes.
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn run(
    storage: &dyn Storage,
    cache: &LodCache,
    job: &Job,
    stop: &AtomicBool,
) -> Result<(), DecodeError> {
    if !cache.bakes() || cache.has_pack(job.hash, job.format) {
        return Ok(());
    }
    let bytes = read_original(storage, job.handle)?;
    if blake3::hash(&bytes) != job.hash {
        // The handle no longer holds this content; nothing to bake.
        return Ok(());
    }
    let original = tiles::dimensions(&bytes)?;
    let levels = tiles::tiled_levels(original);
    let count = tiles::tile_count(original, levels.clone());
    if count == 0 || count > tiles::MAX_BAKED_TILES {
        return Ok(());
    }
    let Some(out) = cache.pack_writer(job.hash, job.format, pack::size(job.format, count)) else {
        return Ok(());
    };
    let mut writer = pack::PackWriter::new(out, job.format);
    tiles::bake(&bytes, levels, stop, &mut |tile| {
        writer
            .push(&apply_format(job.format, tile))
            .map_err(DecodeError::from)
    })?;
    drop(bytes);
    // An unfinished writer is dropped without committing, which discards it.
    writer.finish(job.hash, original)?.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, io};

    #[derive(Default)]
    struct Store(Mutex<HashMap<String, Vec<u8>>>);
    struct Pending(Vec<u8>, String, Arc<Store>);
    impl io::Write for Pending {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl crate::LodWriter for Pending {
        fn commit(self: Box<Self>) -> io::Result<()> {
            let Pending(bytes, key, store) = *self;
            store.0.lock().unwrap().insert(key, bytes);
            Ok(())
        }
    }
    struct Shared(Arc<Store>);
    impl crate::LodStore for Shared {
        fn read(&self, key: &str) -> io::Result<Option<Box<dyn moth_format::ReadSeek>>> {
            Ok(self
                .0
                .0
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .map(|bytes| Box::new(io::Cursor::new(bytes)) as Box<dyn moth_format::ReadSeek>))
        }
        fn write(&self, key: &str, parts: &[&[u8]]) -> io::Result<()> {
            self.0.0.lock().unwrap().insert(key.into(), parts.concat());
            Ok(())
        }
        fn writer(&self, key: &str, size: u64) -> io::Result<Option<Box<dyn crate::LodWriter>>> {
            Ok(Some(Box::new(Pending(
                Vec::with_capacity(size as usize),
                key.into(),
                self.0.clone(),
            ))))
        }
    }
    struct Source(Vec<u8>);
    impl Storage for Source {
        fn open(&self, _: Handle) -> io::Result<Box<dyn moth_format::ReadSeek>> {
            Ok(Box::new(io::Cursor::new(self.0.clone())))
        }
        fn append(&self, _: Handle) -> io::Result<Box<dyn io::Write + Send>> {
            Err(io::ErrorKind::Unsupported.into())
        }
        fn sync(&self, _: Handle) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_bake_serves_every_tile_the_renderer_can_request() {
        let source = image::RgbaImage::from_fn(2600, 300, |x, y| {
            image::Rgba([x as u8, y as u8, (x / 7) as u8, 255])
        });
        let mut encoded = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let bytes = encoded.into_inner();
        let hash = blake3::hash(&bytes);
        let store = Arc::new(Store::default());
        let cache = LodCache::new();
        cache.set_store(Arc::new(Shared(store.clone())));
        cache.set_mode(crate::CacheMode::Disk);
        let job = Job {
            handle: Handle(1),
            hash,
            format: ImageFormat::Rgba8,
        };
        run(
            &Source(bytes.clone()),
            &cache,
            &job,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(cache.has_pack(hash, ImageFormat::Rgba8));
        let original = [2600, 300];
        let levels = tiles::tiled_levels(original);
        assert_eq!(levels, 0..4);
        let cancel = AtomicBool::new(false);
        for level in levels {
            let size = original.map(|v| v.div_ceil(1 << level));
            for y in 0..size[1].div_ceil(tiles::CORE) {
                for x in 0..size[0].div_ceil(tiles::CORE) {
                    let tile = tiles::Tile { level, x, y };
                    let baked = cache.pack_tile(hash, ImageFormat::Rgba8, tile).unwrap();
                    let direct = tiles::decode(&bytes, tile, &cancel).unwrap();
                    assert_eq!(baked.tile, Some(tile));
                    assert_eq!(baked.original, direct.original);
                    assert_eq!(baked.mips[0].bytes, direct.mips[0].bytes, "{tile:?}");
                }
            }
        }
        // A second offer finds the pack and does nothing.
        run(&Source(bytes), &cache, &job, &AtomicBool::new(false)).unwrap();
        cache.set_mode(crate::CacheMode::NoCache);
        assert!(
            cache
                .pack_tile(
                    hash,
                    ImageFormat::Rgba8,
                    tiles::Tile {
                        level: 0,
                        x: 0,
                        y: 0
                    }
                )
                .is_none()
        );
    }

    #[test]
    fn cancelled_or_interlaced_bakes_leave_no_pack() {
        let cancel = AtomicBool::new(true);
        let bytes = include_bytes!("../../../corpus/images/4k.jpg");
        let mut tiles = 0;
        let result = tiles::bake(bytes, 0..1, &cancel, &mut |_| {
            tiles += 1;
            Ok(())
        });
        assert!(matches!(result, Err(DecodeError::Cancelled)));
        assert_eq!(tiles, 0);
        let interlaced = include_bytes!("../../../corpus/images/adam7.png");
        let result = tiles::bake(interlaced, 0..1, &AtomicBool::new(false), &mut |_| Ok(()));
        assert!(result.is_err());
    }
}
