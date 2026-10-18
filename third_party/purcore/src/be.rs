// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 achrllrogia45
// Source: https://github.com/achrllrogia45/LazyRef
// Derived from achrllrogia45/LazyRef, commit e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654.
// Original copyright and permission notice: ../LICENSE.
//! Big-endian read/write helpers.
//!
//! Bounds failures return typed errors without indexing untrusted ranges.

fn bytes<const N: usize>(b: &[u8], off: usize) -> Result<[u8; N], crate::Error> {
    let end = off.checked_add(N).ok_or(crate::Error::Invalid)?;
    b.get(off..end)
        .ok_or(crate::Error::Invalid)?
        .try_into()
        .map_err(|_| crate::Error::Invalid)
}
pub(crate) fn i8(b: &[u8], off: usize) -> Result<i8, crate::Error> {
    b.get(off).map(|v| *v as i8).ok_or(crate::Error::Invalid)
}
pub(crate) fn u16(b: &[u8], off: usize) -> Result<u16, crate::Error> {
    Ok(u16::from_be_bytes(bytes(b, off)?))
}
pub(crate) fn u32(b: &[u8], off: usize) -> Result<u32, crate::Error> {
    Ok(u32::from_be_bytes(bytes(b, off)?))
}
pub(crate) fn i32(b: &[u8], off: usize) -> Result<i32, crate::Error> {
    Ok(i32::from_be_bytes(bytes(b, off)?))
}
pub(crate) fn u64(b: &[u8], off: usize) -> Result<u64, crate::Error> {
    Ok(u64::from_be_bytes(bytes(b, off)?))
}
pub(crate) fn f64(b: &[u8], off: usize) -> Result<f64, crate::Error> {
    Ok(f64::from_be_bytes(bytes(b, off)?))
}
pub(crate) fn utf16_be_to_string(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes(*p))
        .collect();
    String::from_utf16_lossy(&units)
}
