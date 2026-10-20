// SPDX-License-Identifier: GPL-3.0-or-later
//! Rebuildable LOD caches that let zoom reuse earlier decodes.
//!
//! Without a cache every zoom step that crosses a preview size class re-reads the
//! original, decodes it, rebuilds the mip pyramid and re-encodes it for the GPU. The
//! cache keeps the GPU-ready pyramid of one decode, either in RAM or on disk, and later
//! requests take the mips they need from it. Every entry is disposable: losing it only
//! costs a new decode from the original.
use crate::{Decoded, ImageFormat, MAX_SIDE, Mip};
use moth_format::ReadSeek;
use std::{
    collections::HashMap,
    io::{self, Read, Seek, SeekFrom},
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

/// Smallest side a caching mode decodes at; equal to the import preview side, so
/// opening a zoomed-out board costs no more than importing it.
const CACHED_MIN_SIDE: u32 = 256;
/// Caching modes decode this many times the requested side, so a 4x zoom-in is served
/// from the cache instead of a new decode.
const CACHED_HEADROOM: u32 = 4;
/// Detail ceiling of [`CacheMode::MemoryLowQuality`].
pub const LOW_QUALITY_SIDE: u32 = 1024;

/// How decoded previews are kept between zoom steps, mirroring PureRef's performance modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CacheMode {
    /// Decode from the original for every new detail level: least memory, most CPU.
    NoCache,
    /// Keep GPU-ready pyramids on disk: little RAM, disk reads instead of decodes.
    Disk,
    /// Keep GPU-ready pyramids in RAM within a budget: fastest zoom, more memory.
    #[default]
    Memory,
    /// Like [`CacheMode::Memory`], but detail stops at [`LOW_QUALITY_SIDE`] pixels.
    MemoryLowQuality,
}
impl CacheMode {
    /// Every mode, in the order the settings list shows them.
    pub const ALL: [Self; 4] = [
        Self::Memory,
        Self::MemoryLowQuality,
        Self::Disk,
        Self::NoCache,
    ];
    /// Stable preference value, independent of the enum layout.
    pub fn code(self) -> &'static str {
        match self {
            Self::NoCache => "none",
            Self::Disk => "disk",
            Self::Memory => "memory",
            Self::MemoryLowQuality => "memory_low",
        }
    }
    /// Parses a preference value written by [`CacheMode::code`].
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.code() == code.trim())
    }
    fn index(self) -> u8 {
        match self {
            Self::NoCache => 0,
            Self::Disk => 1,
            Self::Memory => 2,
            Self::MemoryLowQuality => 3,
        }
    }
    fn from_index(index: u8) -> Self {
        match index {
            1 => Self::Disk,
            2 => Self::Memory,
            3 => Self::MemoryLowQuality,
            _ => Self::NoCache,
        }
    }
    /// Whether decoded pyramids are kept in RAM.
    pub fn uses_memory(self) -> bool {
        matches!(self, Self::Memory | Self::MemoryLowQuality)
    }
    /// Largest preview side this mode ever requests or keeps.
    pub fn max_side(self) -> u32 {
        match self {
            Self::MemoryLowQuality => LOW_QUALITY_SIDE,
            _ => MAX_SIDE,
        }
    }
}

/// Disk backend for [`CacheMode::Disk`]; paths stay in the platform crate.
pub trait LodStore: Send + Sync {
    /// Opens a cached object, or `None` when it does not exist.
    fn read(&self, key: &str) -> io::Result<Option<Box<dyn ReadSeek>>>;
    /// Replaces a cached object with the concatenated parts. Readers never observe a
    /// partial object; a failed write leaves the previous object or none.
    fn write(&self, key: &str, parts: &[&[u8]]) -> io::Result<()>;
}

struct Entry {
    image: Arc<Decoded>,
    bytes: u64,
    used: u64,
}
#[derive(Default)]
struct Memory {
    entries: HashMap<(blake3::Hash, ImageFormat), Entry>,
    bytes: u64,
    clock: u64,
}
impl Memory {
    fn evict_to(&mut self, budget: u64) {
        while self.bytes > budget {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.used)
                .map(|(key, _)| *key)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(entry.bytes);
            }
        }
        if self.entries.is_empty() {
            self.entries.shrink_to_fit();
        }
    }
}

