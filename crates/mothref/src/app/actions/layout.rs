// SPDX-License-Identifier: GPL-3.0-or-later
//! Arranging several items: layouts, normalize, align, distribute and scale.
use super::*;
impl App {
    /// Whether an action of this area is available once its [`Gate`] is passed.
    pub(super) fn arrange_enabled(&self, _action: Action) -> bool {
        !self.workspace.board.scene.is_empty()
    }
    /// Runs an action of this area; `None` when it finished without an intent.
    pub(super) fn run_arrange(
        &mut self,
        action: Action,
        event_loop: &ActiveEventLoop,
    ) -> Option<Intent> {
        use Action::*;
        // While the post-import prompt is open, layouts apply to the imported items.
        if matches!(action, Horizontal | Vertical | Grid | Optimal)
            && let Some(ids) = self.workspace.board.intake.burst.prompt().cloned()
        {
            let mode = match action {
                Horizontal => arrange::Arrange::Horizontal,
                Vertical => arrange::Arrange::Vertical,
                Grid => arrange::Arrange::Grid,
                _ => arrange::Arrange::Optimal,
            };
            self.apply(Intent::Arrange(mode, ids), event_loop);
            return None;
        }
        let mut items = layout_items(&self.workspace.board.scene, &self.workspace.board.selection);
        let intent = match action {
            ArrangeOptions => {
                self.tools.arrange.open = true;
                return None;
            }
            ScaleApply => Intent::Commands(arrange::scale(&items, self.tools.arrange.factor)),
            Horizontal | Vertical | Grid | Optimal | ArrangeName | ArrangeTime | PackFit
            | NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea | AlignLeft
            | AlignRight | AlignTop | AlignBottom | Stack | DistributeHorizontal
            | DistributeVertical => {
                let commands = match action {
                    NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea => {
                        arrange::normalize(
                            &items,
                            match action {
                                NormalizeWidth => arrange::Normalize::Width,
                                NormalizeHeight => arrange::Normalize::Height,
                                NormalizeSize => arrange::Normalize::Size,
                                _ => arrange::Normalize::Area,
                            },
                        )
                    }
                    AlignLeft | AlignRight | AlignTop | AlignBottom | Stack
                    | DistributeHorizontal | DistributeVertical => arrange::align(
                        &items,
                        match action {
                            AlignLeft => arrange::Align::Left,
                            AlignRight => arrange::Align::Right,
                            AlignTop => arrange::Align::Top,
                            AlignBottom => arrange::Align::Bottom,
                            DistributeHorizontal => arrange::Align::Horizontal,
                            DistributeVertical => arrange::Align::Vertical,
                            _ => arrange::Align::Stack,
                        },
                    ),
                    _ => {
                        if action == ArrangeName {
                            self.tools.arrange.order = 1;
                        }
                        if action == ArrangeTime {
                            self.tools.arrange.order = 2;
                        }
                        self.tools.arrange(
                            &mut items,
                            match action {
                                Horizontal => arrange::Arrange::Horizontal,
                                Vertical => arrange::Arrange::Vertical,
                                Grid => arrange::Arrange::Grid,
                                _ => arrange::Arrange::Optimal,
                            },
                            &self.settings,
                        )
                    }
                };
                if action == PackFit {
                    let ids = items.iter().map(|item| item.id).collect();
                    let in_flight = self.workspace.board.intake.in_flight;
                    self.workspace
                        .persist_batch(commands.into_iter().map(|c| (c, None)).collect());
                    if self.workspace.board.intake.in_flight > in_flight {
                        self.workspace.board.intake.layout_fit = Some(ids);
                    }
                    return None;
                }
                Intent::Commands(commands)
            }
            _ => return None,
        };
        Some(intent)
    }
}

/// An empty selection means the board; an empty selected group means no members.
fn layout_items<'a>(scene: &'a Scene, selection: &HashSet<Uuid>) -> Vec<&'a Item> {
    let mut items: Vec<_> = if selection.is_empty() {
        scene.items().collect()
    } else {
        selection.iter().filter_map(|id| scene.get(*id)).collect()
    };
    items.retain(|item| !item.is_group());
    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pack_scope_keeps_single_selection_and_empty_group_off_the_rest_of_the_board() {
        let mut scene = Scene::default();
        let rect = Rect {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 100.,
        };
        let a = Item::text("a", rect, None);
        let b = Item::text("b", rect, Some(&a.z));
        let group = Item::group(Default::default(), rect, Default::default());
        let (a_id, b_id, group_id) = (a.id, b.id, group.id);
        for item in [a, b, group] {
            scene.apply(Command::Insert(item)).unwrap();
        }
        assert_eq!(
            layout_items(&scene, &HashSet::from([a_id]))
                .iter()
                .map(|i| i.id)
                .collect::<Vec<_>>(),
            vec![a_id]
        );
        assert!(layout_items(&scene, &HashSet::from([group_id])).is_empty());
        assert_eq!(layout_items(&scene, &HashSet::new()).len(), 2);
        scene
            .apply(Command::SetParent {
                id: a_id,
                parent: Some(group_id),
            })
            .unwrap();
        let mut selection = HashSet::from([group_id]);
        selection::with_members(&scene, &mut selection, None);
        let items = layout_items(&scene, &selection);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, a_id);
        assert!(!items.iter().any(|i| i.id == b_id));
    }
}
