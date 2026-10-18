// SPDX-License-Identifier: MIT
// Copyright (c) 2026 achrllrogia45
// Source: https://github.com/achrllrogia45/LazyRef
// Derived from achrllrogia45/LazyRef, commit e59ff1fb63a7ad3b4c6d52b0ab2de540e957b654.
// Original copyright and permission notice: ../LICENSE.
//! Scene model mirroring the 7 SQLite tables.
//!
//! Types are the mechanical SQLite storage classes — nullable where the writer can leave a
//! column NULL, otherwise raw (`i32` INTEGER, `f64` REAL, `String` TEXT, `Vec<u8>` BLOB).
//! The app layer maps these to board/sticker domain types; this layer stays format-faithful so
//! round-tripping never loses information.

use crate::blobs::{BigRational, QPainterPath, QRectF, QTransform};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub images: Vec<Image>,
    pub items: Vec<Item>,
    pub items_images: Vec<ItemImage>,
    pub groups: Vec<Group>,
    pub drawings: Vec<Drawing>,
    pub notes: Vec<Note>,
    pub metadata: Option<Metadata>,
}

/// `images` — embedded (`source_type=1`) or linked (`source_type=2`) image payloads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Image {
    pub id: i64,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub checksum: Option<String>, // md5(data) hex, app-written
    pub format: Option<String>,   // e.g. "PNG"
    pub source: Option<String>,
    pub source_type: Option<i32>,
    pub origin: Option<String>,
    pub data: Option<Vec<u8>>, // original file bytes (not re-encoded)
}

/// `items` base row. `parent = -1` means root.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: i64,
    pub name: Option<String>,
    pub transform: Option<QTransform>,
    pub sort_order: Option<BigRational>,
    pub z: Option<f64>,
    pub opacity: Option<f64>,
    pub parent: i64,
    pub locked: Option<i32>,
    pub comment: Option<String>,
}

impl Default for Item {
    fn default() -> Self {
        Item {
            id: 0,
            name: None,
            transform: None,
            sort_order: None,
            z: None,
            opacity: None,
            parent: -1,
            locked: None,
            comment: None,
        }
    }
}

/// `items_images` — image item subtype.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemImage {
    pub id: i64,
    pub image: Option<i64>,
    pub image_transform: Option<QTransform>,
    pub image_bounds: Option<QPainterPath>,
    pub flags: Option<i32>,
    pub playback_state: Option<i32>,
    pub playback_frame: Option<i32>,
    pub playback_speed: Option<f64>,
}

/// `items_groups` — group subtype.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Group {
    pub id: i64,
    pub background_color: Option<String>,
    pub lock_mode: Option<i32>,
}

/// `items_drawings` — drawing subtype.
///
/// `strokes` stays raw: the codec (`QList<GraphicsDrawItem::Stroke>`) has no captured samples;
/// decode once a drawing scene exists.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawing {
    pub id: i64,
    pub strokes: Option<Vec<u8>>,
}

/// `items_notes` — text/note subtype.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Note {
    pub id: i64,
    pub text: Option<String>,
    pub background_color: Option<String>,
    pub text_color: Option<String>,
    pub style: Option<i32>,
    pub fixed_size: Option<String>,
}

/// `metadata` — always the single row `id = 0`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    pub id: i64,
    pub thumbnail: Option<Vec<u8>>,
    pub saved: Option<i32>,
    pub last_load_checksum: Option<String>,
    pub last_load_path: Option<String>,
    pub last_save_path: Option<String>,
    pub vertical_scroll: Option<i32>,
    pub scene_rect: Option<QRectF>,
    pub view_transform: Option<QTransform>,
    pub application_version: Option<String>,
    pub horizontal_scroll: Option<i32>,
}
