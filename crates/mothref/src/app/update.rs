// SPDX-License-Identifier: GPL-3.0-or-later
//! Update dialog, native actions and the handoff from live boards to the updater.
use super::{App, Document, Intent, bindings::Action};
use crossbeam_channel::Receiver;
use moth_platform::update::{self, Armed, Board, Prepared, Release, Session, Startup};
use moth_ui::{Text, text};
use std::{io, sync::Arc};
use winit::event_loop::ActiveEventLoop;

type Work = Receiver<io::Result<Outcome>>;

enum Outcome {
    Offer(Option<Release>),
    Notes(Release),
    Download(Box<Prepared>),
    Armed(Armed),
    Skipped,
    Started(Startup),
}

#[derive(Default)]
pub(super) struct Updates {
    checked: bool,
    startup_pending: bool,
    starting: bool,
    arming: bool,
    offer: Option<Release>,
    notes: Option<Release>,
    window: Option<winit::window::WindowId>,
    showing: bool,
    show_offer: bool,
    error: Option<Text>,
    check: Option<Work>,
    work: Option<Work>,
    prepared: Option<Box<Prepared>>,
    pub(super) restart: Option<Armed>,
}

impl Updates {
    pub(super) fn new() -> Self {
        Self {
            startup_pending: update::resuming(),
            ..Self::default()
        }
    }

    pub(super) fn check(&mut self, enabled: bool, wake: Arc<dyn Fn() + Send + Sync>) {
        if !std::mem::replace(&mut self.checked, true)
            && enabled
            && !self.startup_pending
            && !self.starting
        {
            self.check = Some(worker(wake, || update::check().map(Outcome::Offer)));
        }
    }

    pub(super) fn presented(&mut self, wake: Arc<dyn Fn() + Send + Sync>) {
        if std::mem::take(&mut self.startup_pending) {
            self.starting = true;
            self.work = Some(worker(wake, || update::startup().map(Outcome::Started)));
        }
    }

    fn resuming(&self) -> bool {
        self.startup_pending || self.starting
    }

    pub(super) fn switching_versions(&self) -> bool {
        self.resuming() || self.arming
    }

    pub(super) fn enabled(&self, action: Action) -> bool {
        match action {
            Action::UpdateDownload => {
                self.offer.is_some() && self.work.is_none() && self.prepared.is_none()
            }
            Action::UpdateInstall => {
                self.prepared.is_some() && self.work.is_none() && self.restart.is_none()
            }
            Action::UpdateLater => self.showing && self.work.is_none(),
            Action::UpdateSkip => {
                self.offer.is_some() && self.work.is_none() && self.prepared.is_none()
            }
            Action::UpdateShowOffer => self.offer.is_some() && !self.switching_versions(),
            Action::UpdateWhatsNew => self.work.is_none() && !self.switching_versions(),
            _ => unreachable!("update action table"),
        }
    }

    pub(super) fn show(
        &self,
        ui: &mut egui::Ui,
        enabled: &[bool],
        window: winit::window::WindowId,
    ) -> Vec<Intent> {
        if !self.showing || self.window != Some(window) {
            return Vec::new();
        }
        let mut intents = Vec::new();
        let available = ui.ctx().content_rect().size();
        let width = (available.x * 0.80).min(620.);
        let offer = self.offer.as_ref().filter(|_| self.show_offer);
        let prepared = offer.is_some() && self.prepared.is_some();
        let release = if self.show_offer {
            offer
        } else {
            self.notes.as_ref()
        };
        let title = text(if offer.is_some() {
            Text::UpdateAvailable
        } else {
            Text::UpdateWhatsNew
        });
        egui::Window::new(title)
            .id(egui::Id::unique("mothref_update"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .default_width(width)
            .max_width(width)
            .show(ui.ctx(), |ui| {
                ui.set_width(width);
                if let Some(release) = release {
                    ui.heading(release.tag());
                    if !release.notes().trim().is_empty() {
                        egui::ScrollArea::vertical()
                            .max_width(width)
                            .max_height(available.y * 0.55)
                            .show(ui, |ui| release_notes(ui, release.notes()));
                    } else {
                        ui.label(text(Text::UpdateNoNotes));
                    }
                }
                if let Some(error) = self.error {
                    ui.label(text(error));
                } else if self.work.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(text(if self.arming {
                            Text::UpdatePreparingRestart
                        } else if offer.is_some() {
                            Text::UpdateDownloading
                        } else {
                            Text::UpdateLoadingNotes
                        }));
                    });
                } else if prepared {
                    ui.label(text(Text::UpdateReady));
                }
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let actions: &[Action] = if prepared {
                        &[Action::UpdateInstall, Action::UpdateLater]
                    } else if offer.is_some() {
                        &[
                            Action::UpdateDownload,
                            Action::UpdateLater,
                            Action::UpdateSkip,
                        ]
                    } else {
                        &[Action::UpdateLater]
                    };
                    for &action in actions {
                        let label = if action == Action::UpdateLater && offer.is_none() {
                            Text::CloseSummary
                        } else {
                            action.label()
                        };
                        if ui
                            .add_enabled(enabled[action as usize], egui::Button::new(text(label)))
                            .clicked()
                        {
                            intents.push(Intent::Action(action));
                        }
                    }
                });
            });
        intents
    }
}

