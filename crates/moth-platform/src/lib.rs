// SPDX-License-Identifier: GPL-3.0-or-later
//! Native paths and clipboard access stay on the desktop side of the boundary.
/// Background persistence and recovery for native boards.
pub mod boards;
mod clipboard_web;
/// Native shutdown and bounded local crash reports.
pub mod crash;
pub mod export;
/// Video-memory detection for the texture limit.
pub mod gpu;
pub mod input;
/// Bounded read-only desktop board catalog.
pub mod library;
/// Best-effort detection of the user's interface language.
pub mod locale;
/// Disk backend for the rebuildable LOD cache.
pub mod lod_store;
/// Background sampling of available physical RAM for bounded image residency.
pub mod memory;
/// Plain-text app preferences.
pub mod settings;
/// Event-driven system shortcuts and backend capability checks.
pub mod shortcuts;
mod source;
mod viewer;
use moth_format::{Handle, ReadSeek, Storage};
#[cfg(unix)]
use std::fs::File;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
};

/// Native failures cross worker boundaries as values instead of panics.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// Keeps filesystem failures available for diagnostics.
    #[error("storage: {0}")]
    Io(#[from] io::Error),
    /// Clipboard ownership and availability vary with the desktop session.
    #[error("clipboard: {0}")]
    Clipboard(#[from] arboard::Error),
    /// Invalid pixels never reach GPU upload.
    #[error("image: {0}")]
    Image(#[from] moth_io::DecodeError),
    /// Limits reject a paste before it can evict another original.
    #[error("{0}")]
    Budget(&'static str),
    /// Downloads fail without exposing URL credentials or query strings.
    #[error("Clipboard image download failed: {0}")]
    Download(&'static str),
    /// OS dimensions must fit the portable image representation.
    #[error("invalid clipboard dimensions")]
    Dimensions,
    /// A native backend panic becomes an ordinary failed paste.
    #[error("clipboard backend panicked")]
    Panic,
    /// The UI supplies a localized explanation for unsupported clipboard content.
    #[error("clipboard has no supported image, files, URL or HTML image")]
    EmptyClipboard,
}
/// The desktop catalog stores source identities, never image pixels.
pub struct DesktopStorage {
    paths: Mutex<HashMap<Handle, source::Source>>,
    board: uuid::Uuid,
    next_handle: std::sync::atomic::AtomicU64,
    directory: Option<PathBuf>,
}
impl Default for DesktopStorage {
    fn default() -> Self {
        Self {
            paths: Mutex::default(),
            board: uuid::Uuid::now_v7(),
            next_handle: std::sync::atomic::AtomicU64::new(0),
            directory: None,
        }
    }
}
fn data_directory() -> io::Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|value| PathBuf::from(value).is_absolute())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    base.filter(|path| path.is_absolute())
        .map(|path| path.join("mothref/recovery"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "application data directory unavailable",
            )
        })
}
impl DesktopStorage {
    /// Isolates tools and tests from the user's recovery and recent-file catalog.
    pub fn in_directory(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
            ..Self::default()
        }
    }
    fn base_directory(&self) -> io::Result<PathBuf> {
        self.directory
            .clone()
            .map(Ok)
            .unwrap_or_else(data_directory)
    }
    /// Registering an OS-provided path performs no filesystem access.
    pub fn register(&self, path: PathBuf) -> io::Result<Handle> {
        self.register_source(source::Source {
            path,
            range: None,
            member: None,
        })
    }
    /// Registers a read-only ZIP entry without doing I/O on the caller.
    pub fn register_zip(&self, path: PathBuf, member: String) -> io::Result<Handle> {
        self.register_source(source::Source {
            path,
            range: None,
            member: Some(member),
        })
    }
    fn register_range(&self, path: PathBuf, range: moth_format::board::Blob) -> io::Result<Handle> {
        self.register_source(source::Source {
            path,
            range: Some(range),
            member: None,
        })
    }
    fn register_source(&self, source: source::Source) -> io::Result<Handle> {
        let mut paths = self
            .paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?;
        if paths.len() >= 1_000_000 {
            return Err(io::Error::other("source catalog budget full"));
        }
        let handle = Handle(
            self.next_handle
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        paths.insert(handle, source);
        Ok(handle)
    }
    /// Returns only registered metadata; no filesystem access occurs on the caller.
    pub fn source_name(&self, handle: Handle) -> String {
        self.paths
            .lock()
            .ok()
            .and_then(|paths| {
                paths
                    .get(&handle)
                    .filter(|source| source.range.is_none())
                    .and_then(|source| {
                        source
                            .member
                            .as_ref()
                            .map(std::path::Path::new)
                            .unwrap_or(&source.path)
                            .file_name()
                    })
                    .map(|name| name.to_string_lossy().chars().take(256).collect())
            })
            .unwrap_or_default()
    }
    /// Persists clipboard originals on a worker before acknowledging a paste.
    pub fn stage_rgba(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<Handle, PlatformError> {
        self.stage_png(width, height, rgba)
            .map(|(handle, _)| handle)
    }
    fn stage_png(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<(Handle, blake3::Hash), PlatformError> {
        if width == 0
            || height == 0
            || u64::from(width) * u64::from(height) > 32 * 1024 * 1024
            || rgba.len() as u64 + 48 > moth_io::MAX_INPUT_BYTES
            || u64::from(width) * u64::from(height) * 4 != rgba.len() as u64
        {
            return Err(PlatformError::Budget(
                "Invalid or oversized clipboard image",
            ));
        }
        let (directory, used) = self.staging_directory(rgba.len() as u64 + 48)?;
        let path = directory.join(format!("{}.png", uuid::Uuid::now_v7()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        let result = (|| -> Result<blake3::Hash, PlatformError> {
            let mut writer = io::BufWriter::new(file);
            let hash = moth_io::write_png(&mut writer, width, height, rgba)?;
            io::Write::flush(&mut writer)?;
            let encoded_bytes = writer.get_ref().metadata()?.len();
            if encoded_bytes > moth_io::MAX_INPUT_BYTES
                || used.saturating_add(encoded_bytes) > 8 * 1024 * 1024 * 1024
            {
                return Err(PlatformError::Budget(
                    "Compressed clipboard image exceeds input budget",
                ));
            }
            writer.get_ref().sync_all()?;
            #[cfg(unix)]
            {
                File::open(&directory)?.sync_all()?;
                File::open(self.base_directory()?)?.sync_all()?;
            }
            Ok(hash)
        })();
        match result {
            Ok(hash) => Ok((self.register(path)?, hash)),
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                Err(error)
            }
        }
    }
    fn staging_directory(&self, bytes: u64) -> Result<(PathBuf, u64), PlatformError> {
        let directory = self.recovery_directory()?;
        std::fs::create_dir_all(&directory)?;
        if std::fs::symlink_metadata(&directory)?
            .file_type()
            .is_symlink()
        {
            return Err(PlatformError::Budget(
                "Recovery directory must not be a symlink",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }

        let mut used = 0u64;
        for (count, entry) in std::fs::read_dir(&directory)?.enumerate() {
            if count >= 10000 {
                return Err(PlatformError::Budget(
                    "Clipboard recovery entry budget full",
                ));
            }
            used = used.saturating_add(entry?.metadata()?.len());
        }
        // Originals are not an evictable cache; each board has its own disk budget.
        if used.saturating_add(bytes) > 8 * 1024 * 1024 * 1024 {
            return Err(PlatformError::Budget("Board recovery budget full (8 GiB)"));
        }
        Ok((directory, used))
    }
    fn stage_encoded(&self, bytes: &[u8]) -> Result<(Handle, moth_io::Decoded), PlatformError> {
        let image = moth_io::decode(bytes, 256)?;
        let (directory, _) = self.staging_directory(bytes.len() as u64)?;
        let path = directory.join(format!("{}.image", uuid::Uuid::now_v7()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        let result = (|| -> Result<Handle, PlatformError> {
            io::Write::write_all(&mut file, bytes)?;
            file.sync_all()?;
            #[cfg(unix)]
            {
                File::open(&directory)?.sync_all()?;
                File::open(self.base_directory()?)?.sync_all()?;
            }
            Ok(self.register(path.clone())?)
        })();
        match result {
            Ok(handle) => Ok((handle, image)),
            Err(error) => {
                let _ = std::fs::remove_file(path);
                Err(error)
            }
        }
    }
    /// Reuses clipboard pixels for the first preview while staging the original durably.
    pub fn stage_preview(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(Handle, moth_io::Decoded), PlatformError> {
        let (handle, hash) = self.stage_png(width, height, pixels)?;
        let preview = moth_io::preview_rgba_with_hash(width, height, pixels, 256, hash)?;
        Ok((handle, preview))
    }
    fn recovery_directory(&self) -> io::Result<PathBuf> {
        Ok(self.base_directory()?.join(self.board.to_string()))
    }
    /// Removes only this board's staged originals after an explicit discard.
    /// The clipboard worker serializes this with staging; crashes retain originals.
    pub fn discard(&self) -> io::Result<()> {
        match std::fs::remove_dir_all(self.recovery_directory()?) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        }
    }
    fn forget(&self, handle: Handle) -> io::Result<()> {
        self.paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?
            .remove(&handle);
        Ok(())
    }
    fn path(&self, handle: Handle) -> io::Result<PathBuf> {
        let source = self.source(handle)?;
        if source.range.is_some() || source.member.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "original ranges are read-only",
            ));
        }
        Ok(source.path)
    }
    fn source(&self, handle: Handle) -> io::Result<source::Source> {
        self.paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?
            .get(&handle)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown storage handle"))
    }
}
fn parent_directory(path: &std::path::Path) -> &std::path::Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."))
}
impl Storage for DesktopStorage {
    fn temporary(&self, destination: Handle) -> io::Result<Handle> {
        let path = self.path(destination)?;
        let parent = parent_directory(&path);
        let temporary = parent.join(format!(".mothref-{}.tmp", uuid::Uuid::now_v7()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&temporary)?;
        self.register(temporary)
    }
    fn replace(&self, temporary: Handle, destination: Handle) -> io::Result<()> {
        let source = self.path(temporary)?;
        let target = self.path(destination)?;
        atomic_replace(&source, &target)?;
        #[cfg(unix)]
        File::open(parent_directory(&target))?.sync_all()?;
        self.paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?
            .remove(&temporary);
        Ok(())
    }
    fn open(&self, h: Handle) -> io::Result<Box<dyn ReadSeek>> {
        self.source(h)?.open()
    }
    fn append(&self, h: Handle) -> io::Result<Box<dyn io::Write + Send>> {
        Ok(Box::new(
            OpenOptions::new().append(true).open(self.path(h)?)?,
        ))
    }
    fn sync(&self, h: Handle) -> io::Result<()> {
        OpenOptions::new()
            .write(true)
            .open(self.path(h)?)?
            .sync_all()
    }
}
/// Capabilities are explicit so unsupported native features can be reflected in UI.
#[derive(Clone, Copy)]
pub struct Capabilities {
    /// Lets UI disable native operations a backend cannot supply.
    pub clipboard_images: bool,
}
/// Native operations have one boundary instead of platform cfgs in the app.
pub trait Platform {
    /// Opens a prepared image using the OS association; called only on a worker.
    fn open_image(&self, path: &std::path::Path) -> io::Result<()>;
    /// Queries physical RAM off the main thread; unknown systems use a fixed fallback.
    fn available_memory(&self) -> Option<u64>;
    /// Lets callers adapt controls to available native operations.
    fn capabilities(&self) -> Capabilities;
    /// Compositors can reject topmost windows while still supporting transparent surfaces.
    fn window_capabilities(&self, window: &winit::window::Window) -> WindowCapabilities;
    /// Global shortcuts require desktop registration; Wayland retains the Over controller.
    fn global_shortcuts_available(&self, window: &winit::window::Window) -> bool {
        shortcuts::available(window)
    }
    /// Uses native stacking only on compositors which expose it.
    fn window_bottom(&self, window: &winit::window::Window, enabled: bool) {
        if self.window_capabilities(window).on_top {
            window.set_window_level(if enabled {
                winit::window::WindowLevel::AlwaysOnBottom
            } else {
                winit::window::WindowLevel::Normal
            });
        }
    }
    /// Validates the remembered target before restoring a pin.
    fn pin_target_available(&self, pin: &pin::Pin) -> bool {
        pin.available()
    }
    /// Applies target stacking without transferring window ownership.
    fn pin_target(
        &self,
        pin: &mut pin::Pin,
        window: &winit::window::Window,
        enabled: bool,
    ) -> Result<(), String> {
        pin.enable(window, enabled)
    }
    /// Polls native selection only while the explicit picker is active.
    fn pick_pin_target(&self, pin: &mut pin::Pin, window: &winit::window::Window) -> Option<bool> {
        pin.pick(window)
    }
    /// Applies reversible window effects through one platform boundary.
    /// Apply canvas transparency and optional background blur without changing stacking.
    fn window_background(&self, window: &winit::window::Window, transparent: bool, blur: bool);
    fn window_effects(
        &self,
        window: &winit::window::Window,
        overlay: bool,
        on_top: bool,
        click_through: bool,
    ) -> Result<(), String>;
}
/// Native window affordances are independent of GPU alpha support.
#[derive(Clone, Copy)]
pub struct WindowCapabilities {
    /// The native backend implements background blur.
    pub blur: bool,
    /// Wayland compositors control stacking, so the app cannot promise topmost behavior.
    pub on_top: bool,
    /// Selecting a foreign window is supported by this backend.
    pub above_window: bool,
    /// The backend supports an empty input region.
    pub click_through: bool,
}
/// M0 native clipboard backend; overlay capabilities are added with M3.
pub struct Desktop;
impl Platform for Desktop {
    fn open_image(&self, path: &std::path::Path) -> io::Result<()> {
        viewer::open(path)
    }
    fn available_memory(&self) -> Option<u64> {
        let mut system = sysinfo::System::new();
        system.refresh_memory_specifics(sysinfo::MemoryRefreshKind::nothing().with_ram());
        (system.total_memory() > 0).then(|| system.available_memory())
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            clipboard_images: true,
        }
    }
    fn window_capabilities(&self, window: &winit::window::Window) -> WindowCapabilities {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let wayland = window
            .window_handle()
            .is_ok_and(|h| matches!(h.as_raw(), RawWindowHandle::Wayland(_)));
        WindowCapabilities {
            blur: cfg!(target_os = "macos"),
            on_top: !wayland,
            above_window: cfg!(target_os = "windows") || (cfg!(target_os = "linux") && !wayland),
            click_through: true,
        }
    }
    fn window_background(&self, window: &winit::window::Window, transparent: bool, blur: bool) {
        window.set_transparent(transparent);
        window.set_blur(blur && self.window_capabilities(window).blur);
    }
    fn window_effects(
        &self,
        window: &winit::window::Window,
        overlay: bool,
        on_top: bool,
        click_through: bool,
    ) -> Result<(), String> {
        let caps = self.window_capabilities(window);
        window.set_window_level(if on_top && caps.on_top {
            winit::window::WindowLevel::AlwaysOnTop
        } else {
            winit::window::WindowLevel::Normal
        });
        // Keep the normal window frameless too; the application owns its titlebar.
        window.set_decorations(false);
        window.set_transparent(overlay);
        window
            .set_cursor_hittest(!click_through)
            .map_err(|error| error.to_string())
    }
}
/// Worker messages keep staging and disposal in order.
pub enum ClipboardRequest {
    /// Capture the clipboard on the worker.
    Paste,
    /// Discard this board before the app exits.
    Discard,
}
/// Separates staging failures from discard failures during asynchronous shutdown.
pub enum ClipboardCompletion {
    /// A failed paste leaves the board and its recovery files intact.
    Pasted(Result<(Handle, moth_io::Decoded), PlatformError>),
    /// Native file lists use the same bounded import path as dropped files.
    Files(Vec<PathBuf>),
    /// Acknowledges cleanup before the application exits.
    Discarded(io::Result<()>),
}
fn file_payload(paths: Vec<PathBuf>) -> Result<ClipboardCompletion, PlatformError> {
    if paths.is_empty() || paths.len() > 1024 || paths.iter().any(|path| !path.is_absolute()) {
        return Err(PlatformError::Budget(
            "Clipboard file list must contain 1–1024 absolute paths",
        ));
    }
    Ok(ClipboardCompletion::Files(paths))
}
/// Serializes clipboard staging and explicit board disposal.
pub struct ClipboardWorker {
    /// A bounded queue prevents callers accumulating work.
    pub requests: crossbeam_channel::Sender<ClipboardRequest>,
    /// Backpressure caps memory while the UI is busy.
    pub results: crossbeam_channel::Receiver<ClipboardCompletion>,
}
impl ClipboardWorker {
    /// Clipboard calls and recovery writes run on the worker, including initialization.
    pub fn new(
        storage: Arc<DesktopStorage>,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        let (requests, rx) = crossbeam_channel::bounded(1);
        let (tx, results) = crossbeam_channel::bounded(1);
        std::thread::Builder::new()
            .name("clipboard".into())
            .spawn(move || {
                while let Ok(request) = rx.recv() {
                    if matches!(request, ClipboardRequest::Discard) {
                        let result = storage.discard();
                        let discarded = result.is_ok();
                        if tx.send(ClipboardCompletion::Discarded(result)).is_err() {
                            break;
                        }
                        wake();
                        if discarded {
                            break;
                        }
                        continue;
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut clipboard = arboard::Clipboard::new()?;
                        if let Ok(paths) = clipboard.get().file_list()
                            && !paths.is_empty()
                        {
                            return file_payload(paths);
                        }
                        let image = match clipboard.get_image() {
                            Ok(image) => {
                                let width = u32::try_from(image.width)
                                    .map_err(|_| PlatformError::Dimensions)?;
                                let height = u32::try_from(image.height)
                                    .map_err(|_| PlatformError::Dimensions)?;
                                storage.stage_preview(width, height, &image.bytes)?
                            }
                            Err(arboard::Error::ContentNotAvailable) => {
                                let html = clipboard.get().html();
                                let text = clipboard.get_text();
                                let url = clipboard_web::source(
                                    html.as_deref().ok(),
                                    text.as_deref().ok(),
                                )?;
                                clipboard_web::import(&storage, url.as_str())?
                            }
                            Err(error) => return Err(error.into()),
                        };
                        Ok(ClipboardCompletion::Pasted(Ok(image)))
                    }))
                    .unwrap_or_else(|_| Err(PlatformError::Panic));
                    let completion =
                        result.unwrap_or_else(|error| ClipboardCompletion::Pasted(Err(error)));
                    if tx.send(completion).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self { requests, results })
    }
}

#[cfg(not(target_os = "windows"))]
fn atomic_replace(source: &std::path::Path, target: &std::path::Path) -> io::Result<()> {
    std::fs::rename(source, target)
}
#[cfg(target_os = "windows")]
fn atomic_replace(source: &std::path::Path, target: &std::path::Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are NUL-terminated and live for the duration of the call.
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::ReplaceFileW(
            target.as_ptr(),
            source.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if result == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(windows_sys::Win32::Foundation::ERROR_FILE_NOT_FOUND as i32)
        {
            return Err(error);
        }
        // ReplaceFileW requires a destination; first writes move without overwriting a raced creation.
        // SAFETY: both paths are NUL-terminated and live for the duration of the call.
        let moved = unsafe {
            windows_sys::Win32::Storage::FileSystem::MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
            )
        };
        if moved == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_ranges_cannot_be_used_as_writable_destinations() {
        let root = std::env::temp_dir().join(format!("mothref-read-only-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        let storage = DesktopStorage::in_directory(root.clone());
        let path = root.join("journal");
        std::fs::write(&path, b"prefixIMAGEsuffix").unwrap();
        let range = storage
            .register_range(
                path.clone(),
                moth_format::board::Blob {
                    offset: 6,
                    length: 5,
                },
            )
            .unwrap();
        assert!(storage.append(range).is_err());
        assert!(storage.sync(range).is_err());
        assert!(storage.temporary(range).is_err());
        let temporary = storage.register(root.join("temporary")).unwrap();
        std::fs::write(root.join("temporary"), b"replacement").unwrap();
        assert!(storage.replace(temporary, range).is_err());
        assert!(storage.replace(range, temporary).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"prefixIMAGEsuffix");
        storage.forget(range).unwrap();
        assert!(storage.open(range).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn atomic_replacement_creates_then_replaces_and_preserves_on_failure() {
        let root = std::env::temp_dir().join(format!("mothref-replace-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("temporary");
        let target = root.join("destination");
        for bytes in [b"first", b"after"] {
            std::fs::write(&source, bytes).unwrap();
            atomic_replace(&source, &target).unwrap();
            assert_eq!(std::fs::read(&target).unwrap(), bytes);
            assert!(!source.exists());
        }
        assert!(atomic_replace(&source, &target).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"after");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn relative_file_replacement_syncs_the_current_directory() {
        let path = PathBuf::from(format!(".mothref-relative-test-{}", uuid::Uuid::now_v7()));
        let mut original = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        original.write_all(b"before").unwrap();
        drop(original);
        let storage = DesktopStorage::default();
        let handle = storage.register(path.clone()).unwrap();
        let temporary = storage.temporary(handle).unwrap();
        storage
            .append(temporary)
            .unwrap()
            .write_all(b"after")
            .unwrap();
        storage.sync(temporary).unwrap();
        let result = storage.replace(temporary, handle);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(result.is_ok(), "relative replacement failed: {result:?}");
        assert_eq!(bytes, b"after");
    }
    #[test]
    fn clipboard_files_preserve_order_and_enforce_backlog_budget() {
        let first = std::env::temp_dir().join("first.png");
        let second = std::env::temp_dir().join("second.jpg");
        match file_payload(vec![first.clone(), second.clone()]).unwrap() {
            ClipboardCompletion::Files(paths) => assert_eq!(paths, [first.clone(), second]),
            _ => panic!("file clipboard payload was not preserved"),
        }
        assert!(file_payload(Vec::new()).is_err());
        assert!(file_payload(vec![PathBuf::from("relative.png")]).is_err());
        assert!(file_payload(vec![first.clone(); 1024]).is_ok());
        assert!(file_payload(vec![first; 1025]).is_err());
    }
    #[test]
    fn worker_acknowledges_discard_after_cleanup() {
        let storage = Arc::new(DesktopStorage::default());
        let handle = storage.stage_rgba(1, 1, &[0, 0, 0, 255]).unwrap();
        let path = storage.path(handle).unwrap();
        let worker = ClipboardWorker::new(storage, Arc::new(|| {})).unwrap();
        worker.requests.send(ClipboardRequest::Discard).unwrap();
        assert!(matches!(
            worker
                .results
                .recv_timeout(std::time::Duration::from_secs(5)),
            Ok(ClipboardCompletion::Discarded(Ok(())))
        ));
        assert!(!path.exists());
    }
    #[test]
    fn clipboard_originals_are_compressed_and_board_scoped() {
        let storage = DesktopStorage::default();
        let pixels = [20, 40, 60, 255].repeat(256 * 256);
        let (handle, preview) = storage.stage_preview(256, 256, &pixels).unwrap();
        let path = storage.path(handle).unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() < pixels.len() as u64 / 4);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(moth_io::decode(&bytes, 256).unwrap().mips[0].bytes, pixels);
        assert_eq!(preview.hash, moth_io::decode(&bytes, 256).unwrap().hash);
        assert!(path.starts_with(data_directory().unwrap()));
        let other = DesktopStorage::default();
        let other_handle = other.stage_rgba(1, 1, &[0, 0, 0, 255]).unwrap();
        let other_path = other.path(other_handle).unwrap();
        storage.discard().unwrap();
        assert!(!path.exists());
        assert!(other_path.exists());
        other.discard().unwrap();
    }
}

/// Native target selection and event-driven stacking without owning foreign windows.
pub mod pin;

/// Bounded diagnostic logging for GUI launches without a terminal.
pub mod diagnostics;
