// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Byte-stream boundary shared by desktop and future URI/OPFS storage backends.
use std::io::{self, Read, Seek, Write};

/// Allows readers to navigate a board without knowing how its storage is addressed.
pub trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// Opaque backend-local identity, deliberately independent of filesystem paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle(pub u64);

/// Storage operations run on a worker, never on the event-loop thread.
pub trait Storage: Send + Sync {
    /// Opens an existing object without exposing its native path to core crates.
    fn open(&self, handle: Handle) -> io::Result<Box<dyn ReadSeek>>;
    /// Appends without destroying an existing checkpoint.
    fn append(&self, handle: Handle) -> io::Result<Box<dyn Write + Send>>;
    /// Makes prior writes durable before acknowledging commands.
    fn sync(&self, handle: Handle) -> io::Result<()>;
}
