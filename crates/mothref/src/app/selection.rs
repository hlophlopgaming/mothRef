// SPDX-License-Identifier: GPL-3.0-or-later
//! Selection is transient view state; it never changes board contents.
use super::bindings::Action;
use moth_scene::{Scene, Uuid};
use std::collections::HashSet;

/// What a click on `hit` selects: the outermost group around it, unless the user has
/// entered a group on the way, in which case the member directly inside that group.
pub(super) fn target(scene: &Scene, entered: Option<Uuid>, hit: Uuid) -> Uuid {
    let mut chosen = hit;
    for ancestor in scene.ancestors(hit) {
        if Some(ancestor) == entered {
            return chosen;
        }
        chosen = ancestor;
    }
    chosen
}

/// Leaves an entered group once a click lands outside it.
pub(super) fn keep_entered(scene: &Scene, entered: &mut Option<Uuid>, hit: Option<Uuid>) {
    if let Some(group) = *entered
        && !hit.is_some_and(|hit| hit == group || scene.ancestors(hit).any(|a| a == group))
    {
        *entered = None;
    }
}

/// A selected group carries its members, so moves, deletes and copies include them;
/// a group that left the selection takes its members along.
pub(super) fn with_members(scene: &Scene, selected: &mut HashSet<Uuid>, removed: Option<Uuid>) {
    if let Some(group) = removed {
        for member in scene.descendants(group) {
            selected.remove(&member);
        }
    }
    let groups: Vec<_> = selected
        .iter()
        .copied()
        .filter(|id| scene.get(*id).is_some_and(|item| item.is_group()))
        .collect();
    for group in groups {
        selected.extend(scene.descendants(group));
    }
}

/// The selected group when the selection is exactly one group and its members.
pub(super) fn single_group(scene: &Scene, selected: &HashSet<Uuid>) -> Option<Uuid> {
    let mut groups = selected.iter().copied().filter(|id| {
        scene.get(*id).is_some_and(|item| {
            item.is_group() && item.parent.is_none_or(|parent| !selected.contains(&parent))
        })
    });
    let group = groups.next()?;
    (groups.next().is_none() && scene.descendants(group).len() + 1 == selected.len())
        .then_some(group)
}

/// False prevents a deselected item from starting a move or a selection rectangle.
pub(super) fn click(selected: &mut HashSet<Uuid>, id: Uuid, action: Option<Action>) -> bool {
    if action == Some(Action::ToggleDrag) && selected.remove(&id) {
        return false;
    }
    if !selected.contains(&id) {
        if !matches!(action, Some(Action::ExtendDrag | Action::ToggleDrag)) {
            selected.clear();
        }
        selected.insert(id);
    }
    true
}

/// Called once on release, so crossing an item repeatedly cannot toggle it twice.
pub(super) fn area(
    selected: &mut HashSet<Uuid>,
    hits: impl Iterator<Item = Uuid>,
    action: Option<Action>,
) {
    match action {
        Some(Action::ToggleDrag) => {
            for id in hits {
                if !selected.remove(&id) {
                    selected.insert(id);
                }
            }
        }
        Some(Action::ExtendDrag) => selected.extend(hits),
        _ => {
            selected.clear();
            selected.extend(hits);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::{Command, FractionalIndex, Group, Item, Rect};

    #[test]
    fn clicks_pick_the_outer_group_until_it_is_entered() {
        let mut scene = Scene::default();
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 10.,
            height: 10.,
        };
        let outer = Item::group(Group::default(), rect, FractionalIndex::default());
        let mut inner = Item::group(Group::default(), rect, FractionalIndex::default());
        inner.parent = Some(outer.id);
        let mut image = Item::image(moth_scene::BlobId::from_bytes([1; 32]), rect, None);
        image.parent = Some(inner.id);
        let (outer, inner, image) = (outer.id, inner.id, {
            let id = image.id;
            scene.apply(Command::Insert(outer)).unwrap();
            scene.apply(Command::Insert(inner)).unwrap();
            scene.apply(Command::Insert(image)).unwrap();
            id
        });
        assert_eq!(target(&scene, None, image), outer);
        assert_eq!(target(&scene, Some(outer), image), inner);
        assert_eq!(target(&scene, Some(inner), image), image);
        let mut selected = HashSet::from([outer]);
        with_members(&scene, &mut selected, None);
        assert_eq!(selected, HashSet::from([outer, inner, image]));
        assert_eq!(single_group(&scene, &selected), Some(outer));
        selected.remove(&outer);
        with_members(&scene, &mut selected, Some(outer));
        assert!(selected.is_empty());
        let mut entered = Some(inner);
        keep_entered(&scene, &mut entered, Some(image));
        assert_eq!(entered, Some(inner));
        keep_entered(&scene, &mut entered, None);
        assert_eq!(entered, None);
    }

    #[test]
    fn clicks_preserve_group_moves_and_never_move_a_deselected_item() {
        let [a, b, c] = std::array::from_fn(|_| Uuid::now_v7());
        let mut selected = HashSet::from([a, b]);
        assert!(click(&mut selected, a, Some(Action::MoveDrag)));
        assert_eq!(selected, HashSet::from([a, b]));
        assert!(!click(&mut selected, a, Some(Action::ToggleDrag)));
        assert_eq!(selected, HashSet::from([b]));
        assert!(click(&mut selected, a, Some(Action::ToggleDrag)));
        assert!(click(&mut selected, b, Some(Action::ExtendDrag)));
        assert_eq!(selected, HashSet::from([a, b]));
        assert!(click(&mut selected, c, Some(Action::ExtendDrag)));
        assert_eq!(selected, HashSet::from([a, b, c]));
        selected.remove(&c);
        assert!(click(&mut selected, c, Some(Action::MoveDrag)));
        assert_eq!(selected, HashSet::from([c]));
    }

    #[test]
    fn rectangles_replace_add_or_toggle_only_the_final_hits() {
        let [a, b, c] = std::array::from_fn(|_| Uuid::now_v7());
        for (action, expected) in [
            (Action::MoveDrag, HashSet::from([b, c])),
            (Action::ExtendDrag, HashSet::from([a, b, c])),
            (Action::ToggleDrag, HashSet::from([a, c])),
        ] {
            let mut selected = HashSet::from([a, b]);
            area(&mut selected, [b, c].into_iter(), Some(action));
            assert_eq!(selected, expected);
        }
    }

    #[test]
    fn empty_modified_rectangle_preserves_selection() {
        let id = Uuid::now_v7();
        for action in [Action::ExtendDrag, Action::ToggleDrag] {
            let mut selected = HashSet::from([id]);
            area(&mut selected, std::iter::empty(), Some(action));
            assert_eq!(selected, HashSet::from([id]));
        }
        let mut selected = HashSet::from([id]);
        area(&mut selected, std::iter::empty(), Some(Action::MoveDrag));
        assert!(selected.is_empty());
    }
}
