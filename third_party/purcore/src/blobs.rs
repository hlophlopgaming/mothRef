// SPDX-License-Identifier: MIT AND GPL-3.0-or-later
// Copyright (c) 2026 achrllrogia45
// Source: https://github.com/achrllrogia45/LazyRef
// Derived from achrllrogia45/LazyRef, commit e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654.
// Original copyright and permission notice: ../LICENSE.
//! PureRef 2.x blob codecs — the binary payloads stored in blob-typed columns.
//!
//! All columns are one family: `QDataStream << QVariant` with stream version 19 (Qt_5_15,
//! pre-Qt-6 type ids), then stored via `QString::fromLatin1(bytes)` into a TEXT column.
//! On disk that means the column holds the UTF-8 of the latin-1-decoded serialization
//! ("storage quirk"): read with [`stored_text_to_bytes`], write with [`bytes_to_stored_text`]
//! and bind as TEXT — never as BLOB.
//!
//! Wire envelope:
//!   `[u32 BE typeId][i8 isNull][if typeId in (1024, 65536): u32 nameLen(incl NUL) + name\0][payload]`

use crate::be;

/// Qt5 `QTransform` type id (stream version 19 maps Qt6 0x1010 down to 80).
pub const TYPE_TRANSFORM: u32 = 80;

/// Qt5 `QRectF` type id.
pub const TYPE_QRECTF: u32 = 20;

/// `QMetaType::User` in the Qt5 id space — named user types (BigRational, QPainterPath).
pub const TYPE_USER: u32 = 1024;

/// Qt6 portability form of user types; accepted on read, never written by PureRef 2.1.3.
pub const TYPE_USER_QT6: u32 = 65536;

// ---------------------------------------------------------------- model types

/// QTransform — always 9 doubles BE: m11 m12 m13 / m21 m22 m23 / m31 m32 m33.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QTransform {
    pub m11: f64,
    pub m12: f64,
    pub m13: f64,
    pub m21: f64,
    pub m22: f64,
    pub m23: f64,
    pub m31: f64,
    pub m32: f64,
    pub m33: f64,
}

impl QTransform {
    pub const IDENTITY: QTransform = QTransform {
        m11: 1.0,
        m12: 0.0,
        m13: 0.0,
        m21: 0.0,
        m22: 1.0,
        m23: 0.0,
        m31: 0.0,
        m32: 0.0,
        m33: 1.0,
    };

    /// Values in wire order (m11..m33).
    pub fn values(&self) -> [f64; 9] {
        [
            self.m11, self.m12, self.m13, self.m21, self.m22, self.m23, self.m31, self.m32,
            self.m33,
        ]
    }
}

/// QRectF — 4 doubles BE: x y w h (metadata.scene_rect).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QRectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// BigRational — `items.sort_order` order key.
///
/// Limbs are u32 values in [0, 0xFFFFFFFF], little-endian (limb[0] = least significant).
/// Zero is encoded as `sign=0, limbs=[0]` (not empty) — preserve field-for-field.
#[derive(Debug, Clone, PartialEq)]
pub struct BigRational {
    pub num_sign: i32,
    pub num_limbs: Vec<u32>,
    pub den_sign: i32,
    pub den_limbs: Vec<u32>,
}

impl BigRational {
    /// Approximate float value, matching the Python reference (denominator sign ignored).
    ///
    /// ponytail: f64 limb accumulation instead of BigInteger/BigDecimal-DECIMAL128 division;
    /// identical for small ratios (the only ones the app stores); swap in exact big-int
    /// division if huge sort-order limbs ever surface.
    pub fn value(&self) -> f64 {
        if self.den_limbs.is_empty() {
            return f64::NAN;
        }
        let den = limbs_to_f64(&self.den_limbs);
        if den == 0.0 {
            return f64::NAN;
        }
        let v = limbs_to_f64(&self.num_limbs) / den;
        if self.num_sign >= 0 { v } else { -v }
    }
}

/// Odd-even `< x >` sign convention used by the app.
pub fn sign_of(x: i64) -> i32 {
    match x {
        x if x > 0 => 1,
        x if x < 0 => -1,
        _ => 0,
    }
}

/// QPainterPath element — type: 0=MoveTo 1=LineTo 2=CurveTo 3=CurveToData.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathElement {
    pub r#type: i32,
    pub x: f64,
    pub y: f64,
}

/// QPainterPath — `items_images.image_bounds` (image-pixel coords, centered on origin).
#[derive(Debug, Clone, PartialEq)]
pub struct QPainterPath {
    pub elements: Vec<PathElement>,
    pub c_start: i32,
    pub fill_rule: i32,
}