fn release_notes(ui: &mut egui::Ui, notes: &str) {
    for line in notes.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            ui.add_space(ui.spacing().item_spacing.y);
            continue;
        }
        if let Some(heading) = trimmed.trim_start_matches('#').strip_prefix(' ')
            && trimmed.starts_with('#')
        {
            ui.add(egui::Label::new(egui::RichText::new(inline_text(heading)).strong()).wrap());
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            ui.horizontal_top(|ui| {
                ui.label("•");
                ui.add(egui::Label::new(inline_text(item)).wrap());
            });
        } else {
            ui.add(egui::Label::new(inline_text(trimmed)).wrap());
        }
    }
}

fn inline_text(markdown: &str) -> String {
    let mut result = String::new();
    let mut rest = markdown;
    // Resource links stay inert in the startup dialog.
    while let Some((before, after)) = rest.split_once('[') {
        result.push_str(before);
        let Some((label, suffix)) = after.split_once(']') else {
            result.push('[');
            rest = after;
            break;
        };
        if let Some((_, tail)) = suffix.strip_prefix('(').and_then(|url| url.split_once(')')) {
            result.push_str(label);
            rest = tail;
        } else {
            result.push('[');
            result.push_str(label);
            result.push(']');
            rest = suffix;
        }
    }
    result.push_str(rest);
    result.replace("**", "").replace('`', "")
}

fn worker(
    wake: Arc<dyn Fn() + Send + Sync>,
    task: impl FnOnce() -> io::Result<Outcome> + Send + 'static,
) -> Work {
    let (send, receive) = crossbeam_channel::bounded(1);
    let worker_send = send.clone();
    let wake_worker = wake.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("update".into())
        .spawn(move || {
            let _ = worker_send.send(task());
            wake_worker();
        })
    {
        let _ = send.send(Err(error));
        wake();
    }
    receive
}

fn poll(work: &mut Option<Work>) -> Option<io::Result<Outcome>> {
    let result = match work.as_ref()?.try_recv() {
        Ok(result) => result,
        Err(crossbeam_channel::TryRecvError::Empty) => return None,
        Err(crossbeam_channel::TryRecvError::Disconnected) => {
            Err(io::Error::other("Update worker stopped"))
        }
    };
    *work = None;
    Some(result)
}

impl App {
    pub(super) fn update_action(&mut self, action: Action) {
        use Action::*;
        self.updates.error = None;
        let wake = self.shell.wake();
        match action {
            UpdateDownload => {
                if let Some(release) = self.updates.offer.clone() {
                    self.updates.work = Some(worker(wake, move || {
                        Prepared::download(release)
                            .map(Box::new)
                            .map(Outcome::Download)
                    }));
                }
            }
            UpdateInstall => self.install_update(),
            UpdateLater => {
                self.updates.showing = false;
            }
            UpdateSkip => {
                if let Some(release) = self.updates.offer.clone() {
                    self.updates.work = Some(worker(wake, move || {
                        update::skip(&release).map(|()| Outcome::Skipped)
                    }));
                }
            }
            UpdateShowOffer | UpdateWhatsNew => {
                self.updates.window = self.shell.window.as_ref().map(|window| window.id());
                self.updates.showing = true;
                self.updates.show_offer = action == UpdateShowOffer;
                if action == UpdateWhatsNew && self.updates.notes.is_none() {
                    self.updates.work =
                        Some(worker(wake, || update::current().map(Outcome::Notes)));
                }
            }
            _ => unreachable!("update action table"),
        }
        self.redraw();
    }

