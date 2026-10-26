// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Pixel-free board state. All mutations go through reversible commands.
use fractional_index::FractionalIndex;
use rstar::{AABB, RTree, RTreeObject};
use std::collections::{BTreeSet, HashMap};
pub use uuid::Uuid;

/// Identifies original bytes independently of a storage backend or GPU format.
pub type BlobId = blake3::Hash;
/// Upper bound on a note's UTF-8 length, shared by the scene and the format
/// so a single over-long note can never blow past a command budget.
pub const MAX_NOTE_BYTES: usize = 32 * 1024;
/// What an item carries on the canvas. Images reference original bytes by hash;
/// notes own their text (metadata only, still no pixels in the scene).
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// References original image bytes; the renderer supplies the pixels.
    Image(BlobId),
    /// Plain-text note. Font size is a fixed presentation constant.
    Text(String),
}
/// Note font size in world units; the renderer and the UI share it so measured
/// note frames match what is drawn.
pub const NOTE_FONT_SIZE: f32 = 24.0;
/// Estimates a note's frame from its text, so importers can size notes without
/// a font. shortcut: assumes an average glyph advance of `0.6em`; the UI
/// re-measures with the real font when a note is created or edited.
pub fn note_size(text: &str) -> [f32; 2] {
    let widest = text
        .lines()
        .map(str::chars)
        .map(|c| c.count())
        .max()
        .unwrap_or(0)
        .max(1) as f32;
    let rows = if text.is_empty() {
        1
    } else {
        text.lines().count()
    } as f32;
    [widest * NOTE_FONT_SIZE * 0.6, rows * NOTE_FONT_SIZE * 1.3]
}
/// World-space rectangle; validated before entering the spatial index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// World coordinates keep culling independent of the camera.
    pub x: f32,
    /// World coordinates keep culling independent of the camera.
    pub y: f32,
    /// Positive extents preserve valid geometry and aspect ratio.
    pub width: f32,
    /// Positive extents preserve valid geometry and aspect ratio.
    pub height: f32,
}
impl Rect {
    /// Rejects invalid coordinates before they can poison culling and transforms.
    pub fn valid(self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1e9)
            && self.width > 0.0
            && self.height > 0.0
    }
    fn envelope(self) -> AABB<[f32; 2]> {
        self.rotated_envelope(0.0)
    }
    /// Conservative axis-aligned footprint of the frame turned by `rotation`.
    /// Culling stays correct for rotated items without exposing rotation to the index.
    pub fn rotated_envelope(self, rotation: f32) -> AABB<[f32; 2]> {
        let (sin, cos) = rotation.sin_cos();
        let half = [self.width / 2.0, self.height / 2.0];
        let extent = [
            half[0] * cos.abs() + half[1] * sin.abs(),
            half[0] * sin.abs() + half[1] * cos.abs(),
        ];
        let center = [self.x + half[0], self.y + half[1]];
        AABB::from_corners(
            [center[0] - extent[0], center[1] - extent[1]],
            [center[0] + extent[0], center[1] + extent[1]],
        )
    }
    /// The frame's four corners, turned by `rotation` around the frame centre.
    ///
    /// Order is top-left, top-right, bottom-left, bottom-right as if unrotated, which is
    /// what the renderer's content quad and the canvas handles both expect.
    pub fn corners(self, rotation: f32) -> [[f32; 2]; 4] {
        let center = [self.x + self.width / 2.0, self.y + self.height / 2.0];
        let (sin, cos) = rotation.sin_cos();
        [[-1.0, -1.0], [1.0, -1.0], [-1.0, 1.0], [1.0, 1.0]].map(|[across, down]| {
            let offset = [across * self.width / 2.0, down * self.height / 2.0];
            [
                center[0] + offset[0] * cos - offset[1] * sin,
                center[1] + offset[0] * sin + offset[1] * cos,
            ]
        })
    }
    /// Exact hit test for a rotated frame, unlike the conservative [`Self::rotated_envelope`].
    pub fn contains_rotated(self, rotation: f32, point: [f32; 2]) -> bool {
        let (sin, cos) = rotation.sin_cos();
        let dx = point[0] - (self.x + self.width / 2.0);
        let dy = point[1] - (self.y + self.height / 2.0);
        let local = [dx * cos + dy * sin, -dx * sin + dy * cos];
        local[0].abs() <= self.width / 2.0 && local[1].abs() <= self.height / 2.0
    }
}
/// Per-image sampling may inherit the current canvas preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Sampling {
    /// Follow the user's canvas preference.
    #[default]
    Default = 0,
    /// Smooth interpolation for photographs and downscaled references.
    Smooth = 1,
    /// Keep resident texels sharp when magnifying pixel art.
    Nearest = 2,
    /// Switch to sharp texels at or above two screen pixels per source pixel.
    Automatic = 3,
}
impl Sampling {
    /// Persisted values are validated at the format and settings boundaries.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Default),
            1 => Some(Self::Smooth),
            2 => Some(Self::Nearest),
            3 => Some(Self::Automatic),
            _ => None,
        }
    }
    /// Automatic uses original pixel scale, independent of the currently resident mip.
    pub fn nearest(self, default: Self, pixel_scale: f32) -> bool {
        match if self == Self::Default { default } else { self } {
            Self::Nearest => true,
            Self::Automatic => pixel_scale >= 2.,
            _ => false,
        }
    }
}
/// How item content sits inside its frame, plus the frame's own rotation.
/// Content stays clipped by the frame, so scaling never grows an item's footprint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Rotates frame and content together around the frame centre, in radians.
    pub rotation: f32,
    /// Multiplies rendered alpha without changing original pixels.
    pub opacity: f32,
    /// Displays luminance while preserving the original colors.
    pub grayscale: bool,
    /// Image interpolation override; Default follows the canvas setting.
    pub sampling: Sampling,
    /// Fixed note font size in world units; zero migrates legacy height-derived sizing.
    pub note_font_size: f32,
    /// A fixed-width note wraps instead of expanding to fit its text.
    pub note_wrap: bool,
    /// Mirrors content horizontally, then vertically, before `content_rotation`.
    pub flip: [bool; 2],
    /// Pans content inside the frame, in frame widths and heights.
    pub content_offset: [f32; 2],
    /// Zooms content; 1.0 fills the frame without distortion, clipped to it.
    pub content_scale: f32,
    /// Rotates content inside the frame around the frame centre, in radians.
    pub content_rotation: f32,
}
impl Default for Transform {
    fn default() -> Self {
        Self {
            rotation: 0.0,
            opacity: 1.0,
            grayscale: false,
            sampling: Sampling::Default,
            note_font_size: 0.,
            note_wrap: false,
            flip: [false; 2],
            content_offset: [0.0; 2],
            content_scale: 1.0,
            content_rotation: 0.0,
        }
    }
}
impl Transform {
    /// Geometry resets preserve appearance, which has separate editing controls.
    pub fn reset_geometry(self) -> Self {
        Self {
            opacity: self.opacity,
            grayscale: self.grayscale,
            sampling: self.sampling,
            note_font_size: self.note_font_size,
            note_wrap: self.note_wrap,
            ..Self::default()
        }
    }

