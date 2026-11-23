// SPDX-License-Identifier: GPL-3.0-or-later
//! How a board takes in what the storage worker confirmed, shared by the window and
//! the command line. Only journal-acknowledged commands reach the scene, and they
//! reach it the same way in both; what each caller shows for them (selection and
//! motion in the window, JSON in the CLI) stays with that caller.
use std::{collections::HashMap, path::PathBuf};

use moth_format::Handle;
use moth_platform::boards::Completion;
use moth_scene::{BlobId, Command, Rect, Scene, SceneError, Uuid};

/// A board whose confirmed state the storage worker's answers update.
pub(super) trait Board {
    fn scene(&self) -> &Scene;
    /// Applies one journal-acknowledged command.
    fn apply(&mut self, command: Command) -> Result<(), SceneError>;
    /// Replaces the scene with an opened board or an empty one for an import.
    fn replace(&mut self, scene: Scene);
    /// Journal-backed originals by content hash.
    fn sources(&mut self) -> &mut HashMap<BlobId, Handle>;
    /// Whether the board has changes not saved to its file.
    fn set_dirty(&mut self, dirty: bool);
}

/// What a storage answer changed on the board.
pub(super) enum Change {
    /// Journaled commands reached the scene.
    Applied {
        /// Every item a command touched, in order, once each.
        ids: Vec<Uuid>,
        /// Items a batch inserted that were not on the board before it.
        inserted: Vec<Uuid>,
        /// The answer was a batch (the window selects what it inserted).
        batch: bool,
        /// Commands the scene refused; the rest of the answer still applied.
        errors: Vec<SceneError>,
    },
    /// Part of a board being opened or imported; the request is not finished.
    Loading {
        /// This was the first part: the board was empty before it.
        first: bool,
        /// Board bounds, when known before the originals load.
        bounds: Option<Rect>,
        errors: Vec<SceneError>,
    },
    /// A board finished opening.
    Opened {
        recovered: bool,
        destination: Option<PathBuf>,
    },
    /// A `.bee` or `.pur` board finished importing into a new, unsaved board.
    Imported {
        skipped: usize,
        errors: Vec<SceneError>,
    },
    /// The board was saved.
    Saved,
    /// Not about the board's confirmed state; the caller handles it.
    Other(Box<Completion>),
}

/// Applies a storage answer to `board` and says what changed.
pub(super) fn confirm(board: &mut impl Board, done: Completion) -> Change {
    match done {
        Completion::Applied(command, source) => {
            let id = command.id();
            let mut errors = Vec::new();
            match board.apply(command) {
                Ok(()) => {
                    if let Some((hash, handle)) = source {
                        board.sources().insert(hash, handle);
                    }
                    board.set_dirty(true);
                }
                Err(error) => errors.push(error),
            }
            Change::Applied {
                ids: vec![id],
                inserted: Vec::new(),
                batch: false,
                errors,
            }
        }
        Completion::AppliedBatch(commands, sources) => {
            let mut ids = Vec::with_capacity(commands.len());
            let mut inserted = Vec::new();
            for command in &commands {
                let id = command.id();
                if !ids.contains(&id) {
                    ids.push(id);
                }
                if matches!(command, Command::Insert(_)) && board.scene().get(id).is_none() {
                    inserted.push(id);
                }
            }
            let errors = apply_all(board, commands);
            board.sources().extend(sources);
            board.set_dirty(true);
            Change::Applied {
                ids,
                inserted,
                batch: true,
                errors,
            }
        }
        Completion::Loading {
            commands,
            sources,
            bounds,
        } => {
            let first = board.scene().is_empty();
            let errors = apply_all(board, commands);
            board.sources().extend(sources);
            Change::Loading {
                first,
                bounds,
                errors,
            }
        }
        Completion::Opened(scene, sources, recovered, destination) => {
            board.replace(scene);
            *board.sources() = sources;
            // A recovered journal holds changes the file does not have yet.
            board.set_dirty(recovered);
            Change::Opened {
                recovered,
                destination,
            }
        }
        Completion::Imported(commands, sources, skipped) => {
            board.replace(Scene::default());
            let errors = apply_all(board, commands);
            *board.sources() = sources;
            board.set_dirty(true);
            Change::Imported { skipped, errors }
        }
        Completion::Saved => {
            board.set_dirty(false);
            Change::Saved
        }
        other => Change::Other(Box::new(other)),
    }
}