    pub(super) fn update_results(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(result) = poll(&mut self.updates.check) {
            match result {
                Ok(Outcome::Offer(Some(release))) if self.settings.update_checks => {
                    self.updates.offer = Some(release);
                    if !self.updates.showing {
                        self.updates.window = self.shell.window.as_ref().map(|window| window.id());
                        self.updates.showing = true;
                        self.updates.show_offer = true;
                    }
                    self.redraw();
                }
                Err(error) => tracing::debug!(%error, "startup update check failed"),
                _ => {}
            }
        }
        let Some(result) = poll(&mut self.updates.work) else {
            return;
        };
        self.updates.arming = false;
        match result {
            Ok(Outcome::Armed(armed)) => {
                self.updates.restart = Some(armed);
                event_loop.exit();
            }
            Ok(Outcome::Notes(notes)) => self.updates.notes = Some(notes),
            Ok(Outcome::Download(prepared)) => self.updates.prepared = Some(prepared),
            Ok(Outcome::Skipped) => {
                self.updates.showing = false;
                self.updates.offer = None;
            }
            Ok(Outcome::Started(startup)) => {
                self.updates.starting = false;
                if let Some(release) = startup.release {
                    self.updates.notes = Some(release);
                    self.updates.showing = true;
                }
                if startup.failed {
                    self.updates.error = Some(Text::UpdateRolledBack);
                    self.updates.showing = true;
                }
                self.restore_update_session(startup.session, event_loop);
                self.updates.window = self.shell.window.as_ref().map(|window| window.id());
            }
            Err(error) if self.updates.starting => {
                self.startup_error = Some(error.into());
                event_loop.exit();
            }
            Err(error) => {
                tracing::warn!(%error, "update request failed");
                self.updates.error = Some(
                    if self.updates.show_offer && error.kind() == io::ErrorKind::PermissionDenied {
                        Text::UpdateInstallReadOnly
                    } else {
                        Text::UpdateFailed
                    },
                );
            }
            Ok(Outcome::Offer(_)) => {}
        }
        self.redraw();
    }

    fn install_update(&mut self) {
        let mut session = Session::new();
        let mut result = Ok(());
        self.each_pane(|app| {
            if result.is_err() || app.window.window_closed {
                return;
            }
            if app.editing.is_some()
                || app.dragging()
                || app.workspace.confirm_close
                || app.window.file_picker.is_some()
                || app.window.save_picker.is_some()
                || app.window.export_picker.is_some()
                || app.modal.dialog.is_some()
            {
                result = Err(io::Error::other(
                    "Finish current board editing before restarting",
                ));
                return;
            }
            result = (|| {
                let mut window = Vec::new();
                for board in app.workspace.tabs.iter().chain([&app.workspace.board]) {
                    if !board.idle() || board.intake.loading || board.session.is_some() {
                        return Err(io::Error::other("Boards are busy"));
                    }
                    if let Some(board) = board.restart_board()? {
                        window.push(board);
                    }
                }
                session.push(window);
                Ok(())
            })();
        });
        if result.is_err() {
            self.updates.error = Some(Text::UpdateBoardsBusy);
            return;
        }
        if let Some(prepared) = self.updates.prepared.take() {
            self.updates.arming = true;
            self.updates.work = Some(worker(self.shell.wake(), move || {
                prepared.arm(session).map(Outcome::Armed)
            }));
        }
    }

    fn restore_update_session(&mut self, session: Session, event_loop: &ActiveEventLoop) {
        for (index, window) in session.into_iter().enumerate() {
            if index != 0 {
                let result = Document::new(self.shell.wake(), false)
                    .and_then(|board| self.open_window(event_loop, board, None));
                if let Err(error) = result {
                    tracing::warn!(%error, "could not restore a board window");
                    self.updates.error = Some(Text::UpdateRestoreFailed);
                    self.updates.showing = true;
                    continue;
                }
            }
            for board in window {
                match board {
                    Board::Saved { path, journal } => {
                        self.open_in_tab(path.into());
                        if self.workspace.board.intake.loading {
                            self.workspace.board.file.restart_journal = Some(journal.into());
                        }
                    }
                    Board::Journal(journal) => {
                        self.new_board();
                        self.workspace.restore_journal(journal.into());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn changelog_links_and_inline_markup_render_as_inert_text() {
        assert_eq!(
            super::inline_text("**Fixed** `save` in [boards](https://example.com)"),
            "Fixed save in boards"
        );
        assert_eq!(super::inline_text("Unlinked [text]"), "Unlinked [text]");
    }
}
