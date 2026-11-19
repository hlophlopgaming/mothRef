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

/// Where to file a bug report.
pub const ISSUES: &str = "https://github.com/hlophlopgaming/mothRef/issues/new";

/// What the previous run left behind when it did not close normally: a panic report
/// if Rust caught one, and the end of the log either way. Native crashes (out of
/// memory, a driver fault, an access violation) leave only the log.
#[derive(Clone, Debug, Default)]
pub struct Report {
    /// The panic message and backtrace, when the previous run panicked.
    pub panic: Option<String>,
    /// The last lines the previous run logged.
    pub log: String,
}

impl Report {
    /// The report as plain text for the clipboard or an issue.
    pub fn text(&self) -> String {
        let mut text = format!(
            "MothRef {} on {} ({})\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        if let Some(panic) = &self.panic {
            text.push_str("\nPanic:\n");
            text.push_str(panic.trim_end());
            text.push('\n');
        }
        text.push_str("\nLast log lines:\n");
        text.push_str(self.log.trim_end());
        text.push('\n');
        text
    }
    /// A link that opens a new GitHub issue with the report filled in. Browsers and
    /// GitHub cut very long links, so the body keeps the panic and the newest log
    /// lines within about 6000 characters; the full log stays on disk.
    pub fn issue_url(&self, title: &str, what: &str) -> String {
        const BUDGET: usize = 6000;
        let mut body = format!(
            "**What happened**\n{what}\n\n**Version:** MothRef {} on {} ({})\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        if let Some(panic) = &self.panic {
            let panic: String = panic.chars().take(BUDGET / 2).collect();
            body.push_str(&format!("\n**Panic**\n```\n{}\n```\n", panic.trim_end()));
        }
        let room = BUDGET.saturating_sub(body.len()).max(500);
        let log = self.log.trim_end();
        let start = log
            .char_indices()
            .map(|(i, _)| i)
            .find(|i| log.len() - i <= room)
            .unwrap_or(log.len());
        // Start at a whole line.
        let tail = match log[start..].find('\n') {
            Some(line) if start > 0 => &log[start + line + 1..],
            _ => &log[start..],
        };
        body.push_str(&format!("\n**Last log lines**\n```\n{tail}\n```\n"));
        format!(
            "{ISSUES}?title={}&body={}&labels=crash",
            encode(title),
            encode(&body)
        )
    }
}

fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Marks this process as running until [`Session`] is dropped on a normal exit,
/// and reports whether an earlier run ended without closing. Each run holds a lock
/// on its own marker, so another MothRef still running is never mistaken for a
/// crash: only markers nobody holds belong to runs that died.
pub fn begin_session() -> (Option<Session>, Option<Report>) {
    let Ok(directory) = super::data_directory() else {
        return (None, None);
    };
    let markers = directory.join("sessions");
    if std::fs::create_dir_all(&markers).is_err() {
        return (None, None);
    }
    let mut crashed = false;
    let mut since = None;
    if let Ok(entries) = std::fs::read_dir(&markers) {
        for entry in entries.flatten().take(64) {
            let path = entry.path();
            let Ok(file) = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
            else {
                continue;
            };
            // Still locked: that MothRef is running.
            if file.try_lock().is_err() {
                continue;
            }
            crashed = true;
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                since = Some(since.map_or(modified, |s: std::time::SystemTime| s.min(modified)));
            }
            drop(file);
            let _ = std::fs::remove_file(&path);
        }
    }
    let report = crashed.then(|| Report {
        panic: latest_panic(&directory.join("crashes"), since),
        log: log_tail(&directory.join("logs"), 200),
    });
    let path = markers.join(format!("{}.lock", std::process::id()));
    let session = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .ok()
        .filter(|file| file.try_lock().is_ok())
        .map(|file| Session {
            file: Some(file),
            path,
        });
    (session, report)
}

/// This run's marker; dropping it on a normal exit says the run closed cleanly.
pub struct Session {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}
impl Drop for Session {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = std::fs::remove_file(&self.path);
    }
}

fn latest_panic(
    directory: &std::path::Path,
    since: Option<std::time::SystemTime>,
) -> Option<String> {
    let newest = std::fs::read_dir(directory)
        .ok()?
        .flatten()
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .filter(|(modified, _)| since.is_none_or(|since| *modified >= since))
        .max_by_key(|(modified, _)| *modified)?;
    let text = std::fs::read_to_string(newest.1).ok()?;
    Some(text.chars().take(16 * 1024).collect())
}

fn log_tail(directory: &std::path::Path, lines: usize) -> String {
    use io::{Read, Seek};
    let Ok(mut file) = std::fs::File::open(directory.join("mothref.log")) else {
        return String::new();
    };
    let length = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = length.saturating_sub(64 * 1024);
    let mut bytes = Vec::new();
    if file.seek(io::SeekFrom::Start(start)).is_err()
        || file.take(64 * 1024).read_to_end(&mut bytes).is_err()
    {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn issue_links_are_encoded_and_bounded() {
        let report = Report {
            panic: Some("panicked at 'boom' & more".into()),
            log: (0..5000).map(|i| format!("line {i}\n")).collect(),
        };
        let url = report.issue_url("Crash: boom", "It closed");
        assert!(url.starts_with(ISSUES));
        assert!(url.contains("title=Crash%3A%20boom"));
        assert!(url.contains("%26%20more"), "& is encoded");
        assert!(url.len() < 3 * 7000 + ISSUES.len() + 200);
        assert!(url.contains("line%204999"), "the newest lines are kept");
        assert!(!url.contains("line%200%0A"), "the oldest lines are dropped");
        assert!(report.text().contains("Last log lines"));
    }
}
