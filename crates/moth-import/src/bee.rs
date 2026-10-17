// SPDX-License-Identifier: GPL-3.0-or-later
//! Reader for BeeRef's native `.bee` board format.
//!
//! Format learned from BeeRef's `beeref/fileio` (GPL-3.0-or-later, rbreu/beeref):
//! the `items` schema is `beeref/fileio/schema.py` and the read query is
//! `beeref/fileio/sql.py`. No BeeRef source was copied; this is an independent
//! implementation of the same on-disk layout. See `THIRD_PARTY.md`.
//!
//! A `.bee` file is a SQLite database (see [`crate::sqlite`]) with an `items`
//! table for metadata and an `sqlar` table holding the embedded original image
//! bytes. Only images can be represented in the current scene model, so text
//! notes and transforms MothRef cannot yet store (rotation, flip, opacity,
//! grayscale, crop sub-rects) are counted in [`Summary`] and left out rather
//! than guessed at.
use crate::sqlite::{Database, Error as SqliteError, Value};
use moth_format::ReadSeek;
use std::collections::HashMap;

/// Import failures are recoverable and named, so the UI can explain them.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The underlying SQLite reader rejected the file.
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    /// The caller (usually the storage worker) failed while consuming an image.
    #[error("bee import sink: {0}")]
    Io(#[from] std::io::Error),
    /// A table the `.bee` format requires is missing.
    #[error("bee board is missing the '{0}' table")]
    MissingTable(&'static str),
}

/// One image extracted from a `.bee` board, borrows its bytes for one callback.
pub struct Image<'a> {
    /// BeeRef's `items.id`, used only to order and report duplicates.
    pub id: i64,
    /// Scene top-left x, in BeeRef scene units.
    pub x: f32,
    /// Scene top-left y, in BeeRef scene units.
    pub y: f32,
    /// On-canvas width (`crop.width * scale`).
    pub width: f32,
    /// On-canvas height (`crop.height * scale`).
    pub height: f32,
    /// BeeRef's float z-order; callers sort by it to preserve stacking.
    pub z: f32,
    /// Original encoded bytes exactly as BeeRef stored them.
    pub bytes: &'a [u8],
}

/// How much of the source board the importer could represent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Images emitted to the callback.
    pub images: usize,
    /// Text notes, which the scene cannot store yet.
    pub text: usize,
    /// Unknown items, missing payloads or geometry the scene rejects.
    pub unsupported: usize,
}

