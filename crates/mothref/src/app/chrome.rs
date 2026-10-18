// SPDX-License-Identifier: GPL-3.0-or-later
//! Window chrome: the titlebar, tabs, status bar and the board catalog.
use crate::gpu::Gpu;
use moth_platform::Platform;
use winit::window::Window;

use super::{Document, Intent};

/// Read-only inputs the chrome draws from; bundled to keep the call small.
pub(super) struct Chrome<'a> {
    pub board: &'a mut Document,
    pub tabs: &'a [Document],
    pub gpu: &'a Gpu,
    pub library: bool,
    pub pinned: bool,
    pub catalog: &'a mut moth_ui::library::Library,
    pub cards: &'a [moth_ui::library::Card],
    pub window: &'a Window,
}

/// Draws the titlebar, tab strip, status bar and (in library mode) the catalog.
pub(super) fn chrome(ui: &mut egui::Ui, ctx: Chrome<'_>) -> Vec<Intent> {
    let mut intents = Vec::new();
    let active_title = ctx.board.title().to_owned();
    let mut ordered = vec![(
        ctx.board.ui_id(),
        moth_ui::Tab {
            title: &active_title,
            dirty: ctx.board.dirty,
            active: true,
            slot: 0,
        },
    )];
    ordered.extend(ctx.tabs.iter().enumerate().map(|(i, tab)| {
        (
            tab.ui_id(),
            moth_ui::Tab {
                title: tab.title(),
                dirty: tab.dirty,
                active: false,
                slot: i + 1,
            },
        )
    }));
    ordered.sort_unstable_by_key(|(id, _)| *id);
    let tab_labels: Vec<_> = ordered.into_iter().map(|(_, tab)| tab).collect();
    let mut actions = moth_ui::toolbar(
        ui,
        moth_ui::Status {
            path: &ctx.board.path,
            adapter: &ctx.gpu.adapter,
            count: ctx.board.scene.len(),
            selected: ctx.board.selection.len(),
            zoom: ctx.board.view.camera.zoom,
            texture_bytes: ctx.gpu.registry.allocated_bytes(),
            message: &ctx.board.message,
            idle: ctx.board.idle(),
            dirty: ctx.board.dirty,
            library: ctx.library,
            pinned: ctx.pinned,
            can_pin: moth_platform::Desktop
                .window_capabilities(ctx.window)
                .on_top,
        },
        &mut ctx.board.view.pan,
        &tab_labels,
    );
    if ctx.library {
        let intent = ctx.catalog.show(ui, ctx.cards);
        actions.new |= intent.new;
        actions.open |= intent.browse;
        if let Some(index) = intent.open {
            intents.push(Intent::OpenPath(ctx.cards[index].path.clone()));
        }
        if let Some((index, tags)) = intent.tags {
            intents.push(Intent::Library(moth_platform::library::Request::Tags(
                ctx.cards[index].path.clone(),
                tags,
            )));
        }
        if let Some(index) = intent.pin {
            let card = &ctx.cards[index];
            intents.push(Intent::Library(moth_platform::library::Request::Pin(
                card.path.clone(),
                !card.pinned,
            )));
        }
    }
    intents.extend(super::intent::intents_from_actions(actions));
    intents
}
