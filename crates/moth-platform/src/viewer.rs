// SPDX-License-Identifier: GPL-3.0-or-later
//! OS associations launch only validated images prepared by the storage worker.
use std::{io, path::Path};

#[cfg(target_os = "macos")]
pub(super) fn open(path: &Path) -> io::Result<()> {
    launch(std::process::Command::new("open").arg(path))
}

/// Desktop launchers in the order they are tried. `xdg-open` is the standard one, but
/// minimal and some immutable systems ship only GLib's `gio` or KDE's `kde-open`.
#[cfg(all(unix, not(target_os = "macos")))]
const LAUNCHERS: &[&[&str]] = &[
    &["xdg-open"],
    &["gio", "open"],
    &["kde-open5"],
    &["kde-open"],
];

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn open(path: &Path) -> io::Result<()> {
    open_with(LAUNCHERS, path)
}

/// Tries each launcher until one exists; only a missing program moves to the next.
#[cfg(all(unix, not(target_os = "macos")))]
fn open_with(launchers: &[&[&str]], path: &Path) -> io::Result<()> {
    for launcher in launchers {
        let Some((program, args)) = launcher.split_first() else {
            continue;
        };
        match launch(std::process::Command::new(program).args(args).arg(path)) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            result => return result,
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "No system image viewer found; install xdg-utils or set a default image viewer",
    ))
}

// A launcher may wait for its viewer to close; it must never hold the storage queue.
#[cfg(not(target_os = "windows"))]
fn launch(command: &mut std::process::Command) -> io::Result<()> {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    };
    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    struct Permit;
    impl Drop for Permit {
        fn drop(&mut self) {
            ACTIVE.fetch_sub(1, Ordering::Release);
        }
    }
    if ACTIVE.fetch_add(1, Ordering::AcqRel) >= 4 {
        ACTIVE.fetch_sub(1, Ordering::Release);
        return Err(io::Error::other(
            "System viewer is busy; try again after closing a viewer",
        ));
    }
    let permit = Permit;
    let mut child = command.spawn()?;
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("image-viewer".into())
        .spawn(move || {
            let _permit = permit;
            let result = child.wait().and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(
                        "System image viewer could not open the image",
                    ))
                }
            });
            if let Err(error) = &result {
                tracing::warn!(%error, "system image viewer failed");
            }
            let _ = sender.send(result);
        })?;
    match receiver.recv_timeout(std::time::Duration::from_millis(100)) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(io::Error::other("System image viewer launcher stopped"))
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) fn open(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: path is NUL-terminated and lives through the call; no owner window is needed.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if result as isize > 32 {
        Ok(())
    } else {
        Err(io::Error::other(
            "System image viewer could not open the image",
        ))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn a_failed_spawn_returns_the_error_and_releases_the_launch_budget() {
        for _ in 0..8 {
            let error = launch(&mut std::process::Command::new(
                "/mothref-no-such-image-viewer",
            ))
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::NotFound);
        }
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn missing_launchers_fall_through_to_a_readable_error() {
        let error = open_with(
            &[
                &["/mothref-no-such-launcher"],
                &["/mothref-no-such-launcher-2", "open"],
            ],
            Path::new("/tmp/image.png"),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(error.to_string().contains("No system image viewer found"));
    }
}