    /// Rejects values that would poison hit testing, culling or content mapping.
    pub fn valid(self) -> bool {
        [
            self.rotation,
            self.content_offset[0],
            self.content_offset[1],
            self.content_scale,
            self.content_rotation,
        ]
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1e9)
            && self.content_scale > 0.0
            && self.note_font_size.is_finite()
            && (0.0..=1e9).contains(&self.note_font_size)
            && (!self.note_wrap || self.note_font_size > 0.)
            && self.opacity.is_finite()
            && (0.0..=1.0).contains(&self.opacity)
    }
}
/// Persistent identity and ordering never depend on in-process indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Associates updates with the same object across operations.
    pub id: Uuid,
    /// Source filename only, retained for sorting without storing a native path.
    pub name: String,
    /// What the item shows: original image bytes or a note's text.
    pub content: Content,
    /// Keeps indexed bounds available without decoding an image.
    pub rect: Rect,
    /// Frame and content transform; identity until the user manipulates the item.
    pub transform: Transform,
    /// Allows insertion between images without renumbering the board.
    pub z: FractionalIndex,
}
impl Item {
    /// Appends an image above the supplied fractional index.
    pub fn image(blob: BlobId, rect: Rect, above: Option<&FractionalIndex>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: String::new(),
            content: Content::Image(blob),
            rect,
            transform: Transform::default(),
            z: above.map(FractionalIndex::new_after).unwrap_or_default(),
        }
    }
    /// Appends a note above the supplied fractional index.
    pub fn text(text: impl Into<String>, rect: Rect, above: Option<&FractionalIndex>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: String::new(),
            content: Content::Text(text.into()),
            rect,
            transform: Transform::default(),
            z: above.map(FractionalIndex::new_after).unwrap_or_default(),
        }
    }
    /// Maps a world point back to original image coordinates, rejecting clipped or empty space.
    pub fn source_uv(&self, point: [f32; 2], dimensions: [u32; 2]) -> Option<[f32; 2]> {
        if dimensions.contains(&0) || !self.rect.contains_rotated(self.transform.rotation, point) {
            return None;
        }
        let r = self.rect;
        let t = self.transform;
        let dx = point[0] - r.x - r.width / 2.;
        let dy = point[1] - r.y - r.height / 2.;
        let (sin, cos) = t.rotation.sin_cos();
        let local = [dx * cos + dy * sin, -dx * sin + dy * cos];
        let (sin, cos) = t.content_rotation.sin_cos();
        let local = [
            local[0] * cos + local[1] * sin - t.content_offset[0] * r.width,
            -local[0] * sin + local[1] * cos - t.content_offset[1] * r.height,
        ];
        let fit =
            (r.width / dimensions[0] as f32).max(r.height / dimensions[1] as f32) * t.content_scale;
        let uv = [0, 1].map(|axis| {
            let value = local[axis] / (dimensions[axis] as f32 * fit) + 0.5;
            if t.flip[axis] { 1. - value } else { value }
        });
        uv.iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            .then_some(uv)
    }
    /// The referenced image blob, or `None` for notes.
    pub fn blob(&self) -> Option<BlobId> {
        match self.content {
            Content::Image(blob) => Some(blob),
            Content::Text(_) => None,
        }
    }
    /// Legacy notes derive text scale from their unwrapped height until first edited.
    pub fn note_font_size(&self) -> f32 {
        if self.transform.note_font_size > 0. {
            self.transform.note_font_size
        } else {
            self.rect.height / (self.note().unwrap_or_default().lines().count().max(1) as f32 * 1.3)
        }
    }
    /// Proportional transforms scale note text with the frame; manual widths do not.
    pub fn scaled_note_transform(&self, factor: f32) -> Transform {
        if self.note().is_some() {
            Transform {
                note_font_size: self.note_font_size() * factor,
                ..self.transform
            }
        } else {
            self.transform
        }
    }
    /// The note text, or `None` for images.
    pub fn note(&self) -> Option<&str> {
        match &self.content {
            Content::Text(text) => Some(text),
            Content::Image(_) => None,
        }
    }
}
/// Rebuilds an index entry from the fields `apply` writes, so removal always matches insertion.
fn entry(item: &Item) -> Entry {
    Entry {
        id: item.id,
        rect: item.rect,
        rotation: item.transform.rotation,
    }
}
#[derive(Clone, PartialEq)]
struct Entry {
    id: Uuid,
    rect: Rect,
    rotation: f32,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.rect.rotated_envelope(self.rotation)
    }
}
/// Commands return their inverse, making undo and later journaling share one path.
#[derive(Clone, Debug)]
pub enum Command {
    /// Introduces metadata through the reversible mutation path.
    Insert(Item),
    /// Returns the removed item as its inverse.
    Remove(Uuid),
    /// Updates geometry and spatial indexing atomically.
    SetRect {
        /// Stable identity prevents changes being applied to a reused slot.
        id: Uuid,
        /// Validated bounds update both metadata and the R-tree.
        rect: Rect,
    },
    /// Updates how content sits inside the frame; the frame itself is unchanged.
    SetTransform {
        /// Stable identity prevents changes being applied to a reused slot.
        id: Uuid,
        /// Validated transform; invalid values leave the board unchanged.
        transform: Transform,
    },
    /// Replaces a note's text; the inverse carries the previous text.
    SetText {
        /// Stable identity prevents changes being applied to a reused slot.
        id: Uuid,
        /// New note text, bounded by [`MAX_NOTE_BYTES`].
        text: String,
    },
}
/// Failed commands leave the board unchanged.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SceneError {
    #[error("invalid item geometry or identity")]
    /// Rejects invalid metadata before it reaches the R-tree.
    Invalid,
    #[error("item already exists")]
    /// Prevents accidental replacement of an existing item.
    Duplicate,
    #[error("item does not exist")]
    /// Rejects commands targeting an item that no longer exists.
    Missing,
}
/// Owns metadata only; the R-tree has exactly one entry per live item.
#[derive(Default)]
pub struct Scene {
    items: HashMap<Uuid, Item>,
    tree: RTree<Entry>,
    order: BTreeSet<(FractionalIndex, Uuid)>,
}
impl Scene {
    /// The only mutable entry point for board content.
    pub fn apply(&mut self, command: Command) -> Result<Command, SceneError> {
        match command {
            Command::Insert(item) => {
                if !item.rect.valid()
                    || !item.transform.valid()
                    || item.id.get_version_num() != 7
                    || item.name.len() > 1024
                {
                    return Err(SceneError::Invalid);
                }
                if self.items.contains_key(&item.id) {
                    return Err(SceneError::Duplicate);
                }
                let inverse = Command::Remove(item.id);
                self.tree.insert(entry(&item));
                self.order.insert((item.z.clone(), item.id));
                self.items.insert(item.id, item);
                Ok(inverse)
            }
            Command::Remove(id) => {
                let item = self.items.remove(&id).ok_or(SceneError::Missing)?;
                self.tree.remove(&entry(&item));
                self.order.remove(&(item.z.clone(), id));
                Ok(Command::Insert(item))
            }
            Command::SetRect { id, rect } => {
                if !rect.valid() {
                    return Err(SceneError::Invalid);
                }
                let item = self.items.get_mut(&id).ok_or(SceneError::Missing)?;
                let old = item.rect;
                self.tree.remove(&entry(item));
                item.rect = rect;
                self.tree.insert(entry(item));
                Ok(Command::SetRect { id, rect: old })
            }
            Command::SetTransform { id, transform } => {
                if !transform.valid() {
                    return Err(SceneError::Invalid);
                }
                let item = self.items.get_mut(&id).ok_or(SceneError::Missing)?;
                let old = item.transform;
                // Only rotation changes the indexed footprint; content gestures skip the tree.
                if old.rotation != transform.rotation {
                    self.tree.remove(&entry(item));
                    item.transform = transform;
                    self.tree.insert(entry(item));
                } else {
                    item.transform = transform;
                }
                Ok(Command::SetTransform { id, transform: old })
            }
            Command::SetText { id, text } => {
                if text.len() > MAX_NOTE_BYTES {
                    return Err(SceneError::Invalid);
                }
                let item = self.items.get_mut(&id).ok_or(SceneError::Missing)?;
                let Content::Text(old) = &item.content else {
                    return Err(SceneError::Invalid);
                };
                let inverse = Command::SetText {
                    id,
                    text: old.clone(),
                };
                item.content = Content::Text(text);
                Ok(inverse)
            }
        }
    }
    /// Uses the spatial index's root so navigation need not scan a huge board each frame.
    pub fn bounds(&self) -> Option<Rect> {
        if self.is_empty() {
            return None;
        }
        let envelope = self.tree.root().envelope();
        let min = envelope.lower();
        let max = envelope.upper();
        Some(Rect {
            x: min[0],
            y: min[1],
            width: max[0] - min[0],
            height: max[1] - min[1],
        })
    }
    /// Returns only visible metadata, sorted back to front.
    pub fn query(&self, rect: Rect) -> Vec<&Item> {
        if !rect.valid() {
            return Vec::new();
        }
        let mut items: Vec<_> = self
            .tree
            .locate_in_envelope_intersecting(&rect.envelope())
            .filter_map(|e| self.items.get(&e.id))
            .collect();
        items.sort_unstable_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        items
    }
    /// Read-only access prevents callers bypassing command recording.
    pub fn get(&self, id: Uuid) -> Option<&Item> {
        self.items.get(&id)
    }
    /// Metadata count is useful without exposing mutable scene storage.
    pub fn len(&self) -> usize {
        self.items.len()
    }
    /// Allows a blank canvas to provide its own empty state.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    /// Enables read-only inspection without allowing mutation outside commands.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }
    /// Supplies an insertion index without using integer z-order.
    pub fn top(&self) -> Option<&FractionalIndex> {
        self.order.last().map(|(z, _)| z)
    }
}

