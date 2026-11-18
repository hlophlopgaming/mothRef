// SPDX-License-Identifier: GPL-3.0-or-later
//! Headless JSON-lines transport. Storage remains authoritative; replies follow journal sync.
pub(crate) mod cli;
mod operations;
mod preview;
use anyhow::{Context, Result, bail, ensure};
use moth_platform::{
    DesktopStorage,
    boards::{Completion, Request, Worker},
};
use moth_scene::{BlobId, Command, Item, Rect, Scene};
use operations::Operation;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    io::{BufRead, Read, Write},
    path::PathBuf,
    sync::Arc,
};

const MAX_REQUEST: u64 = 4 * 1024 * 1024;
const PLACEMENT_GAP: f32 = 24.;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    ops: Vec<Operation>,
    #[serde(default)]
    dry_run: bool,
}
struct Session {
    storage: Arc<DesktopStorage>,
    worker: Worker,
    scene: Scene,
    path: Option<PathBuf>,
    recovered: bool,
    dirty: bool,
}
impl Session {
    fn finish(self, success: bool, dry_run: bool) -> Result<()> {
        self.worker.finish()?;
        if success && (!self.dirty || dry_run) {
            self.storage.discard()?;
        }
        Ok(())
    }
    fn new() -> Result<Self> {
        let storage = Arc::new(match std::env::var_os("MOTHREF_AGENT_DATA") {
            Some(path) => DesktopStorage::in_directory(PathBuf::from(path)),
            None => DesktopStorage::default(),
        });
        let worker = Worker::new_tab(storage.clone(), Arc::new(|| {}))?;
        Ok(Self {
            storage,
            worker,
            scene: Scene::default(),
            path: None,
            recovered: false,
            dirty: false,
        })
    }
    fn request(&mut self, request: Request) -> Result<Value> {
        self.worker.requests.send(request)?;
        loop {
            match self.worker.results.recv()? {
                Completion::Saving(_) => {}
                Completion::Loading { .. } => {} // Opened carries the complete authoritative scene.
                Completion::Opened(scene, _, recovered, _) => {
                    self.scene = scene;
                    self.recovered = recovered;
                    return Ok(json!({"recovered": recovered}));
                }
                Completion::Imported(commands, _, skipped) => {
                    self.dirty = true;
                    for command in commands {
                        self.scene.apply(command)?;
                    }
                    return Ok(json!({"skipped": skipped}));
                }
                Completion::Applied(command, _) => {
                    self.dirty = true;
                    let id = command.id();
                    self.scene.apply(command)?;
                    return Ok(self.feedback(&[id]));
                }
                Completion::AppliedBatch(commands, _) => {
                    self.dirty = true;
                    let ids: Vec<_> = commands.iter().map(Command::id).collect();
                    for command in commands {
                        self.scene.apply(command)?;
                    }
                    return Ok(self.feedback(&ids));
                }
                Completion::Exported(message) => return Ok(json!({"message": message})),
                Completion::PixelInfo(info) => {
                    return Ok(
                        json!({"name":info.name,"pixel":info.pixel,"size":info.size,"format":info.format,"bytes":info.bytes,"frame":info.frame}),
                    );
                }
                Completion::Saved => {
                    self.dirty = false;
                    return Ok(json!({"saved": true}));
                }
                Completion::Failed(message) => bail!("{message}"),
                _ => bail!("unexpected storage completion"),
            }
        }
    }
    /// Measures an image's colors on the storage worker for a gamut card.
    fn measure_gamut(&mut self, image: moth_scene::Uuid) -> Result<moth_scene::Gamut> {
        self.worker
            .requests
            .send(Request::Export(moth_platform::export::Target::Gamut {
                image,
                card: None,
            }))?;
        loop {
            match self.worker.results.recv()? {
                Completion::Gamut(gamut, _) => return Ok(gamut),
                Completion::Failed(message) => bail!("{message}"),
                Completion::Saving(_) | Completion::Loading { .. } => {}
                _ => bail!("unexpected storage completion"),
            }
        }
    }
    fn apply(&mut self, commands: Vec<Command>) -> Result<Value> {
        // Validate the complete batch before the worker journals its first command.
        let mut scratch = Scene::default();
        for item in self.scene.items_in_insertion_order() {
            scratch.apply(Command::Insert(item.clone()))?;
        }
        for command in &commands {
            scratch.apply(command.clone())?;
        }
        self.request(Request::ApplyBatch(
            commands.into_iter().map(|c| (c, None)).collect(),
        ))
    }
    fn feedback(&self, ids: &[moth_scene::Uuid]) -> Value {
        let mut seen = std::collections::HashSet::new();
        let ids: Vec<_> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
        json!({"changed_total":ids.len(),"changed":ids.iter().filter_map(|id|self.scene.get(*id)).take(1000).map(item_json).collect::<Vec<_>>(),
            "bounds":self.scene.bounds().map(rect_json)})
    }
    fn next_anchor(&self) -> [f32; 2] {
        self.scene.bounds().map_or([0., 0.], |bounds| {
            let right = f64::from(bounds.x) + f64::from(bounds.width) + f64::from(PLACEMENT_GAP);
            let x = right as f32;
            [if f64::from(x) < right { x.next_up() } else { x }, bounds.y]
        })
    }
    fn info(&self, offset: usize, limit: usize) -> Value {
        let mut items: Vec<_> = self.scene.items().collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        let blobs: std::collections::HashSet<BlobId> =
            items.iter().filter_map(|i| i.blob()).collect();
        json!({"path": self.path, "items_total":items.len(), "blobs_total":blobs.len(), "recovered": self.recovered,
            "bounds":self.scene.bounds().map(rect_json),
            "coordinates":{"space":"board","x_positive":"right","y_positive":"down","rect":"[x,y,width,height]","angles":"clockwise radians","default_gap":PLACEMENT_GAP},
            "offset":offset,"items":items.into_iter().skip(offset).take(limit.min(1000)).map(item_json).collect::<Vec<_>>()})
    }
}
fn item_json(item: &Item) -> Value {
    let r = item.rect;
    let t = item.transform;
    let (kind, content) = match &item.content {
        moth_scene::Content::Image(blob) => ("image", json!({"blob":blob.to_hex().as_str()})),
        moth_scene::Content::Text(text) => ("note", json!({"text":text})),
        moth_scene::Content::Group(group) => {
            ("group", json!({"title":group.title,"color":group.color}))
        }
        moth_scene::Content::Gamut(gamut) => (
            "gamut",
            json!({"image":gamut.image.map(|id|id.to_string()),"view":gamut.view,"sort":gamut.sort,
                "colors":gamut.colors.iter().map(|([r,g,b],share)| json!({"hex":format!("#{r:02X}{g:02X}{b:02X}"),"share":f32::from(*share)/255.})).collect::<Vec<_>>()}),
        ),
    };
    let source = item.source.as_ref().map(|source| match source {
        moth_scene::Source::File(path) => json!({"file":path}),
        moth_scene::Source::Archive { archive, entry } => {
            json!({"archive":archive,"entry":entry})
        }
        moth_scene::Source::Url(url) => json!({"url":url}),
    });
    json!({"id":item.id.to_string(),"name":item.name,"kind":kind,"content":content,"source":source,
        "parent":item.parent.map(|id|id.to_string()),"rect":[r.x,r.y,r.width,r.height],
        "bounds":rect_json(moth_scene::footprint(item)),"center":[r.x+r.width/2.,r.y+r.height/2.],
        "transform":{"rotation":t.rotation,"opacity":t.opacity,"grayscale":t.grayscale,"sampling":t.sampling as u8,
        "note_font_size":t.note_font_size,"note_wrap":t.note_wrap,"flip":t.flip,"content_offset":t.content_offset,
        "content_scale":t.content_scale,"content_rotation":t.content_rotation}})
}
fn rect_json(rect: Rect) -> [f32; 4] {
    [rect.x, rect.y, rect.width, rect.height]
}
fn rect([x, y, width, height]: [f32; 4]) -> Result<Rect> {
    let rect = Rect {
        x,
        y,
        width,
        height,
    };
    ensure!(rect.valid(), "invalid rectangle");
    Ok(rect)
}
/// Runs without a display. A failed request ends the session, retaining its recovery journal.
pub(crate) fn run() -> Result<()> {
    let mut session = Session::new()?;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut output = std::io::stdout().lock();
    let mut mode = None;
    let mut failed = false;
    loop {
        let mut line = Vec::new();
        let count = input
            .by_ref()
            .take(MAX_REQUEST + 1)
            .read_until(b'\n', &mut line)?;
        if count == 0 {
            break;
        }
        let result = (|| -> Result<Value> {
            ensure!(count as u64 <= MAX_REQUEST, "request exceeds 4 MiB");
            let request: Input = serde_json::from_slice(&line).context("invalid agent request")?;
            ensure!(
                mode.is_none_or(|dry_run| dry_run == request.dry_run),
                "dry-run mode cannot change during a session"
            );
            mode = Some(request.dry_run);
            ensure!(
                request.ops.len() <= 1000,
                "at most 1000 operations per request"
            );
            let mut results = Vec::new();
            for op in request.ops {
                results.push(session.execute(op, request.dry_run)?);
            }
            Ok(json!({"protocol":1,"ok":true,"results":results,"dry_run":request.dry_run}))
        })();
        failed = result.is_err();
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => {
                json!({"protocol":1,"ok":false,"error":format!("{error:#}"),"recovery":"Earlier acknowledged operations remain in the private recovery journal; no automatic save follows an error."})
            }
        };
        serde_json::to_writer(&mut output, &reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
        if failed {
            break;
        }
    }
    session.finish(!failed, mode == Some(true))?;
    ensure!(!failed, "agent request failed; see JSON reply");
    Ok(())
}
