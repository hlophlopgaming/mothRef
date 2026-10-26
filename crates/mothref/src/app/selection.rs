// SPDX-License-Identifier: GPL-3.0-or-later
//! Selection is transient view state; it never changes board contents.
use super::bindings::Action;
use moth_scene::Uuid;
use std::collections::HashSet;

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