/// Decoded QVariant envelope.
#[derive(Debug)]
pub struct Variant<'a> {
    pub type_id: u32,
    pub name: Option<String>,
    pub is_null: i8,
    pub payload: &'a [u8],
}

// ------------------------------------------------------------- storage quirk

/// Serialized bytes -> value to bind into the TEXT column (`QString::fromLatin1`).
pub fn bytes_to_stored_text(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

/// TEXT column value -> original serialized bytes (`str.encode('latin-1')`; chars > 0xFF
/// become '?' exactly like `String.toByteArray(ISO_8859_1)`).
pub fn stored_text_to_bytes(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| if (c as u32) <= 0xFF { c as u8 } else { b'?' })
        .collect()
}

// ---------------------------------------------------------------- envelope

pub fn decode_variant(b: &[u8]) -> Result<Variant<'_>, crate::Error> {
    let mut off = 0;
    let type_id = be::u32(b, off)?;
    off += 4;
    let is_null = be::i8(b, off)?;
    off += 1;
    let mut name = None;
    if type_id == TYPE_USER || type_id == TYPE_USER_QT6 {
        let n = be::u32(b, off)? as usize;
        off += 4;
        require!(n >= 1 && n <= b.len() - off);
        name = Some(String::from_utf8_lossy(&b[off..off + n - 1]).into_owned());
        off += n;
    }
    Ok(Variant {
        type_id,
        name,
        is_null,
        payload: &b[off..],
    })
}

pub fn encode_variant(type_id: u32, payload: &[u8], name: Option<&str>, is_null: i8) -> Vec<u8> {
    let name_bytes = name.map(|n| {
        let mut v = n.as_bytes().to_vec();
        v.push(0); // NUL-terminated
        v
    });
    let mut out =
        Vec::with_capacity(4 + 1 + name_bytes.as_ref().map_or(0, |v| 4 + v.len()) + payload.len());
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(is_null as u8);
    if let Some(nb) = name_bytes {
        out.extend_from_slice(&(nb.len() as u32).to_be_bytes());
        out.extend_from_slice(&nb);
    }
    out.extend_from_slice(payload);
    out
}

// --------------------------------------------------------------- QTransform

pub fn decode_transform(b: &[u8]) -> Result<QTransform, crate::Error> {
    let v = decode_variant(b)?;
    require!(
        v.type_id == TYPE_TRANSFORM && v.name.is_none() && v.is_null == 0 && v.payload.len() == 72
    );
    let p = v.payload;
    Ok(QTransform {
        m11: be::f64(p, 0)?,
        m12: be::f64(p, 8)?,
        m13: be::f64(p, 16)?,
        m21: be::f64(p, 24)?,
        m22: be::f64(p, 32)?,
        m23: be::f64(p, 40)?,
        m31: be::f64(p, 48)?,
        m32: be::f64(p, 56)?,
        m33: be::f64(p, 64)?,
    })
}

pub fn encode_transform(t: &QTransform) -> Vec<u8> {
    let mut p = Vec::with_capacity(72);
    for v in t.values() {
        p.extend_from_slice(&v.to_be_bytes());
    }
    encode_variant(TYPE_TRANSFORM, &p, None, 0)
}

// ------------------------------------------------------------------ QRectF

pub fn decode_rect_f(b: &[u8]) -> Result<QRectF, crate::Error> {
    let v = decode_variant(b)?;
    require!(
        v.type_id == TYPE_QRECTF && v.name.is_none() && v.is_null == 0 && v.payload.len() == 32
    );
    let p = v.payload;
    Ok(QRectF {
        x: be::f64(p, 0)?,
        y: be::f64(p, 8)?,
        w: be::f64(p, 16)?,
        h: be::f64(p, 24)?,
    })
}

pub fn encode_rect_f(r: &QRectF) -> Vec<u8> {
    let mut p = Vec::with_capacity(32);
    for v in [r.x, r.y, r.w, r.h] {
        p.extend_from_slice(&v.to_be_bytes());
    }
    encode_variant(TYPE_QRECTF, &p, None, 0)
}

// -------------------------------------------------------------- BigRational

pub fn decode_big_rational(b: &[u8]) -> Result<BigRational, crate::Error> {
    let v = decode_variant(b)?;
    require!(v.type_id == TYPE_USER && v.name.as_deref() == Some("BigRational") && v.is_null == 0);
    let p = v.payload;
    let mut off = 0;
    let num_sign = be::i32(p, off)?;
    off += 4;
    let num_limbs = read_limbs(p, &mut off)?;
    let den_sign = be::i32(p, off)?;
    off += 4;
    let den_limbs = read_limbs(p, &mut off)?;
    require!(off == p.len());
    Ok(BigRational {
        num_sign,
        num_limbs,
        den_sign,
        den_limbs,
    })
}

