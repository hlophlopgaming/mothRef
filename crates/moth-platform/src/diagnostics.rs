// SPDX-License-Identifier: GPL-3.0-or-later
//! Logging never performs disk I/O on the event loop; two files bound disk usage.
use std::io::{self, Write};
const LIMIT: u64 = 2 * 1024 * 1024;
const CHUNK: usize = 8192;

/// Clones share a bounded queue; stderr remains available for deliberate CLI launches.
#[derive(Clone)]
pub struct Writer(crossbeam_channel::Sender<Vec<u8>>);
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(bytes);
        for chunk in bytes.chunks(CHUNK) {
            // shortcut: saturated diagnostic bursts may drop lines; use a larger queue if field logs need it.
            let _ = self.0.try_send(chunk.to_vec());
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// The guard drains queued diagnostics on normal exit.
pub struct Guard(
    Option<std::thread::JoinHandle<()>>,
    crossbeam_channel::Sender<()>,
);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.1.send(());
        if let Some(worker) = self.0.take() {
            let _ = worker.join();
        }
    }
}
/// Attaches only to an existing parent console, never allocating a GUI-launch console.
pub fn parent_console() {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Console::*;
        // SAFETY: queries and attachment use constant identifiers; redirected stderr stays intact.
        unsafe {
            let stderr = GetStdHandle(STD_ERROR_HANDLE);
            if stderr.is_null() || stderr == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
    }
}
/// Creates the worker before tracing starts; startup filesystem access stays on that worker.
pub fn start() -> io::Result<(Writer, Guard)> {
    let (tx, rx) = crossbeam_channel::bounded::<Vec<u8>>(256);
    let (stop, stopped) = crossbeam_channel::bounded(1);
    let worker = std::thread::Builder::new().name("diagnostics".into()).spawn(move || {
        let run = || -> io::Result<()> {
            let directory = super::data_directory()?.join("logs");
            std::fs::create_dir_all(&directory)?;
            let path = directory.join("mothref.log");
            let previous = directory.join("mothref.previous.log");
            let open = || {
                let mut options = std::fs::OpenOptions::new();
                options.create(true).append(true);
                #[cfg(unix)]
                { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
                options.open(&path)
            };
            let first = open()?;
            let mut size = first.metadata()?.len();
            let mut file = Some(first);
            let mut append = |bytes: Vec<u8>| -> io::Result<()> {
                if rotate(size, bytes.len()) {
                    if let Some(mut previous_file) = file.take() { previous_file.flush()?; }
                    if previous.try_exists()? { std::fs::remove_file(&previous)?; }
                    std::fs::rename(&path, &previous)?;
                    file = Some(open()?);
                    size = 0;
                }
                if let Some(file) = &mut file { file.write_all(&bytes)?; }
                size += bytes.len() as u64;
                Ok(())
            };
            loop {
                crossbeam_channel::select! {
                    recv(rx) -> bytes => match bytes { Ok(bytes) => append(bytes)?, Err(_) => break },
                    recv(stopped) -> _ => {
                        for bytes in rx.try_iter() { append(bytes)?; }
                        break;
                    }
                }
            }
            if let Some(mut file) = file { file.flush()?; }
            Ok(())
        };
        if let Err(error) = run() { let _ = writeln!(io::stderr(), "Diagnostic log: {error}"); }
    })?;
    Ok((Writer(tx), Guard(Some(worker), stop)))
}
fn rotate(size: u64, incoming: usize) -> bool {
    size.saturating_add(incoming as u64) > LIMIT
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn log_rotation_is_bounded_even_for_an_old_oversized_file() {
        assert!(!rotate(LIMIT - CHUNK as u64, CHUNK));
        assert!(rotate(LIMIT, 1));
        assert!(rotate(u64::MAX, CHUNK));
    }
}
