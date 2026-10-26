// SPDX-License-Identifier: GPL-3.0-or-later
use moth_format::board;
use moth_scene::{Command, Item, Rect};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, Read, Write},
    path::Path,
    process::{Command as Process, Stdio},
    time::Duration,
};
fn item() -> Item {
    Item::image(
        moth_scene::BlobId::from_bytes(*blake3_hash()),
        Rect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        },
        None,
    )
}
fn blake3_hash() -> &'static [u8; 32] {
    // Use the same original hash as the format fixture without another dependency.
    static HASH: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let image = moth_io::decode(include_bytes!("../../corpus/images/rows.png"), 256)
            .expect("checked-in PNG fixture must decode");
        *image.hash.as_bytes()
    })
}
fn append(file: &mut File, command: &Command) -> anyhow::Result<()> {
    board::append(file, file.metadata()?.len(), 2, &board::encode(command))?;
    file.sync_all()?;
    Ok(())
}
pub fn writer(path: &str) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create_new(true)
        .open(path)?;
    board::initialize(&mut file)?;
    let bytes = include_bytes!("../../corpus/images/rows.png");
    board::append(&mut file, 16, 1, bytes)?;
    let item = item();
    append(&mut file, &Command::Insert(item.clone()))?;
    let mut output = std::io::stdout().lock();
    for index in 1u32.. {
        let command = Command::SetRect {
            id: item.id,
            rect: Rect {
                x: index as f32,
                ..item.rect
            },
        };
        // Split the record so random kills exercise writes before the commit footer.
        let mut record = Vec::new();
        board::append(
            &mut record,
            file.metadata()?.len(),
            2,
            &board::encode(&command),
        )?;
        file.write_all(&record[..record.len() / 2])?;
        std::thread::sleep(Duration::from_micros(100));
        file.write_all(&record[record.len() / 2..])?;
        file.sync_all()?;
        writeln!(output, "{index}")?;
        output.flush()?;
        std::thread::sleep(Duration::from_micros(100));
    }
    Ok(())
}
pub fn killtest(count: usize) -> anyhow::Result<()> {
    killtest_impl(count, false)
}
/// Kills the real storage worker while importing distinct originals.
pub fn load_killtest(count: usize) -> anyhow::Result<()> {
    killtest_impl(count, true)
}
pub fn load_writer(path: &str) -> anyhow::Result<()> {
    use moth_platform::boards::{Completion, Request, Worker};
    let root = Path::new(path);
    std::fs::create_dir_all(root)?;
    let storage = std::sync::Arc::new(moth_platform::DesktopStorage::in_directory(
        root.join("recovery"),
    ));
    let worker = Worker::new_tab(storage.clone(), std::sync::Arc::new(|| {}))?;
    let source = root.join("input.png");
    let handle = storage.register(source.clone())?;
    let mut output = std::io::stdout().lock();
    for index in 1u32.. {
        let mut bytes = include_bytes!("../../corpus/images/rows.png").to_vec();
        bytes.extend_from_slice(&index.to_le_bytes());
        std::fs::write(&source, &bytes)?;
        let image = moth_io::decode(&bytes, 256)?;
        let mut item = item();
        item.content = moth_scene::Content::Image(image.hash);
        item.rect.x = index as f32;
        worker
            .requests
            .send(Request::Apply(Command::Insert(item), Some(handle)))?;
        anyhow::ensure!(
            matches!(
                worker.results.recv_timeout(Duration::from_secs(30))?,
                Completion::Applied(_, _)
            ),
            "insert failed"
        );
        writeln!(output, "{index}")?;
        output.flush()?;
    }
    Ok(())
}
fn killtest_impl(count: usize, loading: bool) -> anyhow::Result<()> {
    anyhow::ensure!(count > 0, "killtest needs at least one iteration");
    let directory =
        std::env::temp_dir().join(format!("mothref-killtest-{}", moth_scene::Uuid::now_v7()));
    std::fs::create_dir(&directory)?;
    let mut seed = 42u64;
    for iteration in 0..count {
        let path = directory.join("board.moth");
        let mut child = Process::new(std::env::current_exe()?)
            .arg(if loading {
                "load-writer"
            } else {
                "kill-writer"
            })
            .arg(&path)
            .stdout(Stdio::piped())
            .spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("child stdout unavailable"))?;
        let mut lines = std::io::BufReader::new(stdout).lines();
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let target = seed % 8 + 1;
        let mut acknowledged = 0;
        for _ in 0..target {
            acknowledged = lines
                .next()
                .ok_or_else(|| anyhow::anyhow!("writer exited before acknowledgment"))??
                .parse::<u32>()?;
        }
        std::thread::sleep(Duration::from_micros((seed >> 32) % 400));
        child.kill()?;
        child.wait()?;
        // Include all acknowledgments already written to the pipe before the kill.
        for line in lines {
            acknowledged = line?.parse()?;
        }
        let journal = if loading {
            let entry = std::fs::read_dir(path.join("recovery"))?
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing recovery directory"))??;
            entry.path().join("board.moth")
        } else {
            path.clone()
        };
        let loaded = board::read(&mut File::open(journal)?)?;
        let recovered = if loading {
            anyhow::ensure!(
                loaded.scene.items().all(|item| item
                    .blob()
                    .is_some_and(|blob| loaded.blobs.contains_key(&blob))),
                "recovered image lost original"
            );
            loaded.scene.len() as u32
        } else {
            loaded
                .scene
                .items()
                .next()
                .ok_or_else(|| anyhow::anyhow!("lost original item"))?
                .rect
                .x as u32
        };
        anyhow::ensure!(
            recovered >= acknowledged,
            "iteration {iteration}: lost acknowledged command {acknowledged}, recovered {recovered}"
        );
        if loading {
            for index in 1..=acknowledged {
                anyhow::ensure!(
                    loaded.scene.items().any(|item| item.rect.x == index as f32),
                    "iteration {iteration}: lost acknowledged image {index}"
                );
            }
        }
        if loading {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    std::fs::remove_dir(directory)?;
    println!("{count} process kills: no acknowledged commands lost");
    Ok(())
}
pub fn fixture() -> anyhow::Result<()> {
    std::fs::create_dir_all("corpus/moth")?;
    // corpus/moth/v1.moth through v4.moth are frozen migration fixtures;
    // regenerating them with the current writer would erase that coverage.
    let mut bytes = Vec::new();
    board::initialize(&mut bytes)?;
    let original = include_bytes!("../../corpus/images/rows.png");
    board::append(&mut bytes, 16, 1, original)?;
    let mut scene = moth_scene::Scene::default();
    let mut item = item();
    item.transform = moth_scene::Transform {
        rotation: 0.75,
        flip: [true, false],
        content_offset: [-0.25, 1.5],
        content_scale: 2.5,
        content_rotation: -0.5,
        opacity: 0.4,
        grayscale: true,
        sampling: moth_scene::Sampling::Nearest,
        ..Default::default()
    };
    item.name = "rows.png".into();
    scene.apply(Command::Insert(item))?;
    let mut note = Item::text(
        "a note that wraps to a fixed frame width",
        Rect {
            x: 10.,
            y: 20.,
            width: 120.,
            height: 40.,
        },
        None,
    );
    note.transform.note_font_size = 24.;
    note.transform.note_wrap = true;
    scene.apply(Command::Insert(note))?;
    let offset = bytes.len() as u64;
    board::append(&mut bytes, offset, 3, &board::snapshot(&scene)?)?;
    std::fs::write("corpus/moth/v5.moth", bytes)?;
    Ok(())
}
pub fn roundtrip(path: &Path) -> anyhow::Result<()> {
    let mut file = File::open(path)?;
    let before = board::read(&mut file)?;
    let mut bytes = Vec::new();
    board::initialize(&mut bytes)?;
    for blob in before.blobs.values() {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(blob.offset))?;
        let mut original = vec![0; blob.length as usize];
        file.read_exact(&mut original)?;
        let offset = bytes.len() as u64;
        board::append(&mut bytes, offset, 1, &original)?;
    }
    let offset = bytes.len() as u64;
    board::append(&mut bytes, offset, 3, &board::snapshot(&before.scene)?)?;
    let after = board::read(&mut std::io::Cursor::new(bytes))?;
    anyhow::ensure!(
        before.scene.len() == after.scene.len(),
        "round-trip count differs"
    );
    for item in before.scene.items() {
        anyhow::ensure!(
            after.scene.get(item.id) == Some(item),
            "round-trip metadata differs"
        );
    }
    Ok(())
}

