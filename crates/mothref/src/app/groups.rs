// SPDX-License-Identifier: GPL-3.0-or-later
//! Group commands built from the selection. Every change is an ordinary command batch,
//! so grouping undoes, journals and syncs like any other edit.
use moth_scene::{Command, FractionalIndex, Group, Item, MAX_GROUP_DEPTH, Rect, Scene, Uuid};
use std::collections::HashSet;

/// Selected items whose own group is not selected: the tops of the selected subtrees.
fn roots<'a>(scene: &'a Scene, selection: &HashSet<Uuid>) -> Vec<&'a Item> {
    let mut roots: Vec<_> = selection
        .iter()
        .filter_map(|id| scene.get(*id))
        .filter(|item| {
            item.parent
                .is_none_or(|parent| !selection.contains(&parent))
        })
        .collect();
    roots.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
    roots
}
/// Selected groups at the top of their selected subtree.
pub(super) fn selected(scene: &Scene, selection: &HashSet<Uuid>) -> Vec<Uuid> {
    roots(scene, selection)
        .into_iter()
        .filter(|item| item.is_group())
        .map(|item| item.id)
        .collect()
}
/// Puts the selection into a new untitled group, nested where the selection already
/// sat when it shares one group. Returns the new group's id for the title editor.
pub(super) fn create(scene: &Scene, selection: &HashSet<Uuid>) -> Option<(Uuid, Vec<Command>)> {
    let roots = roots(scene, selection);
    let first = roots.first()?;
    let parent = first
        .parent
        .filter(|p| roots.iter().all(|r| r.parent == Some(*p)));
    let depth = parent.map_or(0, |p| scene.ancestors(p).count() + 1);
    if roots
        .iter()
        .any(|root| depth + 1 + scene.height(root.id) > MAX_GROUP_DEPTH)
    {
        return None;
    }
    let hull = roots
        .iter()
        .map(|item| moth_scene::footprint(item))
        .reduce(union)?;
    let mut group = Item::group(
        Group::default(),
        moth_scene::group_frame(hull),
        FractionalIndex::new_before(&first.z),
    );
    group.parent = parent;
    let id = group.id;
    let mut commands = vec![Command::Insert(group)];
    commands.extend(roots.iter().map(|root| Command::SetParent {
        id: root.id,
        parent: Some(id),
    }));
    Some((id, commands))
}
/// Dissolves selected groups: members move to the group's own parent, then the empty
/// group goes.
pub(super) fn dissolve(scene: &Scene, selection: &HashSet<Uuid>) -> Vec<Command> {
    let mut commands = Vec::new();
    for group in selected(scene, selection) {
        let parent = scene.get(group).and_then(|item| item.parent);
        commands.extend(scene.children(group).map(|child| Command::SetParent {
            id: child.id,
            parent,
        }));
        commands.push(Command::Remove(group));
    }
    commands
}
fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    Rect {
        x,
        y,
        width: (a.x + a.width).max(b.x + b.width) - x,
        height: (a.y + a.height).max(b.y + b.height) - y,
    }
}
/// Draw order for the renderer: a backdrop moves down to just before its first member,
/// so a member that sits below its group in z-order is never covered by the tint.
pub(super) fn backdrops_first(scene: &Scene, items: &mut Vec<&Item>) {
    if !items.iter().any(|item| item.is_group()) {
        return;
    }
    let mut index = 0;
    while index < items.len() {
        let item = items[index];
        if item.is_group()
            && let Some(first) = items[..index]
                .iter()
                .position(|other| scene.ancestors(other.id).any(|a| a == item.id))
        {
            items.remove(index);
            items.insert(first, item);
        }
        index += 1;
    }
}
/// Dropping a selection moves it into the innermost group under the pointer, or out
/// to the board when the pointer left every group. A group its last members left is
/// removed in the same step, so no empty backdrop stays behind.
pub(super) fn regroup(scene: &Scene, selection: &HashSet<Uuid>, drop: [f32; 2]) -> Vec<Command> {
    let target = group_at(scene, drop, selection);
    let roots: Vec<_> = selection
        .iter()
        .filter_map(|id| scene.get(*id))
        .filter(|item| {
            item.parent
                .is_none_or(|parent| !selection.contains(&parent))
        })
        .filter(|item| item.parent != target)
        .filter(|item| target.is_none_or(|group| scene.can_join(item.id, group)))
        .collect();
    let mut commands: Vec<_> = roots
        .iter()
        .map(|item| Command::SetParent {
            id: item.id,
            parent: target,
        })
        .collect();
    let left: HashSet<_> = roots.iter().filter_map(|item| item.parent).collect();
    for group in left {
        if scene
            .children(group)
            .all(|child| roots.iter().any(|r| r.id == child.id))
        {
            commands.push(Command::Remove(group));
        }
    }
    commands
}
/// The innermost group whose backdrop contains `point`, skipping `excluded` groups.
pub(super) fn group_at(scene: &Scene, point: [f32; 2], excluded: &HashSet<Uuid>) -> Option<Uuid> {
    scene
        .query(Rect {
            x: point[0],
            y: point[1],
            width: 0.001,
            height: 0.001,
        })
        .into_iter()
        .filter(|item| item.is_group() && !excluded.contains(&item.id))
        .filter(|item| item.rect.contains_rotated(0., point))
        .max_by_key(|item| scene.ancestors(item.id).count())
        .map(|item| item.id)
}
/// Where a new item dropped or pasted at `point` belongs: the group under it, when one
/// more level still fits.
pub(super) fn landing(scene: &Scene, point: [f32; 2]) -> Option<Uuid> {
    group_at(scene, point, &HashSet::new())
        .filter(|group| scene.ancestors(*group).count() + 2 <= MAX_GROUP_DEPTH)
}
/// Moves `id` as the Layers drawer asks: into the group `onto`, above the item `onto`
/// in that item's group, or out to the board with `None`.
pub(super) fn move_layer(scene: &Scene, id: Uuid, onto: Option<Uuid>) -> Vec<Command> {
    let Some(item) = scene.get(id) else {
        return Vec::new();
    };
    let target = onto.and_then(|onto| scene.get(onto));
    let parent = match target {
        Some(target) if target.is_group() => Some(target.id),
        Some(target) => target.parent,
        None => None,
    };
    if parent.is_some_and(|group| !scene.can_join(id, group)) {
        return Vec::new();
    }
    let mut commands = Vec::new();
    match target.filter(|target| !target.is_group() && !item.is_group()) {
        // Above a sibling: a new z between it and the next item up, in one step with
        // the new parent.
        Some(target) => {
            let above = scene
                .items()
                .filter(|other| other.z > target.z && other.id != id)
                .map(|other| &other.z)
                .min();
            let z = match above {
                Some(above) => FractionalIndex::new_between(&target.z, above),
                None => Some(FractionalIndex::new_after(&target.z)),
            };
            let Some(z) = z else {
                return Vec::new();
            };
            let mut moved = item.clone();
            moved.z = z;
            moved.parent = parent;
            commands.extend([Command::Remove(id), Command::Insert(moved)]);
        }
        None if item.parent != parent => commands.push(Command::SetParent { id, parent }),
        None => {}
    }
    if let Some(old) = item.parent.filter(|old| Some(*old) != parent)
        && scene.children(old).all(|child| child.id == id)
    {
        commands.push(Command::Remove(old));
    }
    commands
}
/// Removes items members first, so no group is removed while it still holds anything,
/// and removes groups the removal leaves empty.
pub(super) fn removal(scene: &Scene, mut ids: HashSet<Uuid>) -> Vec<Command> {
    let mut parents: Vec<_> = ids.iter().filter_map(|id| scene.get(*id)?.parent).collect();
    while let Some(group) = parents.pop() {
        if !ids.contains(&group) && scene.children(group).all(|child| ids.contains(&child.id)) {
            ids.insert(group);
            parents.extend(scene.get(group).and_then(|g| g.parent));
        }
    }
    // The reverse of an insertion order: nothing is removed while something left
    // still points at it.
    let items = ids.iter().filter_map(|id| scene.get(*id)).collect();
    moth_scene::insertion_order(items, |item| item)
        .into_iter()
        .rev()
        .map(|item| Command::Remove(item.id))
        .collect()
}
/// Screen rectangles of the title chips drawn this frame, innermost last; the pointer
/// handler hit-tests them before the board so a chip can be grabbed like its group.
pub(super) fn chip_rects(ctx: &egui::Context) -> Vec<(Uuid, egui::Rect)> {
    ctx.data(|data| data.get_temp(egui::Id::new("group_chips")))
        .unwrap_or_default()
}
/// Title chips straddle the top edge of each visible backdrop; the one being renamed
/// becomes a text field. Returns the title edit's outcome when it finishes.
pub(super) fn chips(
    ui: &mut egui::Ui,
    board: &super::Document,
    camera: &moth_scene::Camera,
    shown: &mut [Item],
) -> Vec<super::Intent> {
    let tokens = moth_ui::tokens(ui.ctx());
    let selected = super::selection::single_group(&board.scene, &board.selection);
    let height = ui.spacing().interact_size.y;
    let pad = tokens.space_m as f32;
    let dot = tokens.space_s as f32 / 2. + 0.5;
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut intents = Vec::new();
    let mut rects = Vec::new();
    shown.sort_by_cached_key(|item| board.scene.ancestors(item.id).count());
    // The title field of a group still on its way to the journal opens at once.
    if let Some((id, frame)) = board.pending_group
        && board.scene.get(id).is_none()
        && let Some((_, initial)) = board.renaming.as_ref().filter(|(r, _)| *r == id)
    {
        let corner = camera.screen([frame.x, frame.y]);
        let left = egui::pos2(corner[0] + pad, corner[1] - height / 2.);
        intents.extend(title_editor(ui, id, initial, left, tokens.group(0)));
    }
    for item in shown.iter() {
        let Some(style) = board.group_style(item.id) else {
            continue;
        };
        let corner = camera.screen([item.rect.x, item.rect.y]);
        let left = egui::pos2(corner[0] + pad, corner[1] - height / 2.);
        let swatch = tokens.group(style.color);
        if let Some((_, initial)) = board.renaming.as_ref().filter(|(id, _)| *id == item.id) {
            intents.extend(title_editor(ui, item.id, initial, left, swatch));
            continue;
        }
        let active = selected == Some(item.id);
        let (title, title_color) = if style.title.is_empty() {
            (
                moth_ui::text(moth_ui::Text::UntitledGroup).to_owned(),
                tokens.text_muted,
            )
        } else {
            (style.title.clone(), tokens.text)
        };
        let ink = if active {
            tokens.on_group(style.color)
        } else {
            title_color
        };
        let galley = ui.painter().layout_no_wrap(title, font.clone(), ink);
        let count = ui.painter().layout_no_wrap(
            board.scene.children(item.id).count().to_string(),
            font.clone(),
            if active {
                tokens.on_group(style.color)
            } else {
                tokens.text_muted
            },
        );
        let width = pad + dot * 2. + pad / 1.5 + galley.size().x + pad / 1.5 + count.size().x + pad;
        let rect = egui::Rect::from_min_size(left, egui::vec2(width, height));
        let painter = ui.painter();
        let radius = height / 2.;
        if active {
            painter.rect_filled(rect, radius, swatch);
        } else {
            let [_, border] = tokens.group_backdrop(style.color);
            painter.rect_filled(rect, radius, tokens.group_chip(style.color));
            painter.rect_stroke(
                rect,
                radius,
                egui::Stroke::new(1., border),
                egui::StrokeKind::Inside,
            );
        }
        let mut x = rect.left() + pad;
        painter.circle_filled(
            egui::pos2(x + dot, rect.center().y),
            dot,
            if active {
                tokens.on_group(style.color)
            } else {
                swatch
            },
        );
        x += dot * 2. + pad / 1.5;
        let text_y = rect.center().y - galley.size().y / 2.;
        let title_width = galley.size().x;
        painter.galley(egui::pos2(x, text_y), galley, ink);
        x += title_width + pad / 1.5;
        painter.galley(egui::pos2(x, text_y), count, ink);
        rects.push((item.id, rect));
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new("group_chips"), rects));
    intents
}
fn title_editor(
    ui: &mut egui::Ui,
    id: Uuid,
    initial: &str,
    left: egui::Pos2,
    swatch: egui::Color32,
) -> Option<super::Intent> {
    let key = egui::Id::new(("group_title", id));
    let mut draft = ui
        .ctx()
        .data(|data| data.get_temp::<String>(key))
        .unwrap_or_else(|| initial.to_owned());
    let first = !ui
        .ctx()
        .data(|data| data.get_temp::<bool>(key.with("shown")).unwrap_or(false));
    let tokens = moth_ui::tokens(ui.ctx());
    let mut outcome = None;
    egui::Area::new(key.with("area"))
        .fixed_pos(left)
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                egui::Frame::new()
                    .fill(tokens.group_chip(1).lerp_to_gamma(swatch, 0.1))
                    .stroke(egui::Stroke::new(1.5, swatch))
                    .corner_radius(ui.spacing().interact_size.y / 2.)
                    .inner_margin(egui::Margin::symmetric(tokens.space_m as i8, 2))
                    .show(ui, |ui| {
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut draft)
                                .frame(egui::Frame::NONE)
                                .char_limit(moth_scene::MAX_GROUP_TITLE / 4)
                                .hint_text(moth_ui::text(moth_ui::Text::UntitledGroup))
                                .desired_width(160.),
                        );
                        if first {
                            response.request_focus();
                        }
                        let (enter, escape) = ui.input(|input| {
                            (
                                input.key_pressed(egui::Key::Enter),
                                input.key_pressed(egui::Key::Escape),
                            )
                        });
                        if escape {
                            outcome = Some(super::Intent::GroupTitle(None));
                        } else if enter || (response.lost_focus() && !first) {
                            outcome = Some(super::Intent::GroupTitle(Some(draft.clone())));
                        }
                    });
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(tokens.space_s as i8, 2))
                    .show(ui, |ui| {
                        ui.weak(moth_ui::text(moth_ui::Text::GroupTitleHint));
                    });
            });
        });
    ui.ctx().data_mut(|data| {
        if outcome.is_some() {
            data.remove::<String>(key);
            data.remove::<bool>(key.with("shown"));
        } else {
            data.insert_temp(key, draft);
            data.insert_temp(key.with("shown"), true);
        }
    });
    outcome
}
/// One row of the Layers drawer.
struct Row {
    id: Uuid,
    depth: usize,
    group: bool,
}
/// Board items as a tree, topmost first, skipping members of collapsed groups.
fn rows(scene: &Scene, collapsed: &HashSet<Uuid>) -> Vec<Row> {
    let by_z = |items: &mut Vec<&Item>| {
        items.sort_by(|a, b| b.z.cmp(&a.z).then(b.id.cmp(&a.id)));
    };
    let mut top: Vec<_> = scene.items().filter(|item| item.parent.is_none()).collect();
    by_z(&mut top);
    let mut stack: Vec<_> = top.into_iter().rev().map(|item| (item, 0)).collect();
    let mut out = Vec::new();
    while let Some((item, depth)) = stack.pop() {
        out.push(Row {
            id: item.id,
            depth,
            group: item.is_group(),
        });
        if item.is_group() && !collapsed.contains(&item.id) {
            let mut children: Vec<_> = scene.children(item.id).collect();
            by_z(&mut children);
            stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
        }
    }
    out
}
fn row_label(item: &Item) -> String {
    use moth_ui::{Text, text};
    if let Some(style) = item.group_style() {
        return if style.title.is_empty() {
            text(Text::UntitledGroup).to_owned()
        } else {
            style.title.clone()
        };
    }
    if let Some(note) = item.note() {
        let line = note.lines().next().unwrap_or_default();
        let short: String = line.chars().take(40).collect();
        return format!("{}: {short}", text(Text::LayerNote));
    }
    if item.name.is_empty() {
        text(Text::LayerImage).to_owned()
    } else {
        item.name.clone()
    }
}
/// The Layers drawer: a handle on the right edge that opens a tree of groups and items.
/// Clicking a row selects that item exactly, whatever group it sits in; rows drag onto
/// groups, next to items, or onto the header to leave every group.
pub(super) fn layers(
    ctx: &egui::Context,
    board: &super::Document,
    viewport: egui::Rect,
) -> Vec<super::Intent> {
    use moth_ui::icons::{self, Icon};
    use moth_ui::{Text, text};
    let mut intents = Vec::new();
    let open = ctx.data(|data| {
        data.get_temp::<bool>(egui::Id::new("layers_open"))
            .unwrap_or(false)
    });
    let tokens = moth_ui::tokens(ctx);
    let toggle = || super::Intent::Action(super::bindings::Action::Layers);
    if !open {
        egui::Area::new(egui::Id::new("layers_tab"))
            .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(0., 0.))
            .constrain_to(viewport)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .corner_radius(egui::CornerRadius {
                        nw: tokens.radius_l,
                        sw: tokens.radius_l,
                        ne: 0,
                        se: 0,
                    })
                    .inner_margin(egui::Margin::same(tokens.space_xs as i8))
                    .show(ui, |ui| {
                        if icons::button(ui, Icon::Layers, text(Text::Layers), false).clicked() {
                            intents.push(toggle());
                        }
                    });
            });
        return intents;
    }
    let collapsed_key = egui::Id::new("layers_collapsed");
    let mut collapsed: HashSet<Uuid> = ctx
        .data(|data| data.get_temp(collapsed_key))
        .unwrap_or_default();
    let rows = rows(&board.scene, &collapsed);
    let width = (viewport.width() * 0.8).min(tokens.space_xl as f32 * 11.);
    let row_height = 28.;
    let indent = tokens.space_l as f32;
    egui::Area::new(egui::Id::new("layers_drawer"))
        .anchor(
            egui::Align2::RIGHT_TOP,
            egui::vec2(-(tokens.space_s as f32), tokens.space_s as f32),
        )
        .constrain_to(viewport)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .corner_radius(tokens.radius_l)
                .inner_margin(egui::Margin::same(tokens.space_s as i8))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.horizontal(|ui| {
                        ui.add_space(tokens.space_xs as f32);
                        // The header takes a dropped row out of every group.
                        let header = ui.add(
                            egui::Label::new(egui::RichText::new(text(Text::Layers)).strong())
                                .selectable(false)
                                .sense(egui::Sense::hover()),
                        );
                        ui.label(
                            egui::RichText::new(board.scene.len().to_string())
                                .color(tokens.text_muted),
                        );
                        if let Some(id) = header.dnd_release_payload::<Uuid>() {
                            intents.push(super::Intent::MoveLayer(*id, None));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icons::button(ui, Icon::Close, text(Text::CloseSummary), false)
                                .clicked()
                            {
                                intents.push(toggle());
                            }
                        });
                    });
                    ui.add_space(tokens.space_xs as f32);
                    let max_height =
                        (viewport.height() - tokens.space_xl as f32 * 4.).max(row_height * 3.);
                    ui.spacing_mut().item_spacing.y = 0.;
                    egui::ScrollArea::vertical()
                        .max_height(max_height)
                        .min_scrolled_height((rows.len() as f32 * row_height).min(max_height))
                        .show_rows(ui, row_height, rows.len(), |ui, range| {
                            ui.spacing_mut().item_spacing.y = 0.;
                            for row in &rows[range] {
                                if let Some(item) = board.scene.get(row.id) {
                                    layer_row(
                                        ui,
                                        board,
                                        row,
                                        item,
                                        (row_height, indent),
                                        &mut collapsed,
                                        &mut intents,
                                    );
                                }
                            }
                        });
                });
        });
    ctx.data_mut(|data| data.insert_temp(collapsed_key, collapsed));
    intents
}
/// One tree row: fold chevron or type icon, color dot for groups, truncated name.
fn layer_row(
    ui: &mut egui::Ui,
    board: &super::Document,
    row: &Row,
    item: &Item,
    (row_height, indent): (f32, f32),
    collapsed: &mut HashSet<Uuid>,
    intents: &mut Vec<super::Intent>,
) {
    use moth_ui::icons::{self, Icon};
    let tokens = moth_ui::tokens(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click_and_drag(),
    );
    response.dnd_set_drag_payload(row.id);
    let selected = board.selection.contains(&row.id);
    let target = response
        .dnd_hover_payload::<Uuid>()
        .is_some_and(|id| *id != row.id);
    let painter = ui.painter_at(rect);
    let fill = if selected {
        Some(tokens.selection.gamma_multiply(0.7))
    } else if response.hovered() {
        Some(tokens.hover)
    } else {
        None
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect.shrink2(egui::vec2(0., 1.)), tokens.radius_s, fill);
    }
    if target {
        painter.rect_stroke(
            rect.shrink(1.),
            tokens.radius_s,
            egui::Stroke::new(1.5, tokens.accent),
            egui::StrokeKind::Inside,
        );
    }
    let icon = 14.;
    let mid = rect.center().y;
    let mut x = rect.left() + tokens.space_xs as f32 + row.depth as f32 * indent;
    if row.group {
        // A small chevron folds the group; it is a target of its own.
        let chevron = egui::Rect::from_center_size(
            egui::pos2(x + icon / 2., mid),
            egui::Vec2::splat(icon + 6.),
        );
        let fold = ui.interact(chevron, response.id.with("fold"), egui::Sense::click());
        let folded = collapsed.contains(&row.id);
        icons::paint(
            ui,
            egui::Rect::from_center_size(chevron.center(), egui::Vec2::splat(icon - 2.)),
            if folded {
                Icon::ChevronRight
            } else {
                Icon::ChevronDown
            },
            if fold.hovered() {
                tokens.text
            } else {
                tokens.text_muted
            },
        );
        if fold.clicked() {
            if folded {
                collapsed.remove(&row.id);
            } else {
                collapsed.insert(row.id);
            }
        }
        x += icon + tokens.space_xs as f32;
        let color = item.group_style().map_or(0, |g| g.color);
        painter.circle_filled(egui::pos2(x + 4., mid), 4., tokens.group(color));
        x += 8. + tokens.space_s as f32;
    } else {
        x += icon + tokens.space_xs as f32;
        let kind = if item.note().is_some() {
            Icon::List
        } else {
            Icon::Image
        };
        icons::paint(
            ui,
            egui::Rect::from_center_size(egui::pos2(x + icon / 2., mid), egui::Vec2::splat(icon)),
            kind,
            tokens.text_muted,
        );
        x += icon + tokens.space_s as f32;
    }
    let label = board
        .group_style(item.id)
        .filter(|style| !style.title.is_empty())
        .map(|style| style.title)
        .unwrap_or_else(|| row_label(item));
    let room = (rect.right() - x - tokens.space_s as f32).max(0.);
    let mut job = egui::text::LayoutJob::simple_singleline(
        label.clone(),
        egui::TextStyle::Body.resolve(ui.style()),
        if selected || row.group {
            tokens.text
        } else {
            tokens.text.gamma_multiply(0.85)
        },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let truncated = galley.size().x >= room - 1.;
    painter.galley(
        egui::pos2(x, mid - galley.size().y / 2.),
        galley,
        tokens.text,
    );
    let response = if truncated {
        response.on_hover_text(label)
    } else {
        response
    };
    if response.clicked() {
        intents.push(super::Intent::SelectItem(row.id));
    }
    if let Some(id) = response.dnd_release_payload::<Uuid>()
        && *id != row.id
    {
        intents.push(super::Intent::MoveLayer(*id, Some(row.id)));
    }
}
