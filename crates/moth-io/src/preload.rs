// SPDX-License-Identifier: GPL-3.0-or-later
//! Whole-board preloading for [`crate::CacheMode::Full`].
//!
//! When a board opens, every image is prepared at full resolution: its 256-pixel
//! preview (the whole image when it is smaller) and every detail tile of every level.
//! The results stream to the UI thread, which uploads them to video memory and keeps
//! them there, so zooming and panning never load anything afterwards. GPU-ready data
//! comes from the disk cache and tile packs when present; otherwise the original is
//! decoded once and the packs are written for the next opening.
use crate::{
    DecodeError, Decoded, ImageFormat, LodCache, apply_format, pack, read_original, tiles,
};
use crossbeam_channel::{Receiver, SendTimeoutError, Sender};
use moth_format::{Handle, Storage};
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc, Condvar, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

/// Prepared images waiting for upload; bounds pixels in flight between threads
/// (256 BC7 tiles are 16 MiB) while letting the UI upload in large batches.
const RESULTS: usize = 256;
/// Decoded full frames all workers of one pool may hold at once. A frame larger than
/// this still runs, alone.
const FRAME_MEMORY: u64 = 1 << 30;
/// Most images one preload pass queues.
const QUEUE: usize = 1 << 16;

#[derive(Clone, Copy)]
struct Job {
    handle: Handle,
    hash: blake3::Hash,
    format: ImageFormat,
    generation: u64,
}
#[derive(Default)]
struct State {
    queue: VecDeque<Job>,
    offered: HashSet<(blake3::Hash, ImageFormat)>,
    total: usize,
    done: usize,
    started: bool,
    closed: bool,
}
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
    frames: Mutex<u64>,
    freed: Condvar,
    generation: AtomicU64,
    stop: AtomicBool,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
    fn live(&self, job: &Job) -> bool {
        !self.stop.load(Ordering::Relaxed)
            && self.generation.load(Ordering::Relaxed) == job.generation
    }
}

/// Progress of the current preload pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreloadProgress {
    /// Images finished (including ones that failed or needed nothing).
    pub done: usize,
    /// Images queued in this pass.
    pub total: usize,
}
impl PreloadProgress {
    /// Whether images are still being prepared.
    pub fn active(self) -> bool {
        self.done < self.total
    }
}

