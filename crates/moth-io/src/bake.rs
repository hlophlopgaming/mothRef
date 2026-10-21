// SPDX-License-Identifier: GPL-3.0-or-later
//! Background tile baking.
//!
//! One low-priority pass decodes a large original once and writes every detail tile of
//! every level into a tile pack. Later zoom steps read GPU-ready tiles from the pack
//! instead of decoding the original for each batch, which for sequential JPEG means
//! re-reading every row above the tile.
//!
//! Images on screen jump the queue; [`BakeMode::Board`] also queues every image of the
//! board behind them.
use crate::{DecodeError, ImageFormat, LodCache, apply_format, pack, read_original, tiles};
use moth_format::{Handle, Storage};
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
};

/// Most queued bakes, and most remembered offers; both are trimmed, never unbounded.
/// Dropped offers are made again by later requests or the next board pass.
const QUEUE: usize = 4096;

/// Which images get a tile pack, as a user preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BakeMode {
    /// Never bake; existing packs are still read.
    Off,
    /// Bake large images the first time they are on screen.
    #[default]
    Visible,
    /// Also bake every large image of the open board in the background.
    Board,
}
impl BakeMode {
    /// Every mode, in the order the settings list shows them.
    pub const ALL: [Self; 3] = [Self::Visible, Self::Board, Self::Off];
    /// Stable preference value, independent of the enum layout.
    pub fn code(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Visible => "visible",
            Self::Board => "board",
        }
    }
    /// Parses a preference value written by [`BakeMode::code`].
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.code() == code.trim())
    }
    pub(crate) fn index(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Visible => 1,
            Self::Board => 2,
        }
    }
    pub(crate) fn from_index(index: u8) -> Self {
        match index {
            0 => Self::Off,
            2 => Self::Board,
            _ => Self::Visible,
        }
    }
}
/// Shared preference cell, read by every pool's offers.
pub(crate) struct BakeSetting(AtomicU8);
impl Default for BakeSetting {
    fn default() -> Self {
        Self(AtomicU8::new(BakeMode::default().index()))
    }
}
impl BakeSetting {
    pub(crate) fn get(&self) -> BakeMode {
        BakeMode::from_index(self.0.load(Ordering::Relaxed))
    }
    pub(crate) fn set(&self, mode: BakeMode) {
        self.0.store(mode.index(), Ordering::Relaxed);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Job {
    handle: Handle,
    hash: blake3::Hash,
    format: ImageFormat,
}
#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    offered: HashSet<(blake3::Hash, ImageFormat)>,
    running: bool,
    closed: bool,
}
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// One bake thread per decode pool; each image and format is queued once.
pub(crate) struct Baker {
    shared: Arc<Shared>,
    stop: Arc<AtomicBool>,
}
impl Baker {
    pub(crate) fn new(storage: Arc<dyn Storage>, cache: Arc<LodCache>) -> std::io::Result<Self> {
        let shared = Arc::new(Shared::default());
        let stop = Arc::new(AtomicBool::new(false));
        let (queue, flag) = (shared.clone(), stop.clone());
        std::thread::Builder::new()
            .name("lod-bake".into())
            .spawn(move || {
                while let Some(job) = next(&queue) {
                    if let Err(error) = run(storage.as_ref(), &cache, &job, &flag) {
                        tracing::debug!(%error, "tile bake skipped");
                    }
                    queue.lock().running = false;
                }
            })?;
        Ok(Self { shared, stop })
    }
    /// Queues an image on screen ahead of board work, if the renderer draws it from tiles.
    pub(crate) fn offer(
        &self,
        cache: &LodCache,
        handle: Handle,
        hash: blake3::Hash,
        format: ImageFormat,
        original: [u32; 2],
    ) {
        if tiles::tiled_levels(original).is_empty()
            || !cache.bakes()
            || cache.bake_mode() == BakeMode::Off
        {
            return;
        }
        let job = Job {
            handle,
            hash,
            format,
        };
        let mut state = self.shared.lock();
        if state.offered.contains(&(hash, format)) {
            // Already queued behind board work: move it to the front.
            if let Some(index) = state.queue.iter().position(|queued| *queued == job) {
                state.queue.remove(index);
                state.queue.push_front(job);
            }
            return;
        }
        if state.queue.len() == QUEUE {
            // Board work is the least urgent; make room for what is on screen.
            state.queue.pop_back();
        }
        remember(&mut state, job);
        state.queue.push_front(job);
        self.shared.ready.notify_one();
    }
    /// Queues every image of a board behind on-screen work. Sizes are unknown here,
    /// so the bake thread skips images that fit a whole preview after reading their header.
    pub(crate) fn offer_board(
        &self,
        cache: &LodCache,
        format: ImageFormat,
        images: impl IntoIterator<Item = (Handle, blake3::Hash)>,
    ) {
        if !cache.bakes() || cache.bake_mode() != BakeMode::Board {
            return;
        }
        let mut state = self.shared.lock();
        for (handle, hash) in images {
            if state.queue.len() == QUEUE {
                break;
            }
            if state.offered.contains(&(hash, format)) {
                continue;
            }
            let job = Job {
                handle,
                hash,
                format,
            };
            remember(&mut state, job);
            state.queue.push_back(job);
        }
        self.shared.ready.notify_one();
    }
    /// Images waiting for or undergoing a bake, for the settings panel.
    pub(crate) fn pending(&self) -> usize {
        let state = self.shared.lock();
        state.queue.len() + usize::from(state.running)
    }
}
/// Offers are remembered so each image is queued once; the memory is trimmed by
/// forgetting everything, which only costs a pack check when an offer repeats.
fn remember(state: &mut State, job: Job) {
    if state.offered.len() >= QUEUE {
        state.offered.clear();
        state.offered.shrink_to(QUEUE);
        let queued: Vec<_> = state.queue.iter().map(|j| (j.hash, j.format)).collect();
        state.offered.extend(queued);
    }
    state.offered.insert((job.hash, job.format));
}
fn next(shared: &Shared) -> Option<Job> {
    let mut state = shared.lock();
    loop {
        if state.closed {
            return None;
        }
        if let Some(job) = state.queue.pop_front() {
            state.running = true;
            return Some(job);
        }
        state = shared
            .ready
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}
impl Drop for Baker {
    fn drop(&mut self) {
        // The running bake checks the flag per row; the thread exits at its next wait.
        self.stop.store(true, Ordering::Relaxed);
        let mut state = self.shared.lock();
        state.closed = true;
        state.queue.clear();
        self.shared.ready.notify_all();
    }
}

fn run(
    storage: &dyn Storage,
    cache: &LodCache,
    job: &Job,
    stop: &AtomicBool,
) -> Result<(), DecodeError> {
    if stop.load(Ordering::Relaxed)
        || !cache.bakes()
        || cache.bake_mode() == BakeMode::Off
        || cache.has_pack(job.hash, job.format)
    {
        return Ok(());
    }
    let bytes = read_original(storage, job.handle)?;
    // Board passes queue every image; most fit a whole preview and need no pack.
    let original = tiles::dimensions(&bytes)?;
    let levels = tiles::tiled_levels(original);
    let count = tiles::tile_count(original, levels.clone());
    if count == 0 || count > tiles::MAX_BAKED_TILES {
        return Ok(());
    }
    if blake3::hash(&bytes) != job.hash {
        // The handle no longer holds this content; nothing to bake.
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
    fn bake_modes_round_trip_and_off_stops_new_bakes() {
        for mode in BakeMode::ALL {
            assert_eq!(BakeMode::from_code(mode.code()), Some(mode));
            assert_eq!(BakeMode::from_index(mode.index()), mode);
        }
        assert_eq!(BakeMode::from_code("bogus"), None);
        assert_eq!(BakeMode::default(), BakeMode::Visible);
        let cache = LodCache::new();
        cache.set_store(Arc::new(Shared(Arc::new(Store::default()))));
        cache.set_mode(crate::CacheMode::Disk);
        cache.set_bake_mode(BakeMode::Off);
        let job = Job {
            handle: Handle(1),
            hash: blake3::hash(b"never read"),
            format: ImageFormat::Rgba8,
        };
        // Off returns before opening the original, which this storage cannot provide.
        run(&Source(Vec::new()), &cache, &job, &AtomicBool::new(false)).unwrap();
        assert!(!cache.has_pack(job.hash, job.format));
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
