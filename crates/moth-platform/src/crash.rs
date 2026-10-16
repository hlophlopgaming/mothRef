// SPDX-License-Identifier: GPL-3.0-or-later
//! Crash reports are written by a dedicated worker, including from panic hooks.
use std::{
    io::{self, Write},
    sync::Arc,
};
/// Installs native termination notifications before starting the application.
/// Acknowledged commands are already synced; shutdown can exit without a dialog.
pub fn shutdown(wake: Arc<dyn Fn() + Send + Sync>) -> io::Result<()> {
    ctrlc::set_handler(move || wake()).map_err(io::Error::other)
}
/// Keeps panic-time filesystem access off the panicking/event-loop thread.
pub fn install() -> io::Result<()> {
    let (tx, rx) = crossbeam_channel::bounded::<String>(1);
    let (ack, done) = crossbeam_channel::bounded(1);
    std::thread::Builder::new()
        .name("crash-report".into())
        .spawn(move || {
            while let Ok(report) = rx.recv() {
                let result = (|| -> io::Result<()> {
                    let directory = super::data_directory()?.join("crashes");
                    std::fs::create_dir_all(&directory)?;
                    // Only app-owned reports rotate; board recovery files are never evicted.
                    let mut reports =
                        std::fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
                    reports.sort_by_key(|e| e.file_name());
                    let excess = reports.len().saturating_sub(9);
                    for report in reports.into_iter().take(excess) {
                        std::fs::remove_file(report.path())?;
                    }
                    let mut options = std::fs::OpenOptions::new();
                    options.write(true).create_new(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::OpenOptionsExt;
                        options.mode(0o600);
                    }
                    let mut file =
                        options.open(directory.join(format!("{}.txt", uuid::Uuid::now_v7())))?;
                    file.write_all(report.as_bytes())?;
                    file.sync_all()
                })();
                if let Err(e) = result {
                    tracing::error!(%e,"crash report write failed");
                }
                let _ = ack.send(());
            }
        })?;
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if tx
            .try_send(format!(
                "MothRef {}\n{info}\n{}",
                env!("CARGO_PKG_VERSION"),
                std::backtrace::Backtrace::force_capture()
            ))
            .is_ok()
        {
            // Only a panic waits, never an input or frame path.
            let _ = done.recv_timeout(std::time::Duration::from_secs(2));
        }
        previous(info);
    }));
    Ok(())
}
