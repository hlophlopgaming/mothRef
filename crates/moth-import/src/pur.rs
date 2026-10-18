// SPDX-License-Identifier: GPL-3.0-or-later
//! PureRef import through the locally maintained, bounded purcore reader.
use moth_format::ReadSeek;
use moth_scene::{Rect, Transform};
pub use purcore::Error;
use purcore::{
    Entry,
    blobs::{QPainterPath, QTransform},
    legacy,
};

/// Import counts include unsupported items so losses are visible to the user.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Embedded image placements emitted.
    pub images: usize,
    /// Plain text notes emitted.
    pub text: usize,
    /// Items omitted because their content or geometry cannot be represented.
    pub unsupported: usize,
}
impl Summary {
    /// Drives the application's partial-import warning.
    pub fn skipped(&self) -> usize {
        self.unsupported
    }
}
/// One image or plain note; original image bytes are borrowed for one callback.
pub struct Item<'a> {
    /// Source identity, used only to break stacking ties.
    pub id: i64,
    /// Source stacking order.
    pub z: f64,
    /// Frame in scene coordinates.
    pub rect: Rect,
    /// Content and frame orientation.
    pub transform: Transform,
    /// Encoded original; absent for a note.
    pub bytes: Option<&'a [u8]>,
    /// Plain note text; absent for an image.
    pub text: Option<&'a str>,
}
/// Streams supported contents without opening links or modifying the source board.
pub fn read<S: ReadSeek>(
    source: S,
    emit: &mut impl FnMut(Item<'_>) -> Result<(), Error>,
) -> Result<Summary, Error> {
    let mut summary = Summary::default();
    purcore::read(source, &mut |entry| convert(entry, emit, &mut summary))?;
    Ok(summary)
}
fn convert(
    entry: Entry<'_>,
    emit: &mut impl FnMut(Item<'_>) -> Result<(), Error>,
    summary: &mut Summary,
) -> Result<(), Error> {
    match entry {
        Entry::Image(base, placement, image) => {
            let mapped = (|| {
                if base.parent != -1
                    || base.opacity.unwrap_or(1.) != 1.
                    || placement.flags.unwrap_or(0) & !1 != 0
                {
                    return None;
                }
                let (width, height) = (image.width? as f64, image.height? as f64);
                let bytes = image.data.as_deref()?;
                if bytes.is_empty() || width <= 0. || height <= 0. {
                    return None;
                }
                let bounds = bounds(placement.image_bounds.as_ref()?)?;
                let content = placement.image_transform.unwrap_or(QTransform::IDENTITY);
                if !affine(content) || content.m12.abs() > 1e-6 || content.m21.abs() > 1e-6 {
                    return None;
                }
                let content_scale = content.m11.abs();
                if (content_scale - content.m22.abs()).abs() > 1e-5 * content_scale {
                    return None;
                }
                let (rect, mut transform) =
                    geometry(base.transform.unwrap_or(QTransform::IDENTITY), bounds)?;
                let reflected = transform.flip[1];
                transform.flip[0] = content.m11 < 0.;
                transform.flip[1] ^= content.m22 < 0.;
                transform.content_scale =
                    (content_scale / (bounds[2] / width).max(bounds[3] / height)) as f32;
                transform.content_offset = [
                    ((content.m31 + content.m11 * width / 2. - bounds[0] - bounds[2] / 2.)
                        / bounds[2]) as f32,
                    ((content.m32 + content.m22 * height / 2. - bounds[1] - bounds[3] / 2.)
                        / bounds[3]) as f32
                        * if reflected { -1. } else { 1. },
                ];
                if !transform.valid() {
                    return None;
                }
                Some(Item {
                    id: base.id,
                    z: base.z.unwrap_or(0.),
                    rect,
                    transform,
                    bytes: Some(bytes),
                    text: None,
                })
            })();
            if let Some(item) = mapped {
                emit(item)?;
                summary.images += 1;
            } else {
                summary.unsupported += 1;
            }
        }
        Entry::Note(base, note) => {
            let text = note.text.as_deref().unwrap_or("");
            if text.len() > moth_scene::MAX_NOTE_BYTES
                || base.parent != -1
                || base.opacity.unwrap_or(1.) != 1.
                || note.fixed_size.is_some()
                || text.contains('<')
            {
                summary.unsupported += 1;
            } else {
                let [w, h] = moth_scene::note_size(text);
                if let Some((rect, transform)) = geometry(
                    base.transform.unwrap_or(QTransform::IDENTITY),
                    [0., 0., w as f64, h as f64],
                ) {
                    emit(Item {
                        id: base.id,
                        z: base.z.unwrap_or(0.),
                        rect,
                        transform,
                        bytes: None,
                        text: Some(text),
                    })?;
                    summary.text += 1;
                } else {
                    summary.unsupported += 1;
                }
            }
        }
        Entry::Unsupported => summary.unsupported += 1,
        Entry::Legacy(scene) => legacy_items(scene, emit, summary)?,
    }
    Ok(())
}
fn affine(t: QTransform) -> bool {
    t.values().iter().all(|v| v.is_finite()) && t.m13 == 0. && t.m23 == 0. && t.m33 == 1.
}
fn geometry(t: QTransform, bounds: [f64; 4]) -> Option<(Rect, Transform)> {
    if !affine(t) {
        return None;
    }
    let [x, y, w, h] = bounds;
    let sx = t.m11.hypot(t.m12);
    let sy = t.m21.hypot(t.m22);
    if sx <= 0.
        || sy <= 0.
        || w <= 0.
        || h <= 0.
        || (sx - sy).abs() > 1e-5 * sx.max(sy)
        || (t.m11 * t.m21 + t.m12 * t.m22).abs() > 1e-6 * sx * sy
    {
        return None;
    }
    let cx = x + w / 2.;
    let cy = y + h / 2.;
    let rect = Rect {
        x: (t.m11 * cx + t.m21 * cy + t.m31 - w * sx / 2.) as f32,
        y: (t.m12 * cx + t.m22 * cy + t.m32 - h * sy / 2.) as f32,
        width: (w * sx) as f32,
        height: (h * sy) as f32,
    };
    let transform = Transform {
        rotation: t.m12.atan2(t.m11) as f32,
        flip: [false, t.m11 * t.m22 - t.m12 * t.m21 < 0.],
        ..Default::default()
    };
    if !rect.valid() || !transform.valid() {
        return None;
    }
    Some((rect, transform))
}
fn bounds(path: &QPainterPath) -> Option<[f64; 4]> {
    let p = &path.elements;
    if p.len() != 5 || p[0].r#type != 0 || p[1..].iter().any(|p| p.r#type != 1) {
        return None;
    }
    let (x, y) = (p[0].x, p[0].y);
    let (w, h) = (p[1].x - x, p[2].y - y);
    if p[1].y != y
        || p[2].x != p[1].x
        || p[3].x != x
        || p[3].y != p[2].y
        || p[4].x != x
        || p[4].y != y
        || w <= 0.
        || h <= 0.
    {
        return None;
    }
    Some([x, y, w, h])
}
fn legacy_matrix(m: [f64; 4], x: f64, y: f64) -> QTransform {
    QTransform {
        m11: m[0],
        m12: m[1],
        m21: m[2],
        m22: m[3],
        m31: x,
        m32: y,
        ..QTransform::IDENTITY
    }
}
fn legacy_items(
    scene: &legacy::LegacyScene,
    emit: &mut impl FnMut(Item<'_>) -> Result<(), Error>,
    summary: &mut Summary,
) -> Result<(), Error> {
    for image in &scene.images {
        for item in &image.transforms {
            // shortcut: only full-frame legacy crops; extend after capturing crop fixtures.
            let full = image.png.starts_with(b"\x89PNG\r\n\x1a\n") && image.png.len() >= 24;
            let mapped = (|| {
                if !full || !item.text_children.is_empty() {
                    return None;
                }
                let width = u32::from_be_bytes(image.png[16..20].try_into().ok()?) as f64;
                let height = u32::from_be_bytes(image.png[20..24].try_into().ok()?) as f64;
                if item.points_x.len() != 5 || item.points_y.len() != 5 {
                    return None;
                }
                let path = QPainterPath {
                    elements: item
                        .points_x
                        .iter()
                        .zip(&item.points_y)
                        .enumerate()
                        .map(|(i, (&x, &y))| purcore::blobs::PathElement {
                            r#type: if i == 0 { 0 } else { 1 },
                            x,
                            y,
                        })
                        .collect(),
                    c_start: 0,
                    fill_rule: 0,
                };
                let b = bounds(&path)?;
                if (b[2] - width).abs() > 1e-5 || (b[3] - height).abs() > 1e-5 {
                    return None;
                }
                if item.x_crop != -width / 2.
                    || item.y_crop != -height / 2.
                    || item.scale_crop != 1.
                    || item.matrix_before_crop != [1., 0., 0., 1.]
                {
                    return None;
                }
                let (rect, transform) = geometry(legacy_matrix(item.matrix, item.x, item.y), b)?;
                Some(Item {
                    id: item.id as i64,
                    z: item.z_layer,
                    rect,
                    transform,
                    bytes: Some(&image.png),
                    text: None,
                })
            })();
            if let Some(item) = mapped {
                emit(item)?;
                summary.images += 1;
            } else {
                summary.unsupported += 1;
            }
        }
    }
    for note in &scene.texts {
        let [w, h] = moth_scene::note_size(&note.text);
        if note.text.len() > moth_scene::MAX_NOTE_BYTES
            || note.opacity != 65535
            || !note.text_children.is_empty()
            || note.text.contains('<')
        {
            summary.unsupported += 1;
            continue;
        }
        if let Some((rect, transform)) = geometry(
            legacy_matrix(note.matrix, note.x, note.y),
            [0., 0., w as f64, h as f64],
        ) {
            emit(Item {
                id: note.id as i64,
                z: note.z_layer,
                rect,
                transform,
                bytes: None,
                text: Some(&note.text),
            })?;
            summary.text += 1;
        } else {
            summary.unsupported += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    const V1: &[u8] = include_bytes!("../../../corpus/pur/v1.10-redbar.pur");
    const V2: &[u8] = include_bytes!("../../../corpus/pur/v2.1-redbar.pur");
    #[test]
    fn captured_versions_preserve_originals_and_placement() {
        for bytes in [V1, V2] {
            let mut images = Vec::new();
            let summary = read(Cursor::new(bytes), &mut |item| {
                images.push((item.rect, item.transform, item.bytes.unwrap().to_vec()));
                Ok(())
            })
            .unwrap();
            assert_eq!(
                summary,
                Summary {
                    images: 1,
                    text: 0,
                    unsupported: 0
                }
            );
            let (rect, transform, original) = &images[0];
            assert_eq!(
                *rect,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: 1000.,
                    height: 750.
                }
            );
            assert_eq!(*transform, Transform::default());
            assert_eq!(original.len(), 157);
            assert_eq!(
                purcore::checksum::md5_hex(original),
                "3f97a27e8cbfd743b3d6e3a675290a0b"
            );
        }
    }
    #[test]
    fn every_truncation_and_bad_checksum_is_rejected() {
        for bytes in [V1, V2] {
            for cut in 0..bytes.len() {
                assert!(
                    read(Cursor::new(&bytes[..cut]), &mut |_| Ok(())).is_err(),
                    "cut {cut}"
                );
            }
            let mut bad = bytes.to_vec();
            let n = bad.len();
            bad[n - 1] ^= 1;
            assert!(read(Cursor::new(bad), &mut |_| Ok(())).is_err());
        }
    }
    #[test]
    fn sink_errors_propagate() {
        for bytes in [V1, V2] {
            assert!(matches!(
                read(Cursor::new(bytes), &mut |_| Err(std::io::Error::other(
                    "sink failed"
                )
                .into())),
                Err(Error::Io(_))
            ));
        }
    }
    #[test]
    fn rotation_reflection_and_shear_are_distinguished() {
        let (rect, t) = geometry(
            QTransform {
                m11: 0.,
                m12: 2.,
                m21: 2.,
                m22: 0.,
                m31: 10.,
                m32: 20.,
                ..QTransform::IDENTITY
            },
            [-2., -1., 4., 2.],
        )
        .unwrap();
        assert_eq!(
            rect,
            Rect {
                x: 6.,
                y: 18.,
                width: 8.,
                height: 4.
            }
        );
        assert_eq!(t.rotation, std::f32::consts::FRAC_PI_2);
        assert_eq!(t.flip, [false, true]);
        assert!(
            geometry(
                QTransform {
                    m21: 0.5,
                    ..QTransform::IDENTITY
                },
                [0., 0., 4., 2.]
            )
            .is_none()
        );
    }
    proptest::proptest! {
        #[test]
        fn converted_geometry_undoes_and_roundtrips(x in -10000f64..10000., y in -10000f64..10000., scale in 0.1f64..100., rotation in -3f64..3.) {
            use moth_scene::{Command, Item as SceneItem, Scene};
            use moth_format::board;
            let (sin,cos)=rotation.sin_cos();
            let (rect,transform)=geometry(QTransform{m11:cos*scale,m12:sin*scale,m21:-sin*scale,m22:cos*scale,m31:x,m32:y,..QTransform::IDENTITY},[-32.,-24.,64.,48.]).unwrap();
            let mut item=SceneItem::text("note",rect,None);item.transform=transform;
            let mut scene=Scene::default();let empty=board::snapshot(&scene).unwrap();
            let undo=scene.apply(Command::Insert(item.clone())).unwrap();
            let mut bytes=Vec::new();board::initialize(&mut bytes).unwrap();
            let offset=bytes.len() as u64;
            board::append(&mut bytes,offset,3,&board::snapshot(&scene).unwrap()).unwrap();
            let loaded=board::read(&mut Cursor::new(bytes)).unwrap();
            proptest::prop_assert_eq!(loaded.scene.get(item.id),Some(&item));
            scene.apply(undo).unwrap();proptest::prop_assert_eq!(board::snapshot(&scene).unwrap(),empty);
        }
    }
    #[test]
    fn plain_notes_import_and_unsupported_content_is_reported() {
        let base = purcore::model::Item::default();
        let mut note = purcore::model::Note {
            text: Some("hello".into()),
            ..Default::default()
        };
        let mut summary = Summary::default();
        let mut texts = Vec::new();
        convert(
            Entry::Note(&base, &note),
            &mut |item| {
                texts.push(item.text.unwrap().to_owned());
                Ok(())
            },
            &mut summary,
        )
        .unwrap();
        assert_eq!(texts, ["hello"]);
        assert_eq!(summary.text, 1);
        note.text = Some("<p>rich text</p>".into());
        convert(
            Entry::Note(&base, &note),
            &mut |_| panic!("rich notes must be reported as skipped"),
            &mut summary,
        )
        .unwrap();
        convert(
            Entry::Unsupported,
            &mut |_| panic!("unsupported item must be skipped"),
            &mut summary,
        )
        .unwrap();
        assert_eq!(summary.unsupported, 2);
    }
    #[test]
    fn embedded_images_keep_content_flip_and_linked_images_are_not_read() {
        use purcore::blobs::PathElement;
        let base = purcore::model::Item::default();
        let mut placement = purcore::model::ItemImage {
            image_transform: Some(QTransform {
                m11: -1.,
                m31: 32.,
                m32: -24.,
                ..QTransform::IDENTITY
            }),
            image_bounds: Some(QPainterPath {
                elements: [
                    (-32., -24.),
                    (32., -24.),
                    (32., 24.),
                    (-32., 24.),
                    (-32., -24.),
                ]
                .into_iter()
                .enumerate()
                .map(|(i, (x, y))| PathElement {
                    r#type: if i == 0 { 0 } else { 1 },
                    x,
                    y,
                })
                .collect(),
                c_start: 0,
                fill_rule: 0,
            }),
            ..Default::default()
        };
        let mut image = purcore::model::Image {
            width: Some(64),
            height: Some(48),
            data: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        let mut summary = Summary::default();
        convert(
            Entry::Image(&base, &placement, &image),
            &mut |item| {
                assert_eq!(item.transform.flip, [true, false]);
                assert_eq!(item.transform.content_offset, [0., 0.]);
                Ok(())
            },
            &mut summary,
        )
        .unwrap();
        let path = placement.image_bounds.as_mut().unwrap();
        path.elements[1].x = 0.;
        path.elements[2].x = 0.;
        convert(
            Entry::Image(&base, &placement, &image),
            &mut |item| {
                assert_eq!(item.rect.width, 32.);
                assert_eq!(item.rect.height, 48.);
                assert_eq!(item.transform.content_scale, 1.);
                assert_eq!(item.transform.content_offset, [0.5, 0.]);
                Ok(())
            },
            &mut summary,
        )
        .unwrap();
        assert_eq!(summary.images, 2);
        image.data = None;
        image.source = Some("/must/not/be/opened.png".into());
        convert(
            Entry::Image(&base, &placement, &image),
            &mut |_| panic!("linked image was read"),
            &mut summary,
        )
        .unwrap();
        image.data = Some(vec![1]);
        placement.image_transform.as_mut().unwrap().m21 = 0.5;
        convert(
            Entry::Image(&base, &placement, &image),
            &mut |_| panic!("sheared image was imported"),
            &mut summary,
        )
        .unwrap();
        assert_eq!(summary.images, 2);
        assert_eq!(summary.unsupported, 2);
    }
    #[test]
    fn nonuniform_source_scale_is_skipped_instead_of_distorted() {
        assert!(
            geometry(
                QTransform {
                    m11: 2.,
                    m22: 3.,
                    ..QTransform::IDENTITY
                },
                [0., 0., 64., 48.]
            )
            .is_none()
        );
    }
}