/// Exercises bounded history, durable commands, backups and compaction during RSS sampling.
pub struct Soak {
    worker: moth_platform::boards::Worker,
    storage: std::sync::Arc<moth_platform::DesktopStorage>,
    item: Item,
    directory: std::path::PathBuf,
}
impl Soak {
    pub fn new() -> anyhow::Result<Self> {
        let directory =
            std::env::temp_dir().join(format!("mothref-soak-{}", moth_scene::Uuid::now_v7()));
        let storage = std::sync::Arc::new(moth_platform::DesktopStorage::in_directory(
            directory.join("recovery"),
        ));
        let (handle, image) = storage.stage_preview(1, 1, &[1, 2, 3, 255])?;
        let item = Item::image(
            image.hash,
            Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        let worker =
            moth_platform::boards::Worker::new(storage.clone(), std::sync::Arc::new(|| {}))?;
        std::fs::create_dir_all(&directory)?;
        let result = Self {
            worker,
            storage,
            item,
            directory,
        };
        result.send(moth_platform::boards::Request::Apply(
            Command::Insert(result.item.clone()),
            Some(handle),
        ))?;
        Ok(result)
    }
    fn send(&self, request: moth_platform::boards::Request) -> anyhow::Result<()> {
        self.worker.requests.send(request)?;
        loop {
            use moth_platform::boards::Completion;
            match self.worker.results.recv_timeout(Duration::from_secs(30))? {
                Completion::Loading { .. }
                | Completion::Recovery(_)
                | Completion::RecoveryDiscarded(_)
                | Completion::Recent(_)
                | Completion::Settings(_)
                | Completion::Preferences(_)
                | Completion::SettingsSaved => continue,
                Completion::Failed(error) => anyhow::bail!(error),
                Completion::Applied(_, _) | Completion::AppliedBatch(_, _) | Completion::Saved => {
                    break;
                }
                Completion::Opened(_, _, _, _) => anyhow::bail!("unexpected open completion"),
                Completion::Exported(_) => anyhow::bail!("unexpected export completion"),
                Completion::Imported(_, _, _) => anyhow::bail!("unexpected import completion"),
            }
        }
        Ok(())
    }
    pub fn step(&self, frame: usize) -> anyhow::Result<()> {
        use moth_platform::boards::Request;
        self.send(Request::Apply(
            Command::SetRect {
                id: self.item.id,
                rect: Rect {
                    x: (frame % 100) as f32,
                    ..self.item.rect
                },
            },
            None,
        ))?;
        if frame.is_multiple_of(4) {
            self.send(Request::Undo)?;
            self.send(Request::Redo)?;
        }
        if frame.is_multiple_of(100) {
            self.send(Request::Save(self.directory.join("board.moth")))?;
        }
        Ok(())
    }
    pub fn finish(self) -> anyhow::Result<()> {
        self.send(moth_platform::boards::Request::Save(
            self.directory.join("board.moth"),
        ))?;
        roundtrip(&self.directory.join("board.moth"))?;
        self.worker.finish()?;
        self.storage.discard()?;
        std::fs::remove_dir_all(self.directory)?;
        Ok(())
    }
}

/// Foreign imports must survive conversion into MothRef's durable representation.
pub fn roundtrip_pur(path: &Path) -> anyhow::Result<()> {
    use moth_platform::boards::{Completion, Request, Worker};
    let directory =
        std::env::temp_dir().join(format!("mothref-pur-{}", moth_scene::Uuid::now_v7()));
    let storage = std::sync::Arc::new(moth_platform::DesktopStorage::in_directory(
        directory.join("recovery"),
    ));
    let worker = Worker::new_tab(storage, std::sync::Arc::new(|| {}))?;
    let result = (|| -> anyhow::Result<()> {
        let started = std::time::Instant::now();
        worker.requests.send(Request::ImportPur(path.to_owned()))?;
        let (commands, sources, skipped) = loop {
            match worker.results.recv_timeout(Duration::from_secs(300))? {
                Completion::Loading { .. } => continue,
                Completion::Imported(commands, sources, skipped) => {
                    break (commands, sources, skipped);
                }
                Completion::Failed(error) => anyhow::bail!(error),
                _ => anyhow::bail!("unexpected import completion"),
            }
        };
        let elapsed = started.elapsed();
        let mut scene = moth_scene::Scene::default();
        for command in commands {
            scene.apply(command)?;
        }
        let images = scene.items().filter(|item| item.blob().is_some()).count();
        let notes = scene.len() - images;
        println!(
            "{}: {images} images, {notes} notes, {skipped} skipped; durable import {:.3} s",
            path.display(),
            elapsed.as_secs_f64()
        );
        anyhow::ensure!(!scene.is_empty(), "empty PureRef corpus import");
        anyhow::ensure!(
            skipped == 0,
            "PureRef qualification skipped {skipped} items"
        );
        let expected: Vec<_> = scene.items().cloned().collect();
        let same_scene = |scene: &moth_scene::Scene| {
            expected.len() == scene.len()
                && expected.iter().all(|item| scene.get(item.id) == Some(item))
        };
        let destination = directory.join("converted.moth");
        worker.requests.send(Request::Save(destination.clone()))?;
        match worker.results.recv_timeout(Duration::from_secs(300))? {
            Completion::Saved => {}
            Completion::Failed(error) => anyhow::bail!(error),
            _ => anyhow::bail!("unexpected save completion"),
        }
        let after = board::read(&mut File::open(destination)?)?;
        anyhow::ensure!(
            same_scene(&after.scene),
            "PureRef conversion round-trip differs"
        );
        anyhow::ensure!(
            sources.len() == after.blobs.len()
                && sources.keys().all(|hash| after.blobs.contains_key(hash)),
            "PureRef original blob identities differ"
        );
        for (request, empty) in [(Request::Undo, true), (Request::Redo, false)] {
            worker.requests.send(request)?;
            let commands = match worker.results.recv_timeout(Duration::from_secs(300))? {
                Completion::AppliedBatch(commands, _) => commands,
                Completion::Failed(error) => anyhow::bail!(error),
                _ => anyhow::bail!("expected one grouped undo/redo completion"),
            };
            for command in commands {
                scene.apply(command)?;
            }
            anyhow::ensure!(
                scene.is_empty() == empty,
                "PureRef import did not undo/redo as one step"
            );
        }
        anyhow::ensure!(same_scene(&scene), "PureRef redo changed metadata");
        println!(
            "verified .moth round-trip, {} original blobs and grouped undo/redo",
            sources.len()
        );
        Ok(())
    })();
    let finished = worker.finish();
    let cleanup = std::fs::remove_dir_all(directory);
    result?;
    finished?;
    cleanup?;
    Ok(())
}

#[cfg(test)]
mod import_tests {
    #[test]
    fn pur_qualification_covers_both_captured_versions() {
        for name in ["v1.10-redbar.pur", "v2.1-redbar.pur"] {
            super::roundtrip_pur(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../corpus/pur")
                    .join(name),
            )
            .unwrap();
        }
    }
}