/// Preloads one board's images on worker threads started on first use.
pub(crate) struct Preloader {
    shared: Arc<Shared>,
    storage: Arc<dyn Storage>,
    cache: Arc<LodCache>,
    wake: Arc<dyn Fn() + Send + Sync>,
    tx: Sender<Decoded>,
    pub(crate) results: Receiver<Decoded>,
}
impl Preloader {
    pub(crate) fn new(
        storage: Arc<dyn Storage>,
        cache: Arc<LodCache>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (tx, results) = crossbeam_channel::bounded(RESULTS);
        Self {
            shared: Arc::new(Shared {
                state: Mutex::default(),
                ready: Condvar::new(),
                frames: Mutex::new(0),
                freed: Condvar::new(),
                generation: AtomicU64::new(0),
                stop: AtomicBool::new(false),
            }),
            storage,
            cache,
            wake,
            tx,
            results,
        }
    }
    /// Queues images not yet offered in this pass.
    pub(crate) fn offer(
        &self,
        format: ImageFormat,
        images: impl IntoIterator<Item = (Handle, blake3::Hash)>,
    ) -> std::io::Result<()> {
        let generation = self.shared.generation.load(Ordering::Relaxed);
        let mut state = self.shared.lock();
        let before = state.total;
        for (handle, hash) in images {
            if state.queue.len() >= QUEUE {
                break;
            }
            if !state.offered.insert((hash, format)) {
                continue;
            }
            state.queue.push_back(Job {
                handle,
                hash,
                format,
                generation,
            });
            state.total += 1;
        }
        if state.total != before && !state.started {
            state.started = true;
            drop(state);
            self.spawn()?;
        } else {
            drop(state);
        }
        self.shared.ready.notify_all();
        Ok(())
    }
    fn spawn(&self) -> std::io::Result<()> {
        // One core stays free for the UI; the bake thread and viewport decoders are
        // idle while the board is hidden behind the progress bar.
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1))
            .clamp(1, 32);
        for n in 0..threads {
            let shared = self.shared.clone();
            let storage = self.storage.clone();
            let cache = self.cache.clone();
            let wake = self.wake.clone();
            let tx = self.tx.clone();
            std::thread::Builder::new()
                .name(format!("preload-{n}"))
                .spawn(move || {
                    while let Some(job) = next(&shared) {
                        let result = run(storage.as_ref(), &cache, &shared, &job, &tx, &*wake);
                        if let Err(error) = result {
                            tracing::debug!(%error, "preload skipped an image");
                        }
                        let mut state = shared.lock();
                        if job.generation == shared.generation.load(Ordering::Relaxed) {
                            state.done += 1;
                        }
                        drop(state);
                        wake();
                    }
                })?;
        }
        Ok(())
    }
    /// Stops the pass: queued images are dropped and running ones stop at their next
    /// tile. Offered images are not offered again until [`Preloader::restart`].
    pub(crate) fn cancel(&self) {
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        let mut state = self.shared.lock();
        state.queue.clear();
        state.done = state.total;
        drop(state);
        while self.results.try_recv().is_ok() {}
    }
    /// Starts over, for example after video memory was cleared: every image can be
    /// offered again.
    pub(crate) fn restart(&self) {
        self.cancel();
        let mut state = self.shared.lock();
        state.offered.clear();
        state.offered.shrink_to_fit();
        state.total = 0;
        state.done = 0;
    }
    pub(crate) fn progress(&self) -> PreloadProgress {
        let state = self.shared.lock();
        PreloadProgress {
            done: state.done,
            total: state.total,
        }
    }
}
impl Drop for Preloader {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        let mut state = self.shared.lock();
        state.closed = true;
        state.queue.clear();
        drop(state);
        self.shared.ready.notify_all();
    }
}
/// Frame memory held by one bake; released on drop.
struct Lease<'a>(&'a Shared, u64);
impl Drop for Lease<'_> {
    fn drop(&mut self) {
        let mut used = self.0.frames.lock().unwrap_or_else(PoisonError::into_inner);
        *used = used.saturating_sub(self.1);
        self.0.freed.notify_all();
    }
}
/// Waits until `bytes` of frame memory fit [`FRAME_MEMORY`] beside the other workers.
fn lease<'a>(shared: &'a Shared, job: &Job, bytes: u64) -> Result<Lease<'a>, DecodeError> {
    let mut used = shared.frames.lock().unwrap_or_else(PoisonError::into_inner);
    while *used > 0 && used.saturating_add(bytes) > FRAME_MEMORY {
        if !shared.live(job) {
            return Err(DecodeError::Cancelled);
        }
        used = shared
            .freed
            .wait_timeout(used, Duration::from_millis(100))
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
    *used += bytes;
    Ok(Lease(shared, bytes))
}
fn next(shared: &Shared) -> Option<Job> {
    let mut state = shared.lock();
    loop {
        if state.closed {
            return None;
        }
        if let Some(job) = state.queue.pop_front() {
            return Some(job);
        }
        state = shared
            .ready
            .wait(state)
            .unwrap_or_else(PoisonError::into_inner);
    }
}
/// Hands one prepared image to the UI, giving up when the pass is cancelled.
fn send(
    shared: &Shared,
    job: &Job,
    tx: &Sender<Decoded>,
    wake: &dyn Fn(),
    mut image: Decoded,
) -> Result<(), DecodeError> {
    loop {
        if !shared.live(job) {
            return Err(DecodeError::Cancelled);
        }
        match tx.send_timeout(image, Duration::from_millis(100)) {
            Ok(()) => {
                wake();
                return Ok(());
            }
            Err(SendTimeoutError::Timeout(back)) => {
                // The UI drains on its next frame; make sure there is one.
                wake();
                image = back;
            }
            Err(SendTimeoutError::Disconnected(_)) => return Err(DecodeError::Cancelled),
        }
    }
}
fn run(
    storage: &dyn Storage,
    cache: &LodCache,
    shared: &Shared,
    job: &Job,
    tx: &Sender<Decoded>,
    wake: &dyn Fn(),
) -> Result<(), DecodeError> {
    if !shared.live(job) {
        return Ok(());
    }
    let side = tiles::SIDE;
    let threshold = cache.mode().tile_threshold();
    // Reopening: the preview comes from the disk cache and every tile from the pack,
    // so the original is not decoded at all.
    if let Some(mut preview) = cache.lookup(job.hash, job.format, side) {
        preview.hash = job.hash;
        let levels = tiles::tiled_levels(preview.original, threshold);
        if levels.is_empty() {
            return send(shared, job, tx, wake, preview);
        }
        if let Some(reader) = cache.pack_reader(job.hash, job.format) {
            send(shared, job, tx, wake, preview)?;
            let mut cancelled = false;
            let read = pack::read_all(reader, job.hash, job.format, &mut |tile| {
                send(shared, job, tx, wake, tile).map_err(|_| {
                    cancelled = true;
                    std::io::Error::other("preload cancelled")
                })
            });
            match read {
                Ok(()) => return Ok(()),
                Err(_) if cancelled => return Err(DecodeError::Cancelled),
                // A damaged pack is rebuilt below; tiles already sent stay valid.
                Err(error) => tracing::debug!(%error, "tile pack rejected during preload"),
            }
        }
    }
    let bytes = read_original(storage, job.handle)?;
    let original = tiles::dimensions(&bytes)?;
    let _frame = lease(shared, job, tiles::bake_bytes(original))?;
    if blake3::hash(&bytes) != job.hash {
        // The handle no longer holds this content.
        return Ok(());
    }
    let levels = tiles::tiled_levels(original, threshold);
    let count = tiles::tile_count(original, levels.clone());
    let mut writer = if count == 0 {
        None
    } else {
        cache
            .pack_writer(job.hash, job.format, pack::size(job.format, count))
            .map(|out| pack::PackWriter::new(out, job.format))
    };
    // One decode yields every tile and the preview.
    let preview = tiles::bake(&bytes, levels, &shared.stop, &mut |tile| {
        let tile = apply_format(job.format, tile);
        let failed = match writer.as_mut() {
            Some(pack) => pack.push(&tile).err(),
            None => None,
        };
        if let Some(error) = failed {
            // The upload goes on; only the copy for the next opening is lost.
            tracing::debug!(%error, "tile pack write failed during preload");
            writer = None;
        }
        send(shared, job, tx, wake, tile)
    })?;
    drop(bytes);
    if let Some(pack) = writer {
        pack.finish(job.hash, original)?.commit()?;
    }
    // Kept in the disk cache so the next opening skips this decode.
    let preview = cache.keep(apply_format(job.format, preview), side);
    send(shared, job, tx, wake, preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

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
    fn a_preload_sends_the_preview_and_every_tile() {
        let source =
            image::RgbaImage::from_fn(700, 300, |x, y| image::Rgba([x as u8, y as u8, 40, 255]));
        let mut encoded = io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(source)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .unwrap();
        let bytes = encoded.into_inner();
        let hash = blake3::hash(&bytes);
        let cache = Arc::new(LodCache::new());
        cache.set_mode(crate::CacheMode::Full);
        let preloader = Preloader::new(Arc::new(Source(bytes)), cache, Arc::new(|| {}));
        preloader
            .offer(ImageFormat::Rgba8, [(Handle(1), hash), (Handle(1), hash)])
            .unwrap();
        // 700x300 at the full-resolution threshold: levels 0 (3x2 tiles) and 1 (2x1).
        let expected = 1 + 6 + 2;
        let mut received = Vec::new();
        while received.len() < expected {
            received.push(
                preloader
                    .results
                    .recv_timeout(Duration::from_secs(30))
                    .unwrap(),
            );
        }
        let previews: Vec<_> = received.iter().filter(|i| i.tile.is_none()).collect();
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].original, [700, 300]);
        assert!(previews[0].mips[0].width.max(previews[0].mips[0].height) <= tiles::SIDE);
        let tiles: HashSet<_> = received.iter().filter_map(|i| i.tile).collect();
        assert_eq!(tiles.len(), expected - 1);
        for _ in 0..300 {
            if !preloader.progress().active() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(preloader.progress(), PreloadProgress { done: 1, total: 1 });
        preloader.restart();
        assert_eq!(preloader.progress(), PreloadProgress::default());
    }
}
