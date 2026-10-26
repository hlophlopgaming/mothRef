// SPDX-License-Identifier: GPL-3.0-or-later
//! Recovery identities are sidecars, so board files remain portable and unchanged.
use super::*;
use std::{path::Path, time::SystemTime};

/// Recovery metadata is read by the storage worker, never by a UI frame.
#[derive(Clone)]
pub struct Recovery {
    /// Private journal identity, passed back unchanged when restoring or discarding.
    pub journal: PathBuf,
    /// Original file stem; unnamed or legacy journals have no recorded title.
    pub title: Option<String>,
    /// Last journal write, used to distinguish several recovery candidates.
    pub modified: SystemTime,
}

pub(super) fn validate(storage: &DesktopStorage, journal: &Path) -> io::Result<PathBuf> {
    let root = storage.base_directory()?.canonicalize()?;
    let directory = journal
        .parent()
        .ok_or_else(|| error("Missing recovery directory"))?;
    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| error("Invalid recovery identity"))?;
    uuid::Uuid::parse_str(name).map_err(error)?;
    if journal.file_name() != Some(std::ffi::OsStr::new("board.moth"))
        || std::fs::symlink_metadata(directory)?
            .file_type()
            .is_symlink()
        || !std::fs::symlink_metadata(journal)?.file_type().is_file()
        || directory
            .parent()
            .ok_or_else(|| error("Missing recovery root"))?
            .canonicalize()?
            != root
    {
        return Err(error(
            "Recovery must belong to the application's recovery folder",
        ));
    }
    Ok(directory.to_owned())
}

pub(super) fn lock(journal: &Path) -> io::Result<File> {
    // Lock a sentinel: Windows range locks on the journal would block independent image readers.
    let path = journal
        .parent()
        .ok_or_else(|| error("Missing recovery directory"))?
        .join("active");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock().map_err(error)?;
    Ok(file)
}

pub(super) fn discover(storage: &DesktopStorage) -> io::Result<Vec<Recovery>> {
    let root = storage.base_directory()?;
    std::fs::create_dir_all(&root)?;
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(root)?.take(1024) {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let journal = entry.path().join("board.moth");
        if validate(storage, &journal).is_err() {
            continue;
        }
        let Ok(_guard) = lock(&journal) else {
            continue;
        };
        let title = destination(&journal).ok().flatten().and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        });
        entries.push(Recovery {
            modified: std::fs::metadata(&journal)?.modified()?,
            journal,
            title,
        });
    }
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.modified));
    Ok(entries)
}

pub(super) fn remember(storage: &DesktopStorage, destination: &Path) -> io::Result<()> {
    let directory = storage.recovery_directory()?;
    let bytes = encode_path(destination);
    if bytes.len() > 64 * 1024 {
        return Err(error("Recovery destination exceeds budget"));
    }
    let mut data = b"MREFPATH1".to_vec();
    data.extend_from_slice(blake3::hash(&bytes).as_bytes());
    data.extend_from_slice(&bytes);
    let path = directory.join("destination");
    let temporary = directory.join(format!("destination-{}.tmp", uuid::Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&data)?;
    file.sync_all()?;
    drop(file);
    super::super::atomic_replace(&temporary, &path)?;
    sync_directory(&directory)
}

pub(super) fn destination(journal: &Path) -> io::Result<Option<PathBuf>> {
    let path = journal
        .parent()
        .ok_or_else(|| error("Missing recovery directory"))?
        .join("destination");
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(64 * 1024 + 42).read_to_end(&mut bytes)?;
    if bytes.len() < 41
        || bytes.len() > 64 * 1024 + 41
        || &bytes[..9] != b"MREFPATH1"
        || blake3::hash(&bytes[41..]).as_bytes() != &bytes[9..41]
    {
        return Err(error("Invalid recovery destination"));
    }
    let destination = decode_path(&bytes[41..])?;
    if !destination.is_absolute() {
        return Err(error("Recovery destination must be absolute"));
    }
    Ok(Some(destination))
}

#[cfg(unix)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}
#[cfg(unix)]
fn decode_path(bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_vec(bytes.to_vec()).into())
}
#[cfg(windows)]
fn encode_path(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}
#[cfg(windows)]
fn decode_path(bytes: &[u8]) -> io::Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    if !bytes.len().is_multiple_of(2) {
        return Err(error("Invalid Windows recovery destination"));
    }
    let wide: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .collect();
    Ok(std::ffi::OsString::from_wide(&wide).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn windows_destinations_roundtrip_and_reject_partial_utf16_units() {
        let path = PathBuf::from(r"C:\References\猫.moth");
        assert_eq!(decode_path(&encode_path(&path)).unwrap(), path);
        assert!(decode_path(&[1]).is_err());
    }
    #[test]
    fn destination_is_checksummed_and_discard_rejects_paths_outside_recovery() {
        let storage = DesktopStorage::in_directory(
            std::env::temp_dir().join(format!("moth-recovery-{}", uuid::Uuid::now_v7())),
        );
        let directory = storage.recovery_directory().unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        let journal = directory.join("board.moth");
        File::create(&journal).unwrap();
        let original = storage.base_directory().unwrap().join("Original name.moth");
        remember(&storage, &original).unwrap();
        assert_eq!(destination(&journal).unwrap(), Some(original));
        assert_eq!(
            discover(&storage).unwrap()[0].title.as_deref(),
            Some("Original name")
        );
        assert!(validate(&storage, &directory.join("unrelated.moth")).is_err());
        let path = directory.join("destination");
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(path, bytes).unwrap();
        assert!(destination(&journal).is_err());
        std::fs::remove_dir_all(storage.base_directory().unwrap()).unwrap();
    }
}
