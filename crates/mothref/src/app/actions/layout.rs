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
            && let Some(ids) = self.workspace.board.notices.arrange_prompt.clone()
        {
            let mode = match action {
                Horizontal => arrange::Arrange::Horizontal,
                Vertical => arrange::Arrange::Vertical,
                Grid => arrange::Arrange::Grid,
                _ => arrange::Arrange::Optimal,
            };
            self.apply(Intent::Arrange(mode, ids), event_loop);
            self.apply(Intent::DismissArrangePrompt, event_loop);
            return None;
        }
        let mut items = Self::selected_items(&self.workspace.board, action);
        let intent = match action {
            ArrangeOptions => {
                self.tools.arrange.open = true;
                return None;
            }
            ScaleApply => {
                if items.is_empty() {
                    items = self.workspace.board.scene.items().collect();
                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                }
                Intent::Commands(arrange::scale(&items, self.tools.arrange.factor))
            }
            Horizontal | Vertical | Grid | Optimal | ArrangeName | ArrangeTime | PackFit
            | NormalizeWidth | NormalizeHeight | NormalizeSize | NormalizeArea | AlignLeft
            | AlignRight | AlignTop | AlignBottom | Stack | DistributeHorizontal
            | DistributeVertical => {
                if items.is_empty() || action == PackFit {
                    items = self.workspace.board.scene.items().collect();
                    items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
                }
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
                Intent::Commands(commands)
            }
            _ => return None,
        };
        Some(intent)
    }
}
