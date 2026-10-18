// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Local safety fork of LazyRef's PureRef codecs; see ../README.md.
macro_rules! require {
    ($condition:expr) => {
        if !$condition {
            return Err(crate::Error::Invalid);
        }
    };
}
mod be;
pub mod blobs;
pub mod checksum;

pub mod legacy;
pub mod model;
pub mod sqlite;
mod stream;
pub use stream::{Entry, Error, MAX_FILE_BYTES, read};