impl Summary {
    /// Total items left out of the import; drives the UI hint.
    pub fn skipped(&self) -> usize {
        self.text + self.unsupported
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pixmap,
    Text,
    Other,
}

struct Meta {
    id: i64,
    x: f32,
    y: f32,
    z: f32,
    scale: f32,
    crop: Option<[f32; 4]>,
    kind: Kind,
}

/// Reads every embedded image from a `.bee` source, in `sqlar` key order.
///
/// The callback runs once per image and receives borrowed bytes, so callers
/// stream to disk without holding the whole board in memory. Ordering by z is
/// the caller's job because only it knows how inserts become durable.
pub fn read<S: ReadSeek>(
    source: S,
    emit: &mut impl FnMut(Image<'_>) -> Result<(), Error>,
) -> Result<Summary, Error> {
    let mut database = Database::open(source)?;
    let items_root = database
        .root_page("items")?
        .ok_or(Error::MissingTable("items"))?;
    let sqlar_root = database
        .root_page("sqlar")?
        .ok_or(Error::MissingTable("sqlar"))?;

    let mut summary = Summary::default();
    let mut expected_images = 0usize;
    let mut meta: HashMap<i64, Meta> = HashMap::new();
    database.walk_table(items_root, &mut |rowid, values| -> Result<(), Error> {
        let kind = match values.get(1).and_then(Value::text) {
            Some("pixmap") => Kind::Pixmap,
            Some("text") => Kind::Text,
            _ => Kind::Other,
        };
        match kind {
            Kind::Pixmap => expected_images += 1,
            Kind::Text => summary.text += 1,
            Kind::Other => summary.unsupported += 1,
        }
        let crop = values
            .get(8)
            .and_then(Value::text)
            .and_then(|data| serde_json::from_str::<serde_json::Value>(data).ok())
            .and_then(|data| crop(&data));
        meta.insert(
            rowid,
            Meta {
                id: rowid,
                x: values.get(2).and_then(Value::real).unwrap_or(0.0) as f32,
                y: values.get(3).and_then(Value::real).unwrap_or(0.0) as f32,
                z: values.get(4).and_then(Value::real).unwrap_or(0.0) as f32,
                scale: values.get(5).and_then(Value::real).unwrap_or(1.0) as f32,
                crop,
                kind,
            },
        );
        Ok(())
    })?;

    database.walk_table(sqlar_root, &mut |_rowid, values| -> Result<(), Error> {
        let Some(item_id) = values.get(1).and_then(Value::integer) else {
            return Ok(());
        };
        let Some(Value::Blob(bytes)) = values.get(5) else {
            return Ok(());
        };
        let Some(item) = meta.get(&item_id) else {
            return Ok(());
        };
        if item.kind != Kind::Pixmap {
            return Ok(());
        }
        expected_images = expected_images.saturating_sub(1);
        match geometry(item) {
            Some((x, y, width, height)) => {
                emit(Image {
                    id: item.id,
                    x,
                    y,
                    width,
                    height,
                    z: item.z,
                    bytes,
                })?;
                summary.images += 1;
            }
            None => summary.unsupported += 1,
        }
        Ok(())
    })?;

    // Any pixmap without a readable payload was dropped by the loop above.
    summary.unsupported += expected_images;
    Ok(summary)
}

/// Reads `crop: [x, y, width, height]` from an item's JSON `data` column.
fn crop(data: &serde_json::Value) -> Option<[f32; 4]> {
    let array = data.get("crop")?.as_array()?;
    if array.len() != 4 {
        return None;
    }
    let mut values = [0f32; 4];
    for (slot, value) in values.iter_mut().zip(array) {
        *slot = value.as_f64()? as f32;
    }
    Some(values)
}

/// Converts a BeeRef item's geometry into a scene rectangle, rejecting values
/// the scene would refuse anyway.
fn geometry(item: &Meta) -> Option<(f32, f32, f32, f32)> {
    let crop = item.crop?;
    let width = crop[2] * item.scale;
    let height = crop[3] * item.scale;
    let finite = [item.x, item.y, width, height]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1e9);
    (finite && width > 0.0 && height > 0.0).then_some((item.x, item.y, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const FIXTURE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../corpus/bee/tiny.bee"
    ));

    #[test]
    fn reads_geometry_and_counts_skipped_items() {
        let mut images = Vec::new();
        let summary = read(Cursor::new(FIXTURE), &mut |image| {
            images.push((
                image.id,
                image.x,
                image.y,
                image.width,
                image.height,
                image.z,
                image.bytes.to_vec(),
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(summary.images, 1);
        assert_eq!(summary.text, 1);
        assert_eq!(summary.skipped(), 1);
        assert_eq!(images.len(), 1);
        let (_, x, y, width, height, z, bytes) = &images[0];
        assert_eq!((*x, *y), (10.0, 20.0));
        assert_eq!((*width, *height), (2.0, 1.0));
        assert_eq!(*z, 0.001);
        assert!(bytes.starts_with(b"\x89PNG"));
    }

    #[test]
    fn corrupt_input_returns_an_error() {
        let result = read(Cursor::new(vec![0u8; 4096]), &mut |_| Ok(()));
        assert!(result.is_err());
    }

    #[test]
    fn truncating_the_fixture_never_panics() {
        for cut in 0..FIXTURE.len() {
            let _ = read(Cursor::new(FIXTURE[..cut].to_vec()), &mut |_| Ok(()));
        }
    }
}
