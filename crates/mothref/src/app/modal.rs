// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns screen and modal transitions, including the state restored after a path dialog.
use moth_scene::Uuid;

/// Chooses the primary surface independently of the overlay controller.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Screen {
    Board,
    Library,
}

/// Keeps an uncommitted note draft separate from journaled scene data.
pub(super) struct Editing {
    pub(super) id: Uuid,
    pub(super) original: String,
    pub(super) draft: String,
    pub(super) anchor: [f32; 2],
    pub(super) focus: bool,
}

/// Owns modal state and restores the underlying editor or close confirmation on dismissal.
#[derive(Default)]
pub(super) enum Modal {
    #[default]
    None,
    PathDialog {
        path: String,
        focus: bool,
        resume: Box<Modal>,
    },
    ConfirmClose {
        editing: Option<Editing>,
    },
    EditingNote(Editing),
}
impl Modal {
    pub(super) fn editing(&self) -> Option<&Editing> {
        match self {
            Self::EditingNote(editing) => Some(editing),
            Self::ConfirmClose { editing } => editing.as_ref(),
            Self::PathDialog { resume, .. } => resume.editing(),
            Self::None => None,
        }
    }
    pub(super) fn editing_mut(&mut self) -> Option<&mut Editing> {
        match self {
            Self::EditingNote(editing) => Some(editing),
            Self::ConfirmClose { editing } => editing.as_mut(),
            Self::PathDialog { resume, .. } => resume.editing_mut(),
            Self::None => None,
        }
    }
    pub(super) fn take_editor(&mut self) -> Option<Editing> {
        match self {
            Self::ConfirmClose { editing } => editing.take(),
            Self::PathDialog { resume, .. } => resume.take_editor(),
            Self::EditingNote(_) => match std::mem::take(self) {
                Self::EditingNote(editing) => Some(editing),
                _ => unreachable!("the variant was checked before taking the editor"),
            },
            Self::None => None,
        }
    }
    pub(super) fn edit(&mut self, draft: Editing) {
        match self {
            Self::PathDialog { resume, .. } => resume.edit(draft),
            Self::ConfirmClose { editing } => *editing = Some(draft),
            _ => *self = Self::EditingNote(draft),
        }
    }
    pub(super) fn confirming(&self) -> bool {
        match self {
            Self::ConfirmClose { .. } => true,
            Self::PathDialog { resume, .. } => resume.confirming(),
            _ => false,
        }
    }
    pub(super) fn confirm_close(&mut self) {
        if let Self::PathDialog { resume, .. } = self {
            resume.confirm_close();
        } else if !self.confirming() {
            *self = Self::ConfirmClose {
                editing: self.take_editor(),
            };
        }
    }
    pub(super) fn keep_open(&mut self) {
        match self {
            Self::PathDialog { resume, .. } => resume.keep_open(),
            Self::ConfirmClose { editing } => {
                *self = editing.take().map_or(Self::None, Self::EditingNote)
            }
            _ => {}
        }
    }
    pub(super) fn path(&self) -> Option<&str> {
        match self {
            Self::PathDialog { path, .. } => Some(path),
            _ => None,
        }
    }
    pub(super) fn open_path(&mut self) {
        if let Self::PathDialog { path, focus, .. } = self {
            path.clear();
            *focus = true;
        } else {
            *self = Self::PathDialog {
                path: String::new(),
                focus: true,
                resume: Box::new(std::mem::take(self)),
            };
        }
    }
    pub(super) fn dismiss_path(&mut self) {
        if let Self::PathDialog { resume, .. } = std::mem::take(self) {
            *self = *resume;
        }
    }
    /// Mutable path text and its request-focus flag, for the path dialog editor only.
    pub(super) fn path_editor(&mut self) -> Option<(&mut String, &mut bool)> {
        match self {
            Self::PathDialog { path, focus, .. } => Some((path, focus)),
            _ => None,
        }
    }
    pub(super) fn accepts_canvas(&self, screen: Screen) -> bool {
        screen == Screen::Board && !self.confirming() && self.path().is_none()
    }
}