/// Camera math is independent of windowing and device pixel density.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Camera state stays separate from board mutations.
    pub origin: [f32; 2],
    /// Keeps camera scale available to rendering and input.
    pub zoom: f32,
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            origin: [0.0; 2],
            zoom: 1.0,
        }
    }
}
impl Camera {
    /// Keeps zoom anchored under the pointer instead of jumping towards the origin.
    pub fn zoom_at(&mut self, point: [f32; 2], factor: f32) {
        let world = self.world(point);
        self.zoom = (self.zoom * factor).clamp(0.01, 64.0);
        self.origin = [
            world[0] - point[0] / self.zoom,
            world[1] - point[1] / self.zoom,
        ];
    }
    /// Converts logical screen points to world coordinates.
    pub fn world(&self, point: [f32; 2]) -> [f32; 2] {
        [
            self.origin[0] + point[0] / self.zoom,
            self.origin[1] + point[1] / self.zoom,
        ]
    }
    /// Converts world coordinates to logical screen points.
    pub fn screen(&self, point: [f32; 2]) -> [f32; 2] {
        [
            (point[0] - self.origin[0]) * self.zoom,
            (point[1] - self.origin[1]) * self.zoom,
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    #[test]
    fn sampling_inheritance_and_automatic_threshold_use_original_pixels() {
        assert!(!Sampling::Default.nearest(Sampling::Smooth, 10.));
        assert!(Sampling::Default.nearest(Sampling::Nearest, 0.5));
        assert!(!Sampling::Smooth.nearest(Sampling::Nearest, 10.));
        assert!(Sampling::Nearest.nearest(Sampling::Smooth, 0.5));
        assert!(!Sampling::Automatic.nearest(Sampling::Smooth, 1.99));
        assert!(Sampling::Automatic.nearest(Sampling::Smooth, 2.));
        assert!(Sampling::Default.nearest(Sampling::Automatic, 2.));
        assert_eq!(Sampling::from_code(4), None);
        assert!(
            !Transform {
                note_font_size: f32::NAN,
                ..Default::default()
            }
            .valid()
        );
        assert!(
            !Transform {
                note_wrap: true,
                ..Default::default()
            }
            .valid()
        );
    }
    #[test]
    fn source_mapping_undoes_crop_rotation_and_flip() {
        let mut item = Item::image(
            blake3::hash(b"mapping"),
            Rect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 100.,
            },
            None,
        );
        assert_eq!(item.source_uv([25., 75.], [100, 100]), Some([0.25, 0.75]));
        item.transform.flip[0] = true;
        assert_eq!(item.source_uv([25., 75.], [100, 100]), Some([0.75, 0.75]));
        item.transform.content_scale = 2.;
        item.transform.content_offset = [0.25, 0.];
        assert_eq!(item.source_uv([50., 50.], [100, 100]), Some([0.625, 0.5]));
        item.transform.rotation = std::f32::consts::FRAC_PI_2;
        let uv = item.source_uv([50., 75.], [100, 100]).unwrap();
        assert!((uv[0] - 0.5).abs() < 1e-6 && (uv[1] - 0.5).abs() < 1e-6);
        assert_eq!(item.source_uv([-1., 50.], [100, 100]), None);
        item.transform.content_scale = 0.1;
        assert_eq!(item.source_uv([0., 0.], [100, 100]), None);
    }
    #[test]
    fn indexed_bounds_follow_commands_and_undo() {
        let mut scene = Scene::default();
        assert_eq!(scene.bounds(), None);
        let rect = Rect {
            x: -20.,
            y: 10.,
            width: 100.,
            height: 40.,
        };
        let item = Item::image(blake3::hash(b"bounds"), rect, None);
        let id = item.id;
        scene.apply(Command::Insert(item)).unwrap();
        assert_eq!(scene.bounds(), Some(rect));
        let undo = scene
            .apply(Command::SetRect {
                id,
                rect: Rect { x: 200., ..rect },
            })
            .unwrap();
        assert_eq!(scene.bounds().unwrap().x, 200.);
        scene.apply(undo).unwrap();
        assert_eq!(scene.bounds(), Some(rect));
        scene.apply(Command::Remove(id)).unwrap();
        assert_eq!(scene.bounds(), None);
    }
    proptest! {
     #[test] fn apply_undo_identity(x in -10000f32..10000., y in -10000f32..10000., w in 1f32..4096., h in 1f32..4096., rot in -10f32..10., scale in 0.01f32..100., dx in -10f32..10., dy in -10f32..10., opacity in 0f32..=1., grayscale in any::<bool>(), sampling in 0u8..4, note_font_size in 1f32..200., note_wrap in any::<bool>()) {
      let mut scene=Scene::default(); let item=Item::image(blake3::hash(b"image"),Rect{x,y,width:w,height:h},None);
      let id=item.id; let inverse=scene.apply(Command::Insert(item.clone())).unwrap();
      let snapshot=scene.query(item.rect).into_iter().cloned().collect::<Vec<_>>();
      let undo=scene.apply(Command::SetRect{id,rect:Rect{x:x+10.,..item.rect}}).unwrap(); scene.apply(undo).unwrap();
      let transform=Transform{opacity,grayscale,sampling:Sampling::from_code(sampling).unwrap(),note_font_size,note_wrap,rotation:rot,content_offset:[dx,dy],content_scale:scale,..Transform::default()};
      let undo=scene.apply(Command::SetTransform{id,transform}).unwrap();
      prop_assert_eq!(scene.get(id).unwrap().transform,transform);
      scene.apply(undo).unwrap();
      prop_assert_eq!(scene.query(item.rect).into_iter().cloned().collect::<Vec<_>>(),snapshot);
      prop_assert_eq!(scene.apply(Command::SetTransform{id,transform:Transform{content_scale:0.,..transform}}).unwrap_err(),SceneError::Invalid);
      prop_assert_eq!(scene.get(id).unwrap().transform,Transform::default());
      scene.apply(inverse).unwrap(); prop_assert!(scene.is_empty()); prop_assert!(scene.query(item.rect).is_empty());
     }
     #[test] fn zoom_preserves_anchor(x in -1000f32..1000., y in -1000f32..1000., factor in 0.1f32..10.) {
      let mut camera=Camera::default(); let before=camera.world([x,y]); camera.zoom_at([x,y],factor); let after=camera.world([x,y]);
      prop_assert!((before[0]-after[0]).abs()<0.01 && (before[1]-after[1]).abs()<0.01);
     }
    }
    proptest! {
        #[test]
        fn top_tracks_insert_remove_and_undo(operations in prop::collection::vec(any::<bool>(), 0..200)) {
            let mut scene = Scene::default();
            let mut history = Vec::new();
            for insert in operations {
                if insert || history.is_empty() {
                    let item = Item::image(blake3::hash(b"image"), Rect { x: 0., y: 0., width: 1., height: 1. }, scene.top());
                    history.push(scene.apply(Command::Insert(item)).unwrap());
                } else {
                    let command = history.pop().unwrap();
                    let redo = scene.apply(command).unwrap();
                    let undo = scene.apply(redo).unwrap();
                    scene.apply(undo).unwrap();
                }
                prop_assert_eq!(scene.top(), scene.items().map(|item| &item.z).max());
                prop_assert_eq!(scene.order.len(), scene.len());
            }
        }
    }
    #[test]
    fn duplicate_z_survives_removing_one_item() {
        let mut scene = Scene::default();
        let item = Item::image(
            blake3::hash(b"image"),
            Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        let mut duplicate = item.clone();
        duplicate.id = Uuid::now_v7();
        scene.apply(Command::Insert(item.clone())).unwrap();
        scene.apply(Command::Insert(duplicate)).unwrap();
        scene.apply(Command::Remove(item.id)).unwrap();
        assert_eq!(scene.top(), Some(&item.z));
    }
    #[test]
    fn rotation_moves_indexed_bounds_and_queries() {
        let mut scene = Scene::default();
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 10.,
        };
        let item = Item::image(blake3::hash(b"rotated"), rect, None);
        let id = item.id;
        scene.apply(Command::Insert(item)).unwrap();
        assert_eq!(scene.bounds(), Some(rect));
        let quarter = Transform {
            rotation: std::f32::consts::FRAC_PI_2,
            ..Transform::default()
        };
        scene
            .apply(Command::SetTransform {
                id,
                transform: quarter,
            })
            .unwrap();
        let bounds = scene.bounds().unwrap();
        assert!((bounds.x - 45.).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.y + 45.).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.width - 10.).abs() < 1e-3, "{bounds:?}");
        assert!((bounds.height - 100.).abs() < 1e-3, "{bounds:?}");
        // The probe misses the upright frame, so only the rotated envelope can return it.
        let probe = Rect {
            x: 49.,
            y: -40.,
            width: 2.,
            height: 2.,
        };
        assert_eq!(scene.query(probe).len(), 1);
        scene
            .apply(Command::SetTransform {
                id,
                transform: Transform::default(),
            })
            .unwrap();
        assert!(scene.query(probe).is_empty());
    }
    #[test]
    fn frame_corners_and_hit_test_agree_under_rotation() {
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 10.,
        };
        // A square frame keeps the arithmetic readable: a quarter turn is exact.
        let square = Rect {
            width: 10.,
            height: 10.,
            ..rect
        };
        let quarter = std::f32::consts::FRAC_PI_2;
        let corners = square.corners(quarter);
        assert!((corners[0][0] - 10.).abs() < 1e-3, "{corners:?}");
        assert!((corners[0][1] - 0.).abs() < 1e-3, "{corners:?}");
        assert!((corners[3][0] - 0.).abs() < 1e-3, "{corners:?}");
        assert!((corners[3][1] - 10.).abs() < 1e-3, "{corners:?}");
        assert!(
            !square.corners(0.)[0]
                .iter()
                .zip(corners[0])
                .all(|(upright, turned)| (upright - turned).abs() < 1e-3)
        );
        // The upright and the turned frame agree on their own centre and disagree on the
        // upright corner, which rotation carries outside the frame.
        assert!(square.contains_rotated(quarter, [5., 5.]));
        assert!(rect.contains_rotated(0., [10., 5.]));
        assert!(!rect.contains_rotated(quarter, [10., 5.]));
        assert!(rect.contains_rotated(quarter, [50., -40.]));
        assert!(!rect.contains_rotated(quarter, [0., 5.]));
    }
    #[test]
    fn invalid_command_is_atomic() {
        let mut s = Scene::default();
        assert_eq!(
            s.apply(Command::Insert(Item::image(
                blake3::hash(b""),
                Rect {
                    x: f32::NAN,
                    y: 0.,
                    width: 1.,
                    height: 1.
                },
                None
            )))
            .unwrap_err(),
            SceneError::Invalid
        );
        assert!(s.is_empty());
    }
    #[test]
    fn note_text_roundtrips_through_insert_edit_and_undo() {
        let mut scene = Scene::default();
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 10.,
            height: 10.,
        };
        let note = Item::text("hello", rect, None);
        let id = note.id;
        assert_eq!(note.blob(), None);
        assert_eq!(note.note(), Some("hello"));
        let inverse = scene.apply(Command::Insert(note)).unwrap();
        let undo = scene
            .apply(Command::SetText {
                id,
                text: "world".into(),
            })
            .unwrap();
        assert_eq!(scene.get(id).unwrap().note(), Some("world"));
        scene.apply(undo).unwrap();
        assert_eq!(scene.get(id).unwrap().note(), Some("hello"));
        // SetText only targets notes, never images.
        let image = Item::image(blake3::hash(b"i"), rect, None);
        let image_id = image.id;
        scene.apply(Command::Insert(image)).unwrap();
        assert!(matches!(
            scene.apply(Command::SetText {
                id: image_id,
                text: "nope".into(),
            }),
            Err(SceneError::Invalid)
        ));
        scene.apply(inverse).unwrap();
        assert_eq!(scene.len(), 1);
    }
}