/// Shared LOD cache for every decode pool in the process.
///
/// The RAM part has an explicit byte budget with LRU eviction; the app sets it from
/// available memory. Locks are held only for map updates, never for I/O or decoding.
pub struct LodCache {
    mode: AtomicU8,
    budget: AtomicU64,
    memory: Mutex<Memory>,
    store: OnceLock<Arc<dyn LodStore>>,
}
impl Default for LodCache {
    fn default() -> Self {
        Self {
            mode: AtomicU8::new(CacheMode::NoCache.index()),
            budget: AtomicU64::new(0),
            memory: Mutex::default(),
            store: OnceLock::new(),
        }
    }
}
impl LodCache {
    /// Starts in [`CacheMode::NoCache`] with no RAM budget until the app configures it.
    pub fn new() -> Self {
        Self::default()
    }
    /// One cache for all board tabs, so switching tabs keeps decoded previews.
    pub fn shared() -> Arc<Self> {
        static SHARED: OnceLock<Arc<LodCache>> = OnceLock::new();
        SHARED.get_or_init(|| Arc::new(Self::new())).clone()
    }
    /// Current mode; workers read it for each request.
    pub fn mode(&self) -> CacheMode {
        CacheMode::from_index(self.mode.load(Ordering::Relaxed))
    }
    /// Switching away from a RAM mode releases its entries at once.
    pub fn set_mode(&self, mode: CacheMode) {
        self.mode.store(mode.index(), Ordering::Relaxed);
        if !mode.uses_memory() {
            self.set_memory_budget(0);
        }
    }
    /// Largest preview side worth requesting in the current mode.
    pub fn max_side(&self) -> u32 {
        self.mode().max_side()
    }
    /// Installs the disk backend once; later calls keep the first backend.
    pub fn set_store(&self, store: Arc<dyn LodStore>) {
        let _ = self.store.set(store);
    }
    /// Lowering the budget evicts least recently used pyramids immediately.
    pub fn set_memory_budget(&self, bytes: u64) {
        let bytes = if self.mode().uses_memory() { bytes } else { 0 };
        self.budget.store(bytes, Ordering::Relaxed);
        self.lock().evict_to(bytes);
    }
    /// Bytes of decoded pyramids currently held in RAM.
    pub fn memory_bytes(&self) -> u64 {
        self.lock().bytes
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, Memory> {
        // Holders only update the map and never panic; a poisoned map is still consistent.
        self.memory.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Side a worker decodes at for a request of `target` pixels.
    ///
    /// Caching modes decode with headroom so later zoom steps reuse the result. Imports
    /// (`known` is false) keep the requested side so bulk loading does not slow down.
    pub fn decode_side(&self, target: u32, known: bool) -> u32 {
        let mode = self.mode();
        let target = target.clamp(1, mode.max_side());
        if mode == CacheMode::NoCache || !known {
            return target;
        }
        target
            .saturating_mul(CACHED_HEADROOM)
            .max(CACHED_MIN_SIDE)
            .min(mode.max_side())
    }

    /// Returns the mips for `target` without touching the original, if a cached
    /// pyramid has enough detail.
    pub fn lookup(&self, hash: blake3::Hash, format: ImageFormat, target: u32) -> Option<Decoded> {
        let mode = self.mode();
        let quality = mode != CacheMode::MemoryLowQuality;
        match mode {
            CacheMode::NoCache => None,
            CacheMode::Memory | CacheMode::MemoryLowQuality => {
                let image = {
                    let mut memory = self.lock();
                    memory.clock += 1;
                    let clock = memory.clock;
                    let entry = memory.entries.get_mut(&(hash, format))?;
                    entry.used = clock;
                    entry.image.clone()
                };
                let first = pick(&longs(&image.mips), original_long(&image), target, quality)?;
                Some(slice(&image, first))
            }
            CacheMode::Disk => {
                let store = self.store.get()?;
                let reader = match store.read(&key(hash, format)) {
                    Ok(reader) => reader?,
                    Err(error) => {
                        tracing::debug!(%error, "LOD cache read failed");
                        return None;
                    }
                };
                match read(reader, hash, format, target, quality) {
                    Ok(image) => image,
                    Err(error) => {
                        tracing::debug!(%error, "LOD cache entry rejected");
                        None
                    }
                }
            }
        }
    }

    /// Keeps a freshly decoded pyramid according to the mode and returns the mips that
    /// answer a request for `target` pixels.
    pub fn keep(&self, image: Decoded, target: u32) -> Decoded {
        let mode = self.mode();
        let quality = mode != CacheMode::MemoryLowQuality;
        match mode {
            CacheMode::NoCache => image,
            CacheMode::Disk => {
                if let Some(store) = self.store.get()
                    && let Err(error) = write(store.as_ref(), &image)
                {
                    tracing::debug!(%error, "LOD cache write failed");
                }
                trim(image, target, quality)
            }
            CacheMode::Memory | CacheMode::MemoryLowQuality => {
                let image = Arc::new(image);
                self.insert(image.clone());
                let first =
                    pick(&longs(&image.mips), original_long(&image), target, quality).unwrap_or(0);
                match Arc::try_unwrap(image) {
                    Ok(image) => trim(image, target, quality),
                    Err(image) => slice(&image, first),
                }
            }
        }
    }

    fn insert(&self, image: Arc<Decoded>) {
        let bytes = image.mips.iter().map(|m| m.bytes.len() as u64).sum::<u64>();
        let budget = self.budget.load(Ordering::Relaxed);
        if bytes > budget {
            return;
        }
        let key = (image.hash, image.format);
        let mut memory = self.lock();
        // A smaller pyramid never replaces a more detailed one.
        if let Some(existing) = memory.entries.get(&key)
            && top(&existing.image) >= top(&image)
        {
            return;
        }
        memory.clock += 1;
        let used = memory.clock;
        let old = memory.entries.insert(key, Entry { image, bytes, used });
        if let Some(old) = old {
            memory.bytes = memory.bytes.saturating_sub(old.bytes);
        }
        memory.bytes += bytes;
        memory.evict_to(budget);
    }
}

fn long(width: u32, height: u32) -> u32 {
    width.max(height)
}
fn longs(mips: &[Mip]) -> Vec<u32> {
    mips.iter().map(|m| long(m.width, m.height)).collect()
}
fn top(image: &Decoded) -> u32 {
    image.mips.first().map_or(0, |m| long(m.width, m.height))
}
fn original_long(image: &Decoded) -> u32 {
    image.original[0].max(image.original[1])
}
/// The renderer's size class for a preview whose long side is `side`.
fn class(side: u32) -> u32 {
    side.next_power_of_two().max(16)
}

/// Chooses the first mip that satisfies a request for `target` pixels.
///
/// The chosen mip's size class must cover the request exactly as the renderer's
/// residency check does, otherwise the request would repeat forever. With `quality`,
/// the mip must also have at least the requested detail rather than only its class.
/// `None` means the pyramid lacks the detail and a new decode is needed.
fn pick(longs: &[u32], original: u32, target: u32, quality: bool) -> Option<usize> {
    let need = target.min(original).max(1);
    let top = *longs.first()?;
    if class(top) < need {
        return None;
    }
    let mut first = 0;
    for (index, &side) in longs.iter().enumerate() {
        let enough = if quality {
            side >= need
        } else {
            class(side) >= need
        };
        if !enough {
            break;
        }
        first = index;
    }
    Some(first)
}
fn slice(image: &Decoded, first: usize) -> Decoded {
    Decoded {
        hash: image.hash,
        original: image.original,
        mips: image.mips[first.min(image.mips.len().saturating_sub(1))..]
            .iter()
            .map(|mip| Mip {
                width: mip.width,
                height: mip.height,
                bytes: mip.bytes.clone(),
            })
            .collect(),
        format: image.format,
    }
}
/// Drops leading mips without copying the remaining pixel data.
fn trim(mut image: Decoded, target: u32, quality: bool) -> Decoded {
    let first = pick(&longs(&image.mips), original_long(&image), target, quality).unwrap_or(0);
    let first = first.min(image.mips.len().saturating_sub(1));
    image.mips = image.mips.split_off(first);
    image
}

const MAGIC: &[u8; 8] = b"MOTHLOD1";
const HEADER: usize = 8 + 1 + 1 + 4 + 4 + 32;
const ENTRY: usize = 4 + 4 + 8 + 32;
const MAX_MIPS: usize = 16;

fn format_code(format: ImageFormat) -> u8 {
    match format {
        ImageFormat::Rgba8 => 0,
        ImageFormat::Bc7 => 1,
    }
}
fn key(hash: blake3::Hash, format: ImageFormat) -> String {
    let suffix = match format {
        ImageFormat::Rgba8 => "rgba8",
        ImageFormat::Bc7 => "bc7",
    };
    format!("{}-{suffix}", hash.to_hex())
}
/// Exact payload size of one mip, so a damaged table cannot request a large allocation.
fn mip_bytes(format: ImageFormat, width: u32, height: u32) -> u64 {
    match format {
        ImageFormat::Rgba8 => u64::from(width) * u64::from(height) * 4,
        ImageFormat::Bc7 => u64::from(width.div_ceil(4)) * u64::from(height.div_ceil(4)) * 16,
    }
}

/// Layout: magic, format, mip count, original size, content hash, then one table entry
/// (size, payload length, payload digest) per mip, then the payloads largest first.
fn write(store: &dyn LodStore, image: &Decoded) -> io::Result<()> {
    if image.mips.is_empty() || image.mips.len() > MAX_MIPS {
        return Ok(());
    }
    let mut header = Vec::with_capacity(HEADER + ENTRY * image.mips.len());
    header.extend_from_slice(MAGIC);
    header.push(format_code(image.format));
    header.push(image.mips.len() as u8);
    header.extend_from_slice(&image.original[0].to_le_bytes());
    header.extend_from_slice(&image.original[1].to_le_bytes());
    header.extend_from_slice(image.hash.as_bytes());
    for mip in &image.mips {
        header.extend_from_slice(&mip.width.to_le_bytes());
        header.extend_from_slice(&mip.height.to_le_bytes());
        header.extend_from_slice(&(mip.bytes.len() as u64).to_le_bytes());
        header.extend_from_slice(blake3::hash(&mip.bytes).as_bytes());
    }
    let mut parts: Vec<&[u8]> = Vec::with_capacity(image.mips.len() + 1);
    parts.push(&header);
    parts.extend(image.mips.iter().map(|mip| mip.bytes.as_slice()));
    store.write(&key(image.hash, image.format), &parts)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    let mut word = [0; 4];
    word.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(word)
}
fn u64_at(bytes: &[u8], at: usize) -> u64 {
    let mut word = [0; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    u64::from_le_bytes(word)
}
fn invalid(reason: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason.to_owned())
}

/// Reads only the mips a request needs. Cache files are treated as untrusted: every
/// size is validated before allocation and every payload is checked against its digest.
fn read(
    mut reader: Box<dyn ReadSeek>,
    hash: blake3::Hash,
    format: ImageFormat,
    target: u32,
    quality: bool,
) -> io::Result<Option<Decoded>> {
    let mut header = [0u8; HEADER];
    reader.read_exact(&mut header)?;
    if &header[..8] != MAGIC || header[8] != format_code(format) {
        return Err(invalid("LOD cache header mismatch"));
    }
    let count = usize::from(header[9]);
    let original = [u32_at(&header, 10), u32_at(&header, 14)];
    if count == 0
        || count > MAX_MIPS
        || header[18..50] != hash.as_bytes()[..]
        || original[0] == 0
        || original[1] == 0
        || original[0] > 32768
        || original[1] > 32768
    {
        return Err(invalid("LOD cache identity mismatch"));
    }
    let mut table = vec![0u8; ENTRY * count];
    reader.read_exact(&mut table)?;
    let mut mips = Vec::with_capacity(count);
    let mut previous = None;
    for index in 0..count {
        let at = index * ENTRY;
        let width = u32_at(&table, at);
        let height = u32_at(&table, at + 4);
        let len = u64_at(&table, at + 8);
        let mut digest = [0u8; 32];
        digest.copy_from_slice(&table[at + 16..at + 48]);
        if width == 0
            || height == 0
            || width > MAX_SIDE
            || height > MAX_SIDE
            || len != mip_bytes(format, width, height)
            || previous.is_some_and(|(w, h)| width != (w / 2).max(1) || height != (h / 2).max(1))
        {
            return Err(invalid("LOD cache mip table is inconsistent"));
        }
        previous = Some((width, height));
        mips.push((width, height, len, blake3::Hash::from_bytes(digest)));
    }
    let sides: Vec<u32> = mips.iter().map(|&(w, h, _, _)| long(w, h)).collect();
    let Some(first) = pick(&sides, long(original[0], original[1]), target, quality) else {
        return Ok(None);
    };
    let skipped: u64 = mips[..first].iter().map(|&(_, _, len, _)| len).sum();
    reader.seek(SeekFrom::Start((HEADER + table.len()) as u64 + skipped))?;
    let mut decoded = Vec::with_capacity(count - first);
    for &(width, height, len, digest) in &mips[first..] {
        let mut bytes = vec![0u8; len as usize];
        reader.read_exact(&mut bytes)?;
        if blake3::hash(&bytes) != digest {
            return Err(invalid("LOD cache payload checksum mismatch"));
        }
        decoded.push(Mip {
            width,
            height,
            bytes,
        });
    }
    Ok(Some(Decoded {
        hash,
        original,
        mips: decoded,
        format,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn pyramid(side: u32, original: [u32; 2]) -> Decoded {
        let mut mips = Vec::new();
        let (mut w, mut h) = (side, side / 2);
        loop {
            mips.push(Mip {
                width: w,
                height: h,
                bytes: vec![(w % 251) as u8; (w * h * 4) as usize],
            });
            if w == 1 && h == 1 {
                break;
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Decoded {
            hash: blake3::hash(&side.to_le_bytes()),
            original,
            mips,
            format: ImageFormat::Rgba8,
        }
    }

    #[derive(Default)]
    struct Store(Mutex<HashMap<String, Vec<u8>>>);
    impl LodStore for Store {
        fn read(&self, key: &str) -> io::Result<Option<Box<dyn ReadSeek>>> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .map(|bytes| Box::new(Cursor::new(bytes)) as Box<dyn ReadSeek>))
        }
        fn write(&self, key: &str, parts: &[&[u8]]) -> io::Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(key.to_owned(), parts.concat());
            Ok(())
        }
    }

    #[test]
    fn modes_round_trip_through_preference_codes() {
        for mode in CacheMode::ALL {
            assert_eq!(CacheMode::from_code(mode.code()), Some(mode));
            assert_eq!(CacheMode::from_index(mode.index()), mode);
        }
        assert_eq!(CacheMode::from_code("bogus"), None);
        assert_eq!(CacheMode::default(), CacheMode::Memory);
    }

    #[test]
    fn caching_modes_decode_with_headroom_except_for_imports() {
        let cache = LodCache::new();
        assert_eq!(cache.decode_side(64, true), 64);
        cache.set_mode(CacheMode::Memory);
        assert_eq!(cache.decode_side(64, true), 256);
        assert_eq!(cache.decode_side(128, true), 512);
        assert_eq!(cache.decode_side(256, true), 1024);
        assert_eq!(cache.decode_side(1024, true), MAX_SIDE);
        assert_eq!(cache.decode_side(256, false), 256);
        cache.set_mode(CacheMode::MemoryLowQuality);
        assert_eq!(cache.decode_side(512, true), LOW_QUALITY_SIDE);
        assert_eq!(cache.decode_side(2048, false), LOW_QUALITY_SIDE);
    }

    #[test]
    fn pick_matches_the_renderer_residency_rule() {
        // A 1500-pixel original decoded natively: 1500, 750, 375, ...
        let sides = [1500, 750, 375, 187];
        assert_eq!(pick(&sides, 1500, 2048, true), Some(0));
        assert_eq!(pick(&sides, 1500, 1024, true), Some(0));
        assert_eq!(pick(&sides, 1500, 1024, false), Some(1));
        assert_eq!(pick(&sides, 1500, 256, true), Some(2));
        // Too little detail for the request: a new decode is needed.
        assert_eq!(pick(&[512, 256], 4000, 1024, true), None);
        assert_eq!(pick(&[512, 256], 4000, 1024, false), None);
        // Every choice keeps the renderer's class at or above the request.
        for target in [16, 32, 64, 128, 256, 512, 1024, 2048] {
            for quality in [true, false] {
                if let Some(first) = pick(&sides, 1500, target, quality) {
                    assert!(class(sides[first]) >= target.min(1500));
                }
            }
        }
    }

    #[test]
    fn memory_mode_serves_zoom_steps_without_new_decodes() {
        let cache = LodCache::new();
        cache.set_mode(CacheMode::Memory);
        cache.set_memory_budget(64 * 1024 * 1024);
        let image = pyramid(2048, [4096, 2048]);
        let hash = image.hash;
        let served = cache.keep(image, 256);
        assert_eq!(served.mips[0].width, 256);
        for target in [16, 128, 512, 1024, 2048] {
            let hit = cache.lookup(hash, ImageFormat::Rgba8, target).unwrap();
            assert_eq!(hit.mips[0].width, target.max(16));
            assert_eq!(hit.hash, hash);
        }
        assert!(cache.lookup(hash, ImageFormat::Bc7, 256).is_none());
        cache.set_mode(CacheMode::NoCache);
        assert_eq!(cache.memory_bytes(), 0);
        assert!(cache.lookup(hash, ImageFormat::Rgba8, 256).is_none());
    }

    #[test]
    fn memory_budget_evicts_least_recently_used() {
        let cache = LodCache::new();
        cache.set_mode(CacheMode::Memory);
        let one: u64 = pyramid(256, [256, 128])
            .mips
            .iter()
            .map(|m| m.bytes.len() as u64)
            .sum();
        cache.set_memory_budget(one * 2);
        let (a, b, c) = (
            pyramid(256, [256, 128]),
            pyramid(255, [255, 127]),
            pyramid(254, [254, 127]),
        );
        let (ha, hb, hc) = (a.hash, b.hash, c.hash);
        cache.keep(a, 16);
        cache.keep(b, 16);
        assert!(cache.lookup(ha, ImageFormat::Rgba8, 16).is_some());
        cache.keep(c, 16);
        assert!(cache.lookup(ha, ImageFormat::Rgba8, 16).is_some());
        assert!(cache.lookup(hb, ImageFormat::Rgba8, 16).is_none());
        assert!(cache.lookup(hc, ImageFormat::Rgba8, 16).is_some());
        assert!(cache.memory_bytes() <= one * 2);
        cache.set_memory_budget(0);
        assert_eq!(cache.memory_bytes(), 0);
    }

    #[test]
    fn disk_mode_reads_only_needed_mips_and_rejects_damage() {
        let cache = LodCache::new();
        let store = Arc::new(Store::default());
        cache.set_store(store.clone());
        cache.set_mode(CacheMode::Disk);
        let image = pyramid(1024, [3000, 1500]);
        let hash = image.hash;
        let served = cache.keep(image, 128);
        assert_eq!(served.mips[0].width, 128);
        let hit = cache.lookup(hash, ImageFormat::Rgba8, 512).unwrap();
        assert_eq!(hit.mips[0].width, 512);
        assert_eq!(hit.original, [3000, 1500]);
        assert!(cache.lookup(hash, ImageFormat::Rgba8, 2048).is_none());
        let name = key(hash, ImageFormat::Rgba8);
        let mut map = store.0.lock().unwrap();
        let bytes = map.get_mut(&name).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        drop(map);
        // The 1x1 mip is damaged, so any read that includes it fails safely.
        assert!(cache.lookup(hash, ImageFormat::Rgba8, 16).is_none());
        let mut map = store.0.lock().unwrap();
        map.get_mut(&name).unwrap().truncate(HEADER + 3);
        drop(map);
        assert!(cache.lookup(hash, ImageFormat::Rgba8, 16).is_none());
    }
}
