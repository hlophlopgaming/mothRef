// SPDX-License-Identifier: GPL-3.0-or-later
//! Agent operations reuse the GUI geometry helpers and the durable storage worker.
use super::*;
use crate::app::{arrange, groups};
use moth_scene::{FractionalIndex, Sampling, Uuid};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize, clap::Subcommand)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Operation {
    #[command(about = "List operations, GUI coverage and the embedded agent guide")]
    Commands,
    #[command(about = "Inspect native protocol and current registered GUI actions.")]
    Capabilities,
    #[command(about = "Capture a native PNG preview bundle with pixel/board transforms")]
    Preview {
        path: PathBuf,
        #[arg(long, default_value_t = 1024)]
        #[serde(default = "preview_side")]
        side: u32,
    },
    #[command(about = "Inspect the latest completed preview without rendering")]
    PreviewLatest { path: PathBuf },
    #[command(about = "Sample a native bounded image preview, without using the clipboard.")]
    Sample {
        #[arg(long, allow_hyphen_values = true)]
        id: String,
        #[arg(long, value_parser = super::cli::pair, allow_hyphen_values = true)]
        point: [f32; 2],
    },
    #[command(about = "Create a new native board; existing files are refused.")]
    Create { path: PathBuf },
    #[command(about = "Open a moth/bee/pur board in this session.")]
    Open { path: PathBuf },
    #[command(about = "Checkpoint through the native storage worker.")]
    Save { path: Option<PathBuf> },
    #[command(
        about = "Inspect z-sorted items, frame/rotated bounds and centers, global board bounds, coordinate conventions, blobs and recovery state."
    )]
    Info {
        #[serde(default)]
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[serde(default = "page_size")]
        #[arg(long, default_value_t = 1000)]
        limit: usize,
    },
    #[command(about = "Validate the board through the native reader; recovered tails fail.")]
    Check,
    #[command(about = "Add a local image or explicitly download an HTTP(S) image.")]
    Add {
        #[arg(required_unless_present = "url", conflicts_with = "url")]
        path: Option<PathBuf>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, value_parser = super::cli::pair, allow_hyphen_values = true)]
        at: Option<[f32; 2]>,
    },
    #[command(about = "Add a text note with estimated native font bounds.")]
    Note {
        text: String,
        #[arg(long, value_parser = super::cli::pair, allow_hyphen_values = true)]
        at: Option<[f32; 2]>,
    },
    #[command(about = "Edit a frame, note, label or content transform.")]
    Edit {
        #[arg(long, allow_hyphen_values = true)]
        id: String,
        #[arg(long, value_parser = super::cli::rectangle, allow_hyphen_values = true)]
        rect: Option<[f32; 4]>,
        #[arg(long, allow_hyphen_values = true)]
        text: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        name: Option<String>,
        #[command(flatten)]
        transform: Option<Patch>,
    },
    #[command(about = "Arrange images and notes with the GUI layout algorithms.")]
    #[command(name = "arrange", alias = "layout")]
    Layout {
        #[arg(long, default_value = "grid", value_parser = ["horizontal", "vertical", "grid", "optimal"])]
        mode: String,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
        #[serde(default)]
        #[arg(long, default_value_t = 0.0)]
        gap: f32,
        #[arg(long, allow_hyphen_values = true)]
        order: Option<String>,
        #[arg(long, allow_hyphen_values = true)]
        count: Option<usize>,
    },
    #[command(about = "Align visible envelopes, distribute or stack.")]
    Align {
        #[arg(long, value_parser = ["left", "right", "top", "bottom", "horizontal", "vertical", "stack"])]
        mode: String,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Match the first z-sorted item, preserving aspect except size.")]
    Normalize {
        #[arg(long, value_parser = ["width", "height", "size", "area"])]
        mode: String,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Scale size and positions about the first item.")]
    Scale {
        #[arg(long, allow_hyphen_values = true)]
        factor: f32,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Move frames, including members of selected groups.")]
    Move {
        #[arg(long = "by", alias = "delta", value_parser = super::cli::pair, allow_hyphen_values = true)]
        delta: [f32; 2],
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Copy items and remap copied group memberships.")]
    Duplicate {
        #[arg(long = "by", alias = "delta", value_parser = super::cli::pair, allow_hyphen_values = true)]
        delta: Option<[f32; 2]>,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Delete explicitly selected subtrees, children first.")]
    Delete {
        #[arg(long = "id", value_delimiter = ',', num_args = 1.., required = true)]
        ids: Vec<String>,
    },
    #[command(about = "Group explicitly selected items.")]
    Group {
        #[arg(long = "id", value_delimiter = ',', num_args = 1.., required = true)]
        ids: Vec<String>,
        #[serde(default)]
        #[arg(long, default_value = "")]
        title: String,
        #[serde(default)]
        #[arg(long, default_value_t = 0, value_parser = super::cli::color)]
        color: u32,
    },
    #[command(about = "Dissolve selected top-level groups and preserve members.")]
    Ungroup {
        #[arg(long = "id", value_delimiter = ',', num_args = 1.., required = true)]
        ids: Vec<String>,
    },
    #[command(about = "Rename/recolor a group.")]
    GroupEdit {
        #[arg(long, allow_hyphen_values = true)]
        id: String,
        #[arg(long)]
        title: String,
        #[arg(long, value_parser = super::cli::color)]
        color: u32,
    },
    #[command(about = "Reparent an item; cycles and excessive nesting are refused.")]
    Parent {
        #[arg(long, allow_hyphen_values = true)]
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        parent: Option<String>,
    },
    #[command(about = "Change image/note z-order; group backdrop order is derived.")]
    Order {
        #[arg(long, value_parser = ["raise", "lower", "top", "bottom"])]
        mode: String,
        #[arg(long = "id", value_delimiter = ',', num_args = 1.., required = true)]
        ids: Vec<String>,
    },
    #[command(about = "Reset original-backed geometry or match original pixel scale.")]
    Reset {
        #[arg(long, value_parser = ["scale", "normalize", "crop", "all"])]
        mode: String,
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
    },
    #[command(about = "Undo within a JSON-lines session or batch; history ends with the process.")]
    Undo,
    #[command(about = "Redo within a JSON-lines session or batch; history ends with the process.")]
    Redo,
    #[command(about = "Export using the real native renderer; destination must be new.")]
    Export {
        #[arg(long, default_value = "scene")]
        kind: String,
        path: PathBuf,
        #[serde(default)]
        #[arg(long = "id", value_delimiter = ',', num_args = 1..)]
        ids: Vec<String>,
        #[serde(default = "export_side")]
        #[arg(long, default_value_t = 2048)]
        side: u32,
        #[arg(long, value_parser = super::cli::rectangle, allow_hyphen_values = true)]
        region: Option<[f32; 4]>,
    },
    #[command(about = "Read original pixel coordinates and image metadata.")]
    PixelInfo {
        #[arg(long, allow_hyphen_values = true)]
        id: String,
        #[arg(long, value_parser = super::cli::pair, allow_hyphen_values = true)]
        point: [f32; 2],
    },
}
fn preview_side() -> u32 {
    1024
}
fn page_size() -> usize {
    1000
}
fn export_side() -> u32 {
    2048
}
#[derive(Clone, Debug, Deserialize, clap::Args)]
#[serde(deny_unknown_fields)]
pub(super) struct Patch {
    #[arg(long, allow_hyphen_values = true)]
    rotation: Option<f32>,
    #[arg(long, allow_hyphen_values = true)]
    opacity: Option<f32>,
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    grayscale: Option<bool>,
    #[arg(long, allow_hyphen_values = true)]
    sampling: Option<u8>,
    #[arg(long, allow_hyphen_values = true)]
    note_font_size: Option<f32>,
    #[arg(long, action = clap::ArgAction::Set, num_args = 0..=1, default_missing_value = "true")]
    note_wrap: Option<bool>,
    #[arg(long, value_parser = super::cli::flip)]
    flip: Option<[bool; 2]>,
    #[arg(long, value_parser = super::cli::pair, allow_hyphen_values = true)]
    content_offset: Option<[f32; 2]>,
    #[arg(long, allow_hyphen_values = true)]
    content_scale: Option<f32>,
    #[arg(long, allow_hyphen_values = true)]
    content_rotation: Option<f32>,
}
fn id(value: &str) -> Result<Uuid> {
    Uuid::parse_str(value).context("expected item UUID")
}
impl Operation {
    pub(super) fn mutates(&self) -> bool {
        use Operation::*;
        match self {
            Commands
            | Capabilities
            | Preview { .. }
            | PreviewLatest { .. }
            | Sample { .. }
            | Open { .. }
            | Info { .. }
            | Check
            | Export { .. }
            | PixelInfo { .. } => false,
            Create { .. }
            | Save { .. }
            | Add { .. }
            | Note { .. }
            | Edit { .. }
            | Layout { .. }
            | Align { .. }
            | Normalize { .. }
            | Scale { .. }
            | Move { .. }
            | Duplicate { .. }
            | Delete { .. }
            | Group { .. }
            | Ungroup { .. }
            | GroupEdit { .. }
            | Parent { .. }
            | Order { .. }
            | Reset { .. }
            | Undo
            | Redo => true,
        }
    }
}
impl Session {
    fn selected(&self, ids: &[String], members: bool) -> Result<Vec<&Item>> {
        let mut selected: HashSet<_> = ids.iter().map(|v| id(v)).collect::<Result<_>>()?;
        for id in &selected {
            ensure!(self.scene.get(*id).is_some(), "item {id} does not exist");
        }
        if ids.is_empty() {
            selected.extend(self.scene.items().map(|i| i.id));
        }
        if members {
            for group in selected.clone() {
                selected.extend(self.scene.descendants(group));
            }
        }
        let mut items: Vec<_> = selected
            .into_iter()
            .filter_map(|id| self.scene.get(id))
            .collect();
        items.sort_by(|a, b| a.z.cmp(&b.z).then(a.id.cmp(&b.id)));
        Ok(items)
    }
    pub(super) fn execute(&mut self, op: Operation, dry_run: bool) -> Result<Value> {
        use Operation::*;
        match op {
            Commands => Ok(
                json!({"protocol":1,"operations":serde_json::from_str::<Value>(include_str!("api.json"))?,"gui_actions":serde_json::from_str::<Value>(include_str!("actions.json"))?,"command_names":super::cli::command_names(),"agent_guide":include_str!("../../../../../skills/cli-anything-mothref/SKILL.md")}),
            ),
            Preview { path, side } => super::preview::capture(self, path, side, dry_run),
            PreviewLatest { path } => super::preview::latest(path),
            Capabilities => Ok(
                json!({"protocol":1,"gui_actions":crate::app::bindings::ACTIONS.iter().map(|a|a.id()).collect::<Vec<_>>()}),
            ),
            Sample {
                id: identity,
                point,
            } => self.request(Request::Export(
                moth_platform::export::Target::InspectSample {
                    id: id(&identity)?,
                    point,
                },
            )),
            Create { path } => {
                ensure!(
                    self.path.is_none() && self.scene.is_empty(),
                    "session already has a board"
                );
                ensure!(!path.exists(), "destination already exists");
                ensure!(
                    path.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("moth")),
                    "use a .moth destination"
                );
                self.path = Some(path);
                Ok(self.info(0, 1000))
            }
            Open { path } => {
                ensure!(
                    self.path.is_none() && self.scene.is_empty(),
                    "session already has a board"
                );
                let extension = path
                    .extension()
                    .and_then(|v| v.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let request = match extension.as_str() {
                    "moth" => Request::Open(path.clone()),
                    "bee" => Request::ImportBee(path.clone()),
                    "pur" => Request::ImportPur(path.clone()),
                    _ => bail!("supported boards: .moth, .bee, .pur"),
                };
                let result = self.request(request)?;
                if extension == "moth" {
                    self.path = Some(path);
                }
                Ok(json!({"import":result,"board":self.info(0,1000)}))
            }
            Save { path } => {
                let path = path
                    .or_else(|| self.path.clone())
                    .context("save needs a .moth path")?;
                if dry_run {
                    return Ok(json!({"would_save":path}));
                }
                let fresh = !path.try_exists()?;
                let result = self.request(Request::Save(path.clone()))?;
                if fresh {
                    self.recovered = false;
                }
                self.path = Some(path);
                Ok(result)
            }
            Info { offset, limit } => Ok(self.info(offset, limit)),
            Check => {
                // Opening verifies chunks and original hashes through the native storage path.
                ensure!(
                    !self.recovered,
                    "board has a torn/corrupt tail; opened with recovery"
                );
                Ok(json!({"valid":true,"board":self.info(0,0)}))
            }
            Add { path, url, at } => {
                let at = at.unwrap_or_else(|| self.next_anchor());
                let (hash, size, handle, name) = match (path, url) {
                    (Some(path), None) => {
                        let mut bytes = Vec::new();
                        std::fs::File::open(&path)?
                            .take(moth_io::MAX_INPUT_BYTES + 1)
                            .read_to_end(&mut bytes)?;
                        ensure!(
                            bytes.len() as u64 <= moth_io::MAX_INPUT_BYTES,
                            "image exceeds record budget"
                        );
                        let size = moth_io::tiles::dimensions(&bytes)?;
                        let hash = moth_io::content_hash(&bytes);
                        let name = path
                            .file_name()
                            .context("image filename missing")?
                            .to_string_lossy()
                            .into_owned();
                        (hash, size, self.storage.register(path)?, name)
                    }
                    (None, Some(url)) => {
                        let (handle, decoded) = self.storage.import_url(&url)?;
                        (
                            decoded.hash,
                            decoded.original,
                            handle,
                            "downloaded-reference".to_owned(),
                        )
                    }
                    _ => bail!("add requires exactly one local path or URL"),
                };
                let mut item = Item::image(
                    hash,
                    rect([at[0], at[1], size[0] as f32, size[1] as f32])?,
                    self.top().as_ref(),
                );
                item.name = name;
                let identity = item.id.to_string();
                let geometry = item_json(&item);
                self.request(Request::Apply(Command::Insert(item), Some(handle)))?;
                Ok(json!({"id":identity,"item":geometry}))
            }
            Note { text, at } => {
                let at = at.unwrap_or_else(|| self.next_anchor());
                let size = moth_scene::note_size(&text);
                let item = Item::text(
                    text,
                    rect([at[0], at[1], size[0], size[1]])?,
                    self.top().as_ref(),
                );
                let identity = item.id.to_string();
                let geometry = item_json(&item);
                self.apply(vec![Command::Insert(item)])?;
                Ok(json!({"id":identity,"item":geometry}))
            }
            Edit {
                id: identity,
                rect: bounds,
                text,
                name,
                transform,
            } => {
                let identity = id(&identity)?;
                let item = self.scene.get(identity).context("item does not exist")?;
                let mut commands = Vec::new();
                if let Some(name) = name {
                    ensure!(!item.is_group(), "rename groups with group-edit");
                    let mut changed = item.clone();
                    changed.name = name;
                    commands.extend([Command::Remove(identity), Command::Insert(changed)]);
                }
                if let Some(bounds) = bounds {
                    commands.push(Command::SetRect {
                        id: identity,
                        rect: rect(bounds)?,
                    });
                }
                if let Some(text) = text {
                    commands.push(Command::SetText { id: identity, text });
                }
                if let Some(p) = transform {
                    let mut t = item.transform;
                    macro_rules! patch { ($($field:ident),*) => { $(if let Some(value) = p.$field { t.$field = value; })* }; }
                    patch!(
                        rotation,
                        opacity,
                        grayscale,
                        note_font_size,
                        note_wrap,
                        flip,
                        content_offset,
                        content_scale,
                        content_rotation
                    );
                    if let Some(code) = p.sampling {
                        t.sampling = Sampling::from_code(code).context("sampling must be 0..3")?;
                    }
                    ensure!(t.valid(), "invalid transform");
                    commands.push(Command::SetTransform {
                        id: identity,
                        transform: t,
                    });
                }
                self.apply(commands)
            }
            Layout {
                mode,
                ids,
                gap,
                order,
                count,
            } => {
                ensure!(
                    gap.is_finite() && (0. ..=10000.).contains(&gap),
                    "gap must be 0..10000"
                );
                let mut items = self.selected(&ids, true)?;
                items.retain(|i| !i.is_group());
                match order.as_deref().unwrap_or("z") {
                    "z" => {}
                    "name" => items.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id))),
                    "time" => items.sort_by_key(|i| i.id),
                    _ => bail!("order must be z, name or time; source paths are not stored"),
                }
                let mode = match mode.as_str() {
                    "horizontal" => arrange::Arrange::Horizontal,
                    "vertical" => arrange::Arrange::Vertical,
                    "grid" => arrange::Arrange::Grid,
                    "optimal" => arrange::Arrange::Optimal,
                    _ => bail!("layout must be horizontal, vertical, grid or optimal"),
                };
                let commands = if let Some(count) = count {
                    ensure!(
                        count > 0
                            && matches!(
                                mode,
                                arrange::Arrange::Horizontal | arrange::Arrange::Vertical
                            ),
                        "count needs horizontal/vertical and must be positive"
                    );
                    arrange::counted(&items, mode, count, gap)
                } else {
                    arrange::arrange(
                        &items,
                        mode,
                        gap,
                        arrange::FreeRectChoice::BestShortSide,
                        arrange::GuillotineSplit::ShorterLeftoverAxis,
                    )
                };
                ensure!(
                    items.is_empty() || !commands.is_empty(),
                    "layout exceeds geometry limits"
                );
                self.apply(commands)
            }
            Align { mode, ids } => {
                let mode = match mode.as_str() {
                    "left" => arrange::Align::Left,
                    "right" => arrange::Align::Right,
                    "top" => arrange::Align::Top,
                    "bottom" => arrange::Align::Bottom,
                    "horizontal" => arrange::Align::Horizontal,
                    "vertical" => arrange::Align::Vertical,
                    "stack" => arrange::Align::Stack,
                    _ => bail!("unknown alignment"),
                };
                let items: Vec<_> = self
                    .selected(&ids, true)?
                    .into_iter()
                    .filter(|i| !i.is_group())
                    .collect();
                self.apply(arrange::align(&items, mode))
            }
            Normalize { mode, ids } => {
                let items: Vec<_> = self
                    .selected(&ids, true)?
                    .into_iter()
                    .filter(|i| !i.is_group())
                    .collect();
                let mode = match mode.as_str() {
                    "width" => arrange::Normalize::Width,
                    "height" => arrange::Normalize::Height,
                    "size" => arrange::Normalize::Size,
                    "area" => arrange::Normalize::Area,
                    _ => bail!(
                        "normalize must be width, height, size or area; use reset normalize for pixel scale"
                    ),
                };
                self.apply(arrange::normalize(&items, mode))
            }
            Scale { factor, ids } => {
                ensure!(
                    factor.is_finite() && factor > 0.,
                    "scale factor must be positive"
                );
                let items: Vec<_> = self
                    .selected(&ids, true)?
                    .into_iter()
                    .filter(|i| !i.is_group())
                    .collect();
                let commands = arrange::scale(&items, factor);
                ensure!(
                    items.is_empty() || !commands.is_empty(),
                    "scale exceeds geometry limits"
                );
                self.apply(commands)
            }
            Move { delta, ids } => self.translate(&ids, Some(delta), false),
            Duplicate { delta, ids } => self.translate(&ids, delta, true),
            Delete { ids } => {
                ensure!(!ids.is_empty(), "delete requires explicit ids");
                let mut items = self.selected(&ids, true)?;
                items.sort_by_key(|i| std::cmp::Reverse(self.scene.ancestors(i.id).count()));
                self.apply(items.iter().map(|i| Command::Remove(i.id)).collect())
            }
            Group { ids, title, color } => {
                ensure!(!ids.is_empty(), "group requires explicit ids");
                let selected = self.selected(&ids, true)?.iter().map(|i| i.id).collect();
                let (identity, mut commands) = groups::create(&self.scene, &selected)
                    .context("cannot group this selection")?;
                commands.push(Command::SetGroup {
                    id: identity,
                    group: moth_scene::Group { title, color },
                });
                self.apply(commands)?;
                Ok(
                    json!({"id":identity.to_string(),"item":self.scene.get(identity).map(item_json)}),
                )
            }
            Ungroup { ids } => {
                ensure!(!ids.is_empty(), "ungroup requires explicit ids");
                let selected = self.selected(&ids, true)?.iter().map(|i| i.id).collect();
                self.apply(groups::dissolve(&self.scene, &selected))
            }
            GroupEdit {
                id: identity,
                title,
                color,
            } => self.apply(vec![Command::SetGroup {
                id: id(&identity)?,
                group: moth_scene::Group { title, color },
            }]),
            Parent {
                id: identity,
                parent,
            } => self.apply(vec![Command::SetParent {
                id: id(&identity)?,
                parent: parent.as_deref().map(id).transpose()?,
            }]),
            Order { mode, ids } => {
                ensure!(!ids.is_empty(), "order requires explicit ids");
                let items = self.selected(&ids, false)?;
                ensure!(
                    items.iter().all(|i| !i.is_group()),
                    "order individual images/notes; group layering is derived"
                );
                let selected = items.iter().map(|i| i.id).collect();
                let (front, extreme) = match mode.as_str() {
                    "raise" => (true, false),
                    "lower" => (false, false),
                    "top" => (true, true),
                    "bottom" => (false, true),
                    _ => bail!("order must be raise, lower, top or bottom"),
                };
                self.apply(arrange::reorder(&self.scene, &selected, front, extreme))
            }
            Reset { mode, ids } => {
                let ids = self
                    .selected(&ids, true)?
                    .iter()
                    .filter(|i| i.blob().is_some())
                    .map(|i| i.id)
                    .collect();
                let mode = match mode.as_str() {
                    "scale" => moth_platform::export::Geometry::Scale,
                    "normalize" => moth_platform::export::Geometry::Normalize,
                    "crop" => moth_platform::export::Geometry::Crop,
                    "all" => moth_platform::export::Geometry::All,
                    _ => bail!("reset must be scale, normalize, crop or all"),
                };
                self.request(Request::OriginalGeometry { ids, mode })
            }
            Undo => self.request(Request::Undo),
            Redo => self.request(Request::Redo),
            Export {
                kind,
                path,
                ids,
                side,
                region,
            } => {
                ensure!(!dry_run, "export is disabled in dry-run");
                let ids = self
                    .selected(&ids, true)?
                    .iter()
                    .map(|i| i.id)
                    .collect::<Vec<_>>();
                use moth_platform::export::Target;
                let target = match kind.as_str() {
                    "scene" => Target::Scene {
                        path,
                        ids,
                        side,
                        region: region.map(rect).transpose()?,
                    },
                    "originals" => Target::Originals {
                        directory: path,
                        ids,
                    },
                    "archive" => Target::Archive { path, ids },
                    "original" => {
                        ensure!(ids.len() == 1, "original needs one image id");
                        Target::Original { path, id: ids[0] }
                    }
                    _ => bail!(
                        "export kind must be scene, original, originals or archive; scene extension .png or .svg"
                    ),
                };
                self.request(Request::Export(target))
            }
            PixelInfo {
                id: identity,
                point,
            } => self.request(Request::Export(moth_platform::export::Target::Info {
                id: id(&identity)?,
                point,
            })),
        }
    }
    fn top(&self) -> Option<FractionalIndex> {
        self.scene.items().map(|i| i.z.clone()).max()
    }
    fn translate(
        &mut self,
        ids: &[String],
        delta: Option<[f32; 2]>,
        duplicate: bool,
    ) -> Result<Value> {
        let items = self.selected(ids, true)?;
        let delta = delta.unwrap_or_else(|| {
            let anchor = self.next_anchor();
            let start = items
                .iter()
                .map(|item| moth_scene::footprint(item).x)
                .reduce(f32::min)
                .unwrap_or(anchor[0]);
            [anchor[0] - start, 0.]
        });
        let commands = arrange::translate(&items, delta, duplicate, self.top().as_ref());
        ensure!(
            items.is_empty() || !commands.is_empty(),
            "translation exceeds geometry limits"
        );
        let inserted: Vec<_> = commands
            .iter()
            .filter_map(|c| match c {
                Command::Insert(i) => Some(i.id.to_string()),
                _ => None,
            })
            .collect();
        let mut result = self.apply(commands)?;
        result["inserted"] = json!(inserted);
        Ok(result)
    }
}