/// Applies every command, collecting refusals instead of stopping: each one was
/// journaled, so the rest must still reach the scene.
fn apply_all(board: &mut impl Board, commands: Vec<Command>) -> Vec<SceneError> {
    commands
        .into_iter()
        .filter_map(|command| board.apply(command).err())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use moth_scene::Item;

    #[derive(Default)]
    struct Fake {
        scene: Scene,
        sources: HashMap<BlobId, Handle>,
        dirty: bool,
    }
    impl Board for Fake {
        fn scene(&self) -> &Scene {
            &self.scene
        }
        fn apply(&mut self, command: Command) -> Result<(), SceneError> {
            self.scene.apply(command).map(|_| ())
        }
        fn replace(&mut self, scene: Scene) {
            self.scene = scene;
        }
        fn sources(&mut self) -> &mut HashMap<BlobId, Handle> {
            &mut self.sources
        }
        fn set_dirty(&mut self, dirty: bool) {
            self.dirty = dirty;
        }
    }
    fn note(x: f32) -> Item {
        Item::text(
            "n",
            Rect {
                x,
                y: 0.,
                width: 10.,
                height: 10.,
            },
            None,
        )
    }
    fn handle() -> Handle {
        Handle(1)
    }

    #[test]
    fn applied_commands_reach_the_scene_and_mark_it_unsaved() {
        let mut board = Fake::default();
        let a = note(0.);
        let change = confirm(
            &mut board,
            Completion::Applied(Command::Insert(a.clone()), None),
        );
        assert!(
            matches!(&change, Change::Applied { ids, batch: false, errors, .. }
            if ids == &[a.id] && errors.is_empty())
        );
        assert!(board.dirty && board.scene.get(a.id).is_some());
        // A refused command reports and leaves the board as it was.
        board.dirty = false;
        let change = confirm(
            &mut board,
            Completion::Applied(Command::Insert(a.clone()), None),
        );
        assert!(matches!(change, Change::Applied { errors, .. } if errors.len() == 1));
        assert!(!board.dirty);
    }

    #[test]
    fn a_batch_applies_every_journaled_command_and_names_its_inserts() {
        let mut board = Fake::default();
        let a = note(0.);
        let b = note(20.);
        confirm(
            &mut board,
            Completion::Applied(Command::Insert(a.clone()), None),
        );
        let hash = BlobId::from_bytes([1; 32]);
        let change = confirm(
            &mut board,
            Completion::AppliedBatch(
                vec![
                    // Refused (already there), then two that must still apply.
                    Command::Insert(a.clone()),
                    Command::Insert(b.clone()),
                    Command::Remove(a.id),
                ],
                vec![(hash, handle())],
            ),
        );
        let Change::Applied {
            ids,
            inserted,
            batch,
            errors,
        } = change
        else {
            panic!("a batch is an applied change");
        };
        assert!(batch);
        assert_eq!(ids, vec![a.id, b.id]);
        assert_eq!(inserted, vec![b.id]);
        assert_eq!(errors.len(), 1);
        assert!(board.scene.get(b.id).is_some() && board.scene.get(a.id).is_none());
        assert!(board.sources.contains_key(&hash));
    }

    #[test]
    fn loading_builds_the_board_until_opened_replaces_it() {
        let mut board = Fake::default();
        let a = note(0.);
        let change = confirm(
            &mut board,
            Completion::Loading {
                commands: vec![Command::Insert(a.clone())],
                sources: Vec::new(),
                bounds: None,
            },
        );
        assert!(matches!(change, Change::Loading { first: true, .. }));
        assert!(!board.dirty, "loading is not an edit");
        let change = confirm(
            &mut board,
            Completion::Loading {
                commands: Vec::new(),
                sources: Vec::new(),
                bounds: None,
            },
        );
        assert!(matches!(change, Change::Loading { first: false, .. }));
        // The opened scene is authoritative; a recovered journal is unsaved work.
        let mut opened = Scene::default();
        let b = note(20.);
        opened.apply(Command::Insert(b.clone())).unwrap();
        let change = confirm(
            &mut board,
            Completion::Opened(opened, HashMap::new(), true, Some("/b.moth".into())),
        );
        assert!(matches!(
            change,
            Change::Opened {
                recovered: true,
                ..
            }
        ));
        assert!(board.scene.get(a.id).is_none() && board.scene.get(b.id).is_some());
        assert!(board.dirty);
        assert!(matches!(
            confirm(&mut board, Completion::Saved),
            Change::Saved
        ));
        assert!(!board.dirty);
    }

    #[test]
    fn an_import_starts_from_an_empty_board_and_other_answers_pass_through() {
        let mut board = Fake::default();
        let old = note(0.);
        confirm(
            &mut board,
            Completion::Applied(Command::Insert(old.clone()), None),
        );
        let imported = note(50.);
        let change = confirm(
            &mut board,
            Completion::Imported(vec![Command::Insert(imported.clone())], HashMap::new(), 2),
        );
        assert!(matches!(change, Change::Imported { skipped: 2, .. }));
        assert!(board.scene.get(old.id).is_none());
        assert!(board.scene.get(imported.id).is_some() && board.dirty);
        let change = confirm(&mut board, Completion::Failed("disk full".into()));
        assert!(
            matches!(change, Change::Other(other) if matches!(&*other, Completion::Failed(message) if message == "disk full"))
        );
    }
}
