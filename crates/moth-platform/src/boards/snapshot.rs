// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional filesystem snapshots; unsupported filesystems keep the journal copy path.
use super::{append_record, error, recovery, sync_directory};
use crate::DesktopStorage;
use moth_format::board;
use moth_scene::Command;
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

pub(super) struct Prepared {
    pub(super) board: board::Board,
    pub(super) journal: File,
    pub(super) guard: recovery::Guard,
}

/// Only a validated, durable private file may become the session journal.
pub(super) fn prepare(
    storage: &DesktopStorage,
    source: &File,
    source_path: &Path,
) -> io::Result<Option<Prepared>> {
    let mut reader = source;
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0; 16];
    if reader.read_exact(&mut header).is_err() || header[4] != board::SCHEMA {
        return Ok(None);
    }
    let _span = tracing::info_span!("snapshot_board").entered();
    let (directory, guard) = recovery::prepare(storage)?;
    let temporary = directory.join(format!("opening-{}.tmp", uuid::Uuid::now_v7()));
    let Some(mut clone) = clone(source, source_path, &temporary)? else {
        return Ok(None);
    };
    let path = directory.join("board.moth");
    let prepared = (|| -> io::Result<Option<(board::Board, File)>> {
        let length = clone.metadata()?.len();
        if !(16..=8 * 1024 * 1024 * 1024).contains(&length) {
            return Ok(None);
        }
        if clone.read_exact(&mut header).is_err() || header[4] != board::SCHEMA {
            return Ok(None);
        }
        // Validate the immutable snapshot; malformed input keeps the normal reader path.
        let loaded = match board::read(&mut clone) {
            Ok(loaded) if !loaded.recovered => loaded,
            _ => return Ok(None),
        };
        let checkpoint = board::snapshot(&loaded.scene).map_err(error)?;
        let mut rebuilt = 16 + checkpoint.len() as u64 + 96;
        let mut originals_end = 16;
        let mut live = HashSet::new();
        for item in loaded.scene.items() {
            rebuilt += board::encode(&Command::Insert(item.clone())).len() as u64 + 96;
            if let Some(hash) = item.blob()
                && live.insert(hash)
            {
                let blob = loaded
                    .blobs
                    .get(&hash)
                    .ok_or_else(|| error("Missing original"))?;
                rebuilt += blob.length + 96;
                originals_end = originals_end.max(blob.offset + blob.length + 96);
            }
        }
        // Do not retain more history or consume more journal headroom than rebuilding.
        if originals_end + checkpoint.len() as u64 + 96 > rebuilt {
            return Ok(None);
        }
        // Keep original offsets, replacing the old layout history only in our snapshot.
        clone.set_len(originals_end)?;
        clone.seek(SeekFrom::End(0))?;
        append_record(&mut clone, 3, &checkpoint)?;
        clone.sync_all()?;
        std::fs::rename(&temporary, &path)?;
        sync_directory(&directory)?;
        sync_directory(&storage.base_directory()?)?;
        let journal = OpenOptions::new().read(true).append(true).open(&path)?;
        Ok(Some((loaded, journal)))
    })();
    drop(clone);
    match std::fs::remove_file(&temporary) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    prepared.map(|value| {
        value.map(|(board, journal)| Prepared {
            board,
            journal,
            guard,
        })
    })
}

/// A private copy of `source` at `path`: a copy-on-write clone where the filesystem
/// offers one, otherwise a copy made by the operating system (which clones by itself
/// on APFS, ReFS and Dev Drives). `None` when neither works; the caller then stages
/// originals itself.
#[cfg(target_os = "linux")]
pub(super) fn clone(source: &File, source_path: &Path, path: &Path) -> io::Result<Option<File>> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    match rustix::fs::ioctl_ficlone(&file, source) {
        Ok(()) => Ok(Some(file)),
        Err(error) => {
            tracing::debug!(%error, "filesystem snapshot unavailable; copying the board");
            drop(file);
            std::fs::remove_file(path)?;
            copy(source_path, path)
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn clone(_source: &File, source_path: &Path, path: &Path) -> io::Result<Option<File>> {
    copy(source_path, path)
}

fn copy(source: &Path, path: &Path) -> io::Result<Option<File>> {
    let _span = tracing::info_span!("copy_board").entered();
    let copied = std::fs::copy(source, path).and_then(|_| {
        // Recovery files stay private and writable whatever the source allowed.
        #[cfg(unix)]
        let permissions = std::os::unix::fs::PermissionsExt::from_mode(0o600);
        #[cfg(not(unix))]
        let permissions = {
            let mut permissions = std::fs::metadata(path)?.permissions();
            permissions.set_readonly(false);
            permissions
        };
        std::fs::set_permissions(path, permissions)?;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
    });
    match copied {
        Ok(file) => Ok(Some(file)),
        Err(error) => {
            tracing::debug!(%error, "board copy unavailable; staging originals");
            match std::fs::remove_file(path) {
                Ok(()) => Ok(None),
                Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e),
            }
        }
    }
}
