// SPDX-License-Identifier: GPL-3.0-or-later
//! Checks official artifacts and delegates replacement to an out-of-process helper.
mod install;
mod release;

pub use install::{Armed, Prepared, Startup, resuming, run_helper, startup};
pub use release::{Release, check, current, skip};
use std::{env, io, path::PathBuf};

/// A retained board opens from its original file or its durable recovery journal.
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Board {
    /// An unchanged board opens normally.
    Saved {
        /// Original file.
        path: std::ffi::OsString,
        /// The old session’s clean journal, superseded by the saved file.
        journal: std::ffi::OsString,
    },
    /// Pending edits are recovered without writing to the original file.
    Journal(std::ffi::OsString),
}

/// Retains tab groups across the restart, with each group's active board last.
pub type Session = Vec<Vec<Board>>;

impl crate::DesktopStorage {
    /// Computes the recovery identity without accessing the filesystem.
    pub fn journal_path(&self) -> io::Result<PathBuf> {
        Ok(self.recovery_directory()?.join("board.moth"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Kind {
    AppImage,
    LinuxArchive,
    WindowsPortable,
    WindowsSetup,
    MacBundle,
}

impl Kind {
    fn asset(self) -> Option<&'static str> {
        match (self, env::consts::ARCH) {
            (Self::AppImage, "x86_64") => Some("mothref-linux-x86_64.AppImage"),
            (Self::AppImage, "aarch64") => Some("mothref-linux-arm64.AppImage"),
            (Self::LinuxArchive, "x86_64") => Some("mothref-linux-x86_64.tar.gz"),
            (Self::LinuxArchive, "aarch64") => Some("mothref-linux-arm64.tar.gz"),
            (Self::WindowsPortable, "x86_64") => Some("mothref-windows-x86_64.zip"),
            (Self::WindowsSetup, "x86_64") => Some("mothref-windows-x86_64-setup.exe"),
            (Self::MacBundle, "x86_64") => Some("mothref-macos-x86_64.tar.gz"),
            (Self::MacBundle, "aarch64") => Some("mothref-macos-arm64.tar.gz"),
            _ => None,
        }
    }
}

struct Installation {
    kind: Kind,
    path: PathBuf,
    executable: PathBuf,
}

impl Installation {
    fn detect() -> io::Result<Option<Self>> {
        // Only the official release workflow opts into replacing its own build.
        if option_env!("MOTHREF_RELEASE_BUILD") != Some("1") {
            return Ok(None);
        }
        let executable = env::current_exe()?.canonicalize()?;
        let (kind, path) = match env::consts::OS {
            "linux" => {
                if env::var_os("FLATPAK_ID").is_some()
                    || std::path::Path::new("/.flatpak-info").exists()
                    || env::var_os("SNAP").is_some()
                {
                    return Ok(None);
                }
                if let Some(image) = env::var_os("APPIMAGE") {
                    let image = PathBuf::from(image).canonicalize()?;
                    use std::io::Read;
                    let mut header = [0; 11];
                    if !image.is_file() {
                        return Ok(None);
                    }
                    std::fs::File::open(&image)?.read_exact(&mut header)?;
                    if &header[..4] != b"\x7fELF" || &header[8..] != b"AI\x02" {
                        return Ok(None);
                    }
                    (Kind::AppImage, image)
                } else {
                    // The AppImage's inner executable must never replace itself.
                    if env::var_os("APPDIR").is_some()
                        || ["/usr", "/bin", "/sbin", "/nix/store", "/gnu/store"]
                            .iter()
                            .any(|prefix| executable.starts_with(prefix))
                    {
                        return Ok(None);
                    }
                    (Kind::LinuxArchive, executable.clone())
                }
            }
            "windows" => {
                let setup = executable
                    .parent()
                    .is_some_and(|parent| parent.join("mothref.install").is_file());
                let kind = if setup {
                    Kind::WindowsSetup
                } else {
                    Kind::WindowsPortable
                };
                (kind, executable.clone())
            }
            "macos" => {
                let Some(bundle) = executable.ancestors().nth(3).filter(|path| {
                    path.extension().is_some_and(|ext| ext == "app")
                        && executable.strip_prefix(path).is_ok_and(|part| {
                            part == std::path::Path::new("Contents/MacOS/mothref")
                        })
                }) else {
                    return Ok(None);
                };
                // Run the copy in Applications or another writable location, not the DMG.
                if bundle.starts_with("/Volumes")
                    || bundle
                        .components()
                        .any(|c| c.as_os_str() == "AppTranslocation")
                {
                    return Ok(None);
                }
                (Kind::MacBundle, bundle.to_owned())
            }
            _ => return Ok(None),
        };
        Ok(kind.asset().map(|_| Self {
            kind,
            path,
            executable,
        }))
    }
}

fn state_directory() -> io::Result<PathBuf> {
    let directory = super::data_directory()?.join("updates");
    std::fs::create_dir_all(&directory)?;
    Ok(directory)
}
