// SPDX-License-Identifier: GPL-3.0-or-later
//! Native paths and clipboard access stay on the desktop side of the boundary.
pub mod input;
use moth_format::{Handle, ReadSeek, Storage};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
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
    /// OS dimensions must fit the portable image representation.
    #[error("invalid clipboard dimensions")]
    Dimensions,
    /// A native backend panic becomes an ordinary failed paste.
    #[error("clipboard backend panicked")]
    Panic,
}
/// The desktop catalog stores source identities, never image pixels.
#[derive(Default)]
pub struct DesktopStorage {
    paths: Mutex<HashMap<Handle, PathBuf>>,
}
impl DesktopStorage {
    /// Registering an OS-provided path performs no filesystem access.
    pub fn register(&self, path: PathBuf) -> io::Result<Handle> {
        let mut paths = self
            .paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?;
        let handle = Handle(paths.len() as u64);
        paths.insert(handle, path);
        Ok(handle)
    }
    /// Persists clipboard originals on a worker before acknowledging a paste.
    pub fn stage_rgba(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<Handle, PlatformError> {
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
        let directory = std::env::temp_dir().join("mothref-recovery");
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
        // Reject new pastes rather than silently evicting clipboard originals.
        if used.saturating_add(rgba.len() as u64 + 48) > 512 * 1024 * 1024 {
            return Err(PlatformError::Budget(
                "Clipboard recovery budget full (512 MiB)",
            ));
        }
        let path = directory.join(format!("{}.rgba", uuid::Uuid::now_v7()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path)?;
        let mut writer = io::BufWriter::new(file);
        moth_io::write_rgba(&mut writer, width, height, rgba)?;
        io::Write::flush(&mut writer)?;
        writer.get_ref().sync_all()?;
        #[cfg(unix)]
        {
            File::open(&directory)?.sync_all()?;
        }
        Ok(self.register(path)?)
    }
    /// Reuses clipboard pixels for the first preview while staging the original durably.
    pub fn stage_preview(
        &self,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> Result<(Handle, moth_io::Decoded), PlatformError> {
        let preview = moth_io::preview_rgba(width, height, pixels, 256)?;
        let handle = self.stage_rgba(width, height, pixels)?;
        Ok((handle, preview))
    }
    fn path(&self, handle: Handle) -> io::Result<PathBuf> {
        self.paths
            .lock()
            .map_err(|_| io::Error::other("storage lock poisoned"))?
            .get(&handle)
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown storage handle"))
    }
}
impl Storage for DesktopStorage {
    fn open(&self, h: Handle) -> io::Result<Box<dyn ReadSeek>> {
        Ok(Box::new(File::open(self.path(h)?)?))
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
    /// Lets callers adapt controls to available native operations.
    fn capabilities(&self) -> Capabilities;
}
/// M0 native clipboard backend; overlay capabilities are added with M3.
pub struct Desktop;
impl Platform for Desktop {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            clipboard_images: true,
        }
    }
}
/// Bounded requests avoid spawning a thread per paste.
pub struct ClipboardWorker {
    /// A bounded queue prevents callers accumulating work.
    pub requests: crossbeam_channel::Sender<()>,
    /// Backpressure caps memory while the UI is busy.
    pub results: crossbeam_channel::Receiver<Result<(Handle, moth_io::Decoded), PlatformError>>,
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
                while rx.recv().is_ok() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut clipboard = arboard::Clipboard::new()?;
                        let image = clipboard.get_image()?;
                        let width =
                            u32::try_from(image.width).map_err(|_| PlatformError::Dimensions)?;
                        let height =
                            u32::try_from(image.height).map_err(|_| PlatformError::Dimensions)?;
                        storage.stage_preview(width, height, &image.bytes)
                    }))
                    .unwrap_or_else(|_| Err(PlatformError::Panic));
                    if tx.send(result).is_err() {
                        break;
                    }
                    wake();
                }
            })?;
        Ok(Self { requests, results })
    }
}
