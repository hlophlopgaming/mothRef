// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Importers that turn foreign board files into MothRef scene data.
//!
//! Importers read through [`moth_format::ReadSeek`] and never touch native
//! paths, so they run on the storage worker and stay portable to Android and
//! WASM. Decoding pixels is the caller's job; this crate only extracts bytes
//! and geometry.
pub mod bee;
mod sqlite;

pub mod pur;