pub fn encode_big_rational(r: &BigRational) -> Vec<u8> {
    let mut p = Vec::with_capacity(2 * (4 + 8) + 4 * (r.num_limbs.len() + r.den_limbs.len()));
    p.extend_from_slice(&r.num_sign.to_be_bytes());
    p.extend_from_slice(&(r.num_limbs.len() as u64).to_be_bytes());
    for &l in &r.num_limbs {
        p.extend_from_slice(&l.to_be_bytes());
    }
    p.extend_from_slice(&r.den_sign.to_be_bytes());
    p.extend_from_slice(&(r.den_limbs.len() as u64).to_be_bytes());
    for &l in &r.den_limbs {
        p.extend_from_slice(&l.to_be_bytes());
    }
    encode_variant(TYPE_USER, &p, Some("BigRational"), 0)
}

/// Builds an integer ratio `n/d` (d > 0) in the app's normalized form.
pub fn big_rational_of(n: i64, d: i64) -> BigRational {
    assert!(d > 0, "denominator must be positive: {d}");
    BigRational {
        num_sign: sign_of(n),
        num_limbs: limbs_of(n),
        den_sign: 1,
        den_limbs: limbs_of(d),
    }
}

fn read_limbs(p: &[u8], off: &mut usize) -> Result<Vec<u32>, crate::Error> {
    let n = be::u64(p, *off)?; // u64 count; the app's reader clamps count < 1000
    *off += 8;
    require!(n <= 999);
    let limbs = (0..n as usize)
        .map(|i| be::u32(p, *off + 4 * i))
        .collect::<Result<Vec<_>, _>>()?;
    *off += 4 * n as usize;
    Ok(limbs)
}

fn limbs_of(x: i64) -> Vec<u32> {
    let mut v = x.unsigned_abs();
    if v == 0 {
        return vec![0];
    }
    let mut out = Vec::with_capacity(4);
    while v != 0 {
        out.push((v & 0xFFFF_FFFF) as u32);
        v >>= 32;
    }
    out
}

/// Little-endian u32 limbs -> f64.
fn limbs_to_f64(limbs: &[u32]) -> f64 {
    limbs
        .iter()
        .rev()
        .fold(0.0, |acc, &l| acc * 4294967296.0 + l as f64)
}

// ------------------------------------------------------------- QPainterPath

pub fn decode_path(b: &[u8]) -> Result<QPainterPath, crate::Error> {
    let v = decode_variant(b)?;
    require!(v.type_id == TYPE_USER && v.name.as_deref() == Some("QPainterPath") && v.is_null == 0);
    let p = v.payload;
    let n = be::i32(p, 0)?;
    require!(n >= 0 && (n as usize) <= (p.len() - 4) / 20);
    let mut off = 4;
    let mut elements = Vec::with_capacity(n as usize);
    for _ in 0..n {
        elements.push(PathElement {
            r#type: be::i32(p, off)?,
            x: be::f64(p, off + 4)?,
            y: be::f64(p, off + 12)?,
        });
        off += 20;
    }
    // empty path: payload is a bare u32 0, no cStart/fillRule
    let (c_start, fill_rule) = if n > 0 {
        (be::i32(p, off)?, be::i32(p, off + 4)?)
    } else {
        (0, 0)
    };
    Ok(QPainterPath {
        elements,
        c_start,
        fill_rule,
    })
}

pub fn encode_path(p: &QPainterPath) -> Vec<u8> {
    if p.elements.is_empty() {
        return encode_variant(TYPE_USER, &0u32.to_be_bytes(), Some("QPainterPath"), 0);
    }
    let mut out = Vec::with_capacity(4 + 20 * p.elements.len() + 8);
    out.extend_from_slice(&(p.elements.len() as u32).to_be_bytes());
    for e in &p.elements {
        out.extend_from_slice(&e.r#type.to_be_bytes());
        out.extend_from_slice(&e.x.to_be_bytes());
        out.extend_from_slice(&e.y.to_be_bytes());
    }
    out.extend_from_slice(&p.c_start.to_be_bytes());
    out.extend_from_slice(&p.fill_rule.to_be_bytes());
    encode_variant(TYPE_USER, &out, Some("QPainterPath"), 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_blob_decoders_return_errors_without_panicking() {
        let bytes = encode_transform(&QTransform::IDENTITY);
        for cut in 0..bytes.len() {
            assert!(decode_transform(&bytes[..cut]).is_err());
        }
        assert_eq!(decode_transform(&bytes).unwrap(), QTransform::IDENTITY);
        assert!(decode_path(&[]).is_err());
        assert!(decode_big_rational(&[]).is_err());
    }
}
