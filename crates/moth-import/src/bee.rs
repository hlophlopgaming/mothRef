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
//! bytes. Images and text notes are imported; transforms MothRef cannot yet
//! store (rotation, flip, opacity, grayscale, crop sub-rects) are counted in
//! [`Summary`] and left out rather than guessed at.
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
    /// Frame rotation in radians (converted from BeeRef's degrees).
    pub rotation: f32,
    /// Mirrored horizontally (BeeRef stores this as flip = -1).
    pub flip: bool,
    /// Original encoded bytes exactly as BeeRef stored them.
    pub bytes: &'a [u8],
}

/// One text note extracted from a `.bee` board. Text owns its string because it
/// is copied out of the JSON column, which the row callback drops.
pub struct Text {
    /// BeeRef's `items.id`, used only to order.
    pub id: i64,
    /// Scene top-left x, in BeeRef scene units.
    pub x: f32,
    /// Scene top-left y, in BeeRef scene units.
    pub y: f32,
    /// BeeRef's float z-order; callers sort by it to preserve stacking.
    pub z: f32,
    /// Frame rotation in radians (converted from BeeRef's degrees).
    pub rotation: f32,
    /// Mirrored horizontally (BeeRef stores this as flip = -1).
    pub flip: bool,
    /// BeeRef's uniform scale; the caller sizes the note frame by it.
    pub scale: f32,
    /// The note's text.
    pub text: String,
}

/// One extracted item; images borrow their bytes, notes own their text.
pub enum Item<'a> {
    Image(Image<'a>),
    Text(Text),
}

/// How much of the source board the importer could represent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Images emitted to the callback.
    pub images: usize,
    /// Text notes emitted to the callback.
    pub text: usize,
    /// Unknown items, missing payloads or geometry the scene rejects.
    pub unsupported: usize,
}

impl Summary {
    /// Total items left out of the import; drives the UI hint.
    pub fn skipped(&self) -> usize {
        self.unsupported
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
    rotation: f32,
    flip: bool,
    crop: Option<[f32; 4]>,
    kind: Kind,
}

/// Reads every image and text note from a `.bee` source.
///
/// The callback runs once per item; images receive borrowed bytes, notes receive
/// owned text, so callers stream to disk without holding the whole board in
/// memory. Ordering by z is the caller's job because only it knows how inserts
/// become durable.
pub fn read<S: ReadSeek>(
    source: S,
    emit: &mut impl FnMut(Item<'_>) -> Result<(), Error>,
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
        let x = values.get(2).and_then(Value::real).unwrap_or(0.0) as f32;
        let y = values.get(3).and_then(Value::real).unwrap_or(0.0) as f32;
        let z = values.get(4).and_then(Value::real).unwrap_or(0.0) as f32;
        let scale = values.get(5).and_then(Value::real).unwrap_or(1.0) as f32;
        // BeeRef stores rotation in degrees (Qt convention, clockwise in screen
        // coordinates); the scene uses radians with the same winding.
        let rotation = values
            .get(6)
            .and_then(Value::real)
            .unwrap_or(0.0)
            .to_radians() as f32;
        let flip = values
            .get(7)
            .and_then(Value::integer)
            .is_some_and(|f| f == -1);
        let data = values
            .get(8)
            .and_then(Value::text)
            .and_then(|data| serde_json::from_str::<serde_json::Value>(data).ok());
        match kind {
            Kind::Pixmap => expected_images += 1,
            Kind::Text => {
                let text = data
                    .as_ref()
                    .and_then(|data| data.get("text"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                emit(Item::Text(Text {
                    id: rowid,
                    x,
                    y,
                    z,
                    rotation,
                    flip,
                    scale,
                    text,
                }))?;
                summary.text += 1;
            }
            Kind::Other => summary.unsupported += 1,
        }
        meta.insert(
            rowid,
            Meta {
                id: rowid,
                x,
                y,
                z,
                scale,
                rotation,
                flip,
                crop: data.as_ref().and_then(crop),
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
                emit(Item::Image(Image {
                    id: item.id,
                    x,
                    y,
                    width,
                    height,
                    z: item.z,
                    rotation: item.rotation,
                    flip: item.flip,
                    bytes,
                }))?;
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
    fn reads_geometry_text_and_counts_skipped_items() {
        let mut images = Vec::new();
        let mut notes = Vec::new();
        let summary = read(Cursor::new(FIXTURE), &mut |item| match item {
            Item::Image(image) => {
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
            }
            Item::Text(note) => {
                notes.push((note.id, note.x, note.y, note.z, note.text));
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(summary.images, 1);
        assert_eq!(summary.text, 1);
        assert_eq!(summary.skipped(), 0);
        assert_eq!(images.len(), 1);
        let (_, x, y, width, height, z, bytes) = &images[0];
        assert_eq!((*x, *y), (10.0, 20.0));
        assert_eq!((*width, *height), (2.0, 1.0));
        assert_eq!(*z, 0.001);
        assert!(bytes.starts_with(b"\x89PNG"));
        assert_eq!(notes.len(), 1);
        let (id, x, y, z, text) = &notes[0];
        assert_eq!(*id, 2);
        assert_eq!((*x, *y), (30.0, 40.0));
        assert_eq!(*z, 0.002);
        assert_eq!(text, "hi");
    }

    #[test]
    fn imports_rotation_flip_and_scale() {
        const FIXTURE: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/bee/transforms.bee"
        ));
        let mut images = Vec::new();
        let mut notes = Vec::new();
        read(Cursor::new(FIXTURE), &mut |item| match item {
            Item::Image(image) => {
                images.push((image.width, image.height, image.rotation, image.flip));
                Ok(())
            }
            Item::Text(note) => {
                notes.push((note.scale, note.rotation, note.flip, note.text));
                Ok(())
            }
        })
        .unwrap();
        // Pixmap: scale 2.0 on a 4x2 crop, rotated 90°, flipped horizontally.
        let (width, height, rotation, flip) = images[0];
        assert_eq!((width, height), (8.0, 4.0));
        assert!((rotation - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(flip);
        // Text: scale 1.5, rotated 45°, flipped horizontally.
        let (scale, rotation, flip, text) = &notes[0];
        assert_eq!(*scale, 1.5);
        assert!((rotation - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert!(*flip);
        assert_eq!(text, "note");
    }

    #[test]
    fn cyclic_table_returns_an_error() {
        let bytes = include_bytes!("../../../corpus/bee/invalid/cyclic.bin");
        assert!(matches!(
            read(Cursor::new(bytes), &mut |_| Ok(())),
            Err(Error::Sqlite(SqliteError::Malformed))
        ));
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
