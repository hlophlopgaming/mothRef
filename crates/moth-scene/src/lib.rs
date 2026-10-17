// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
//! Pixel-free board state. All mutations go through reversible commands.
use fractional_index::FractionalIndex;
use rstar::{AABB, RTree, RTreeObject};
use std::collections::{BTreeSet, HashMap};
pub use uuid::Uuid;

/// Identifies original bytes independently of a storage backend or GPU format.
pub type BlobId = blake3::Hash;
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
        AABB::from_corners(
            [self.x, self.y],
            [self.x + self.width, self.y + self.height],
        )
    }
}
/// Persistent identity and ordering never depend on in-process indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// Associates updates with the same object across operations.
    pub id: Uuid,
    /// References original content without retaining pixels.
    pub blob: BlobId,
    /// Keeps indexed bounds available without decoding an image.
    pub rect: Rect,
    /// Allows insertion between images without renumbering the board.
    pub z: FractionalIndex,
}
impl Item {
    /// Appends an image above the supplied fractional index.
    pub fn image(blob: BlobId, rect: Rect, above: Option<&FractionalIndex>) -> Self {
        Self {
            id: Uuid::now_v7(),
            blob,
            rect,
            z: above.map(FractionalIndex::new_after).unwrap_or_default(),
        }
    }
}
#[derive(Clone, PartialEq)]
struct Entry {
    id: Uuid,
    rect: Rect,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.rect.envelope()
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
                if !item.rect.valid() || item.id.get_version_num() != 7 {
                    return Err(SceneError::Invalid);
                }
                if self.items.contains_key(&item.id) {
                    return Err(SceneError::Duplicate);
                }
                let inverse = Command::Remove(item.id);
                self.tree.insert(Entry {
                    id: item.id,
                    rect: item.rect,
                });
                self.order.insert((item.z.clone(), item.id));
                self.items.insert(item.id, item);
                Ok(inverse)
            }
            Command::Remove(id) => {
                let item = self.items.remove(&id).ok_or(SceneError::Missing)?;
                self.tree.remove(&Entry {
                    id,
                    rect: item.rect,
                });
                self.order.remove(&(item.z.clone(), id));
                Ok(Command::Insert(item))
            }
            Command::SetRect { id, rect } => {
                if !rect.valid() {
                    return Err(SceneError::Invalid);
                }
                let item = self.items.get_mut(&id).ok_or(SceneError::Missing)?;
                let old = item.rect;
                self.tree.remove(&Entry { id, rect: old });
                item.rect = rect;
                self.tree.insert(Entry { id, rect });
                Ok(Command::SetRect { id, rect: old })
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
     #[test] fn apply_undo_identity(x in -10000f32..10000., y in -10000f32..10000., w in 1f32..4096., h in 1f32..4096.) {
      let mut scene=Scene::default(); let item=Item::image(blake3::hash(b"image"),Rect{x,y,width:w,height:h},None);
      let id=item.id; let inverse=scene.apply(Command::Insert(item.clone())).unwrap();
      let snapshot=scene.query(item.rect).into_iter().cloned().collect::<Vec<_>>();
      let undo=scene.apply(Command::SetRect{id,rect:Rect{x:x+10.,..item.rect}}).unwrap(); scene.apply(undo).unwrap();
      prop_assert_eq!(scene.query(item.rect).into_iter().cloned().collect::<Vec<_>>(),snapshot);
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
}
