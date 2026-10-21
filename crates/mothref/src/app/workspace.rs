// SPDX-License-Identifier: GPL-3.0-or-later
//! Owns tabs, persistence identities and worker completion routing.
use super::*;
use document::MAX_TABS;

pub(super) enum TabChange {
    None,
    Switched,
    Closed,
    WindowClosed,
}
pub(super) enum Event {
    TabChanged,
    Opened,
    Preferences(std::collections::BTreeMap<String, String>),
}
pub(super) struct Pump<'a> {
    pub format: moth_io::ImageFormat,
    pub cursor: Option<[f32; 2]>,
    pub fallback: [f32; 2],
    pub auto_arrange: bool,
    pub threshold: usize,
    pub upload: &'a mut dyn FnMut(&moth_io::Decoded),
    pub events: Vec<Event>,
}
pub(super) struct Workspace {
    pub(super) board: Document,
    pub(super) tabs: Vec<Document>,
    pub(super) library: bool,
    recovery_paths: Vec<std::path::PathBuf>,
    recent_paths: Vec<std::path::PathBuf>,
    pub(super) refresh_library: bool,
    pub(super) confirm_close: bool,
    pub(super) closing_window: bool,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Workspace {
    pub(super) fn recent_paths(&self) -> &[std::path::PathBuf] {
        &self.recent_paths
    }
    pub(super) fn recovery_paths(&self) -> &[std::path::PathBuf] {
        &self.recovery_paths
    }
    pub(super) fn dismiss_recovery(&mut self) {
        self.recovery_paths.clear();
    }

    pub(super) fn new(board: Document, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            board,
            tabs: Vec::new(),
            library: true,
            recovery_paths: Vec::new(),
            recent_paths: Vec::new(),
            refresh_library: false,
            confirm_close: false,
            closing_window: false,
            wake,
        }
    }
    pub(super) fn activate(&mut self, index: usize) -> TabChange {
        if self.confirm_close || index > self.tabs.len() || (index == 0 && !self.library) {
            return TabChange::None;
        }
        if index > 0 {
            self.board.decode().requests.schedule([]);
            std::mem::swap(&mut self.board, &mut self.tabs[index - 1]);
        }
        self.library = false;
        TabChange::Switched
    }
    pub(super) fn request_close(&mut self, window: bool) -> TabChange {
        self.closing_window = window;
        self.library = false;
        if self.board.dirty || !self.board.idle() {
            self.confirm_close = true;
        } else {
            self.board.discard();
        }
        TabChange::None
    }
    pub(super) fn finish_close(&mut self) -> anyhow::Result<TabChange> {
        self.confirm_close = false;
        if let Some(board) = self.tabs.pop() {
            self.board = board;
            self.library = false;
            if self.closing_window {
                self.request_close(true);
            }
            Ok(TabChange::Switched)
        } else if self.closing_window {
            Ok(TabChange::WindowClosed)
        } else {
            self.board = Document::new(self.wake.clone(), false)?;
            self.library = true;
            self.refresh_library = true;
            Ok(TabChange::Closed)
        }
    }
    pub(super) fn new_board(&mut self) -> TabChange {
        if self.confirm_close {
            return TabChange::None;
        }
        if self.board.scene.is_empty() && self.board.path.is_empty() && self.board.idle() {
            return self.activate(0);
        }
        if self.tabs.len() + 1 >= MAX_TABS {
            self.board.message = moth_ui::text(moth_ui::Text::TabLimit).into();
            return TabChange::None;
        }
        match Document::new(self.wake.clone(), false) {
            Ok(board) => {
                self.board.decode().requests.schedule([]);
                let previous = std::mem::replace(&mut self.board, board);
                self.tabs.push(previous);
                self.library = false;
                return TabChange::Switched;
            }
            Err(error) => self.board.message = error.to_string(),
        }
        TabChange::None
    }
    pub(super) fn open_in_tab(&mut self, path: std::path::PathBuf) -> TabChange {
        if self.confirm_close {
            return TabChange::None;
        }
        if std::path::Path::new(&self.board.path) == path {
            return self.activate(0);
        }
        if let Some(index) = self
            .tabs
            .iter()
            .position(|b| std::path::Path::new(&b.path) == path)
        {
            return self.activate(index + 1);
        }
        if !self.board.scene.is_empty() || !self.board.path.is_empty() || !self.board.idle() {
            let count = self.tabs.len();
            self.new_board();
            if self.tabs.len() == count {
                return TabChange::None;
            }
        }
        self.library = false;
        self.open_board(path);
        TabChange::Switched
    }
    pub(super) fn persist(&mut self, request: moth_platform::boards::Request) {
        use moth_platform::boards::Request;
        if self.board.loading
            && !matches!(&request, Request::Preferences(_) | Request::SaveSettings(_))
        {
            self.board.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
            return;
        }
        let loading = matches!(
            &request,
            Request::Open(_) | Request::ImportBee(_) | Request::ImportPur(_)
        );
        if self.board.locked
            && matches!(
                &request,
                Request::Apply(..)
                    | Request::ApplyBatch(..)
                    | Request::OriginalGeometry { .. }
                    | Request::Undo
                    | Request::Redo
                    | Request::ImportBee(..)
                    | Request::ImportPur(..)
            )
        {
            return;
        }
        match self.board.boards().requests.try_send(request) {
            Ok(()) => {
                self.board.in_flight += 1;
                self.board.loading |= loading;
            }
            Err(_) => self.board.message = moth_ui::text(moth_ui::Text::StorageBusy).into(),
        }
    }
    pub(super) fn persist_batch(&mut self, commands: Vec<(Command, Option<Handle>)>) {
        self.persist(moth_platform::boards::Request::ApplyBatch(commands));
    }
    pub(super) fn begin_batch(&mut self) {
        if self.board.loading {
            return;
        }
        if !self.board.batching
            && self
                .board
                .boards()
                .requests
                .try_send(moth_platform::boards::Request::BeginBatch)
                .is_ok()
        {
            self.board.batching = true;
        }
    }
    pub(super) fn end_batch(&mut self) {
        if self.board.batching
            && self
                .board
                .boards()
                .requests
                .try_send(moth_platform::boards::Request::EndBatch)
                .is_ok()
        {
            self.board.batching = false;
        }
    }
    pub(super) fn discard(&mut self) {
        self.board.discard();
    }
    pub(super) fn open_board(&mut self, path: std::path::PathBuf) {
        if self.board.scene.is_empty()
            && self.board.in_flight == 0
            && self.board.pasting == 0
            && self.board.pending.is_empty()
            && self.board.waiting.is_empty()
            && !self.board.discarding
        {
            self.board.path = path.to_string_lossy().into_owned();
            self.persist(moth_platform::boards::Request::Open(path));
        } else {
            self.board.message = moth_ui::text(moth_ui::Text::OpenEmpty).into();
        }
    }
    pub(super) fn import_foreign(&mut self, path: std::path::PathBuf) -> TabChange {
        if self.confirm_close {
            return TabChange::None;
        }
        if !self.board.scene.is_empty() || !self.board.path.is_empty() || !self.board.idle() {
            let count = self.tabs.len();
            self.new_board();
            if self.tabs.len() == count {
                return TabChange::None;
            }
        }
        self.library = false;
        self.board.path = String::new();
        if self.board.idle() {
            let request = if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("pur"))
            {
                moth_platform::boards::Request::ImportPur(path)
            } else {
                moth_platform::boards::Request::ImportBee(path)
            };
            self.persist(request);
        }
        TabChange::Switched
    }
    pub(super) fn import(
        &mut self,
        path: std::path::PathBuf,
        format: moth_io::ImageFormat,
    ) -> TabChange {
        if self.board.locked || self.board.loading {
            return TabChange::None;
        }
        if self.board.discarding {
            return TabChange::None;
        }
        // A new drop supersedes a pending arrangement prompt.
        self.board.arrange_prompt = None;
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("moth"))
        {
            self.open_board(path);
            return TabChange::None;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("bee") || e.eq_ignore_ascii_case("pur"))
        {
            return self.import_foreign(path);
        }
        if self.board.scene.len() + self.board.waiting.len() + self.board.pending.len() >= 1_000_000
        {
            self.board.message = moth_ui::text(moth_ui::Text::BoardFull).into();
            return TabChange::None;
        }
        if self.board.waiting.len() >= 1024 {
            self.board.message = moth_ui::text(moth_ui::Text::ImportBusy).into();
            return TabChange::None;
        }
        match self.board.storage().register(path) {
            Ok(h) => self.board.waiting.push(h),
            Err(e) => self.board.message = e.to_string(),
        }
        self.queue_decodes(format);
        TabChange::None
    }
    pub(super) fn queue_decodes(&mut self, format: moth_io::ImageFormat) {
        while let Some(&handle) = self.board.waiting.last() {
            match self.board.decode().requests.try_send(moth_io::Request {
                tile: None,
                handle,
                side: 256,
                format,
                blob: None,
            }) {
                Ok(()) => {
                    self.board.waiting.pop();
                    self.board.pending.insert(handle, Pending::Import);
                }
                Err(_) => break,
            }
        }
    }
    pub(super) fn complete(
        &mut self,
        done: moth_io::Completion,
        point: [f32; 2],
        active: bool,
        pump: &mut Pump<'_>,
    ) {
        if let Some(hash) = done.request.blob {
            self.board.decode().requests.acknowledge(done.request);
            if !active || !self.board.decode().requests.current(done.request) {
                return;
            }
            match done.result {
                Ok(image) if image.hash == hash => (pump.upload)(&image),
                Ok(_) => {
                    self.board.failed.insert(done.request.handle);
                    self.board.message = moth_ui::text(moth_ui::Text::SourceChanged).into();
                }
                Err(error) => {
                    self.board.failed.insert(done.request.handle);
                    self.board.message = error.to_string();
                }
            }
            return;
        }
        let Some(pending) = self.board.pending.remove(&done.request.handle) else {
            return;
        };
        let paste = matches!(pending, Pending::Paste);
        let insert = paste || matches!(pending, Pending::Import);
        match done.result {
            Ok(image) => {
                if insert {
                    self.begin_batch();
                    // Both pastes and drops land under the cursor; drops cascade to stay visible.
                    let (p, offset) = if paste {
                        (point, 0.)
                    } else {
                        (point, (self.board.scene.len() % 10) as f32 * 24.)
                    };
                    let width = (image.original[0] as f32).min(600.);
                    let height = width * image.original[1] as f32 / image.original[0] as f32;
                    let mut item = Item::image(
                        image.hash,
                        Rect {
                            x: p[0] - width / 2. + offset,
                            y: p[1] - height / 2. + offset,
                            width,
                            height,
                        },
                        self.board.scene.top(),
                    );
                    if !paste {
                        item.name = self.board.storage().source_name(done.request.handle);
                    }
                    self.board.burst.push(item.id);
                    self.persist(moth_platform::boards::Request::Apply(
                        Command::Insert(item),
                        Some(done.request.handle),
                    ));
                }
                if active {
                    (pump.upload)(&image);
                }
            }
            Err(e) => {
                if insert && let Some(hash) = done.failure_hash {
                    self.begin_batch();
                    let p = point;
                    let mut item = Item::image(
                        hash,
                        Rect {
                            x: p[0] - 120.,
                            y: p[1] - 90.,
                            width: 240.,
                            height: 180.,
                        },
                        self.board.scene.top(),
                    );
                    if !paste {
                        item.name = self.board.storage().source_name(done.request.handle);
                    }
                    self.board.burst.push(item.id);
                    self.persist(moth_platform::boards::Request::Apply(
                        Command::Insert(item),
                        Some(done.request.handle),
                    ));
                }
                self.board.failed.insert(done.request.handle);
                self.board.message = e.to_string();
                tracing::warn!(error=%e,"image load failed");
            }
        }
    }
    fn drain_results(&mut self, active: bool, pump: &mut Pump<'_>) {
        let point = self.board.cursor_point(pump.cursor, pump.fallback);
        while let Ok(done) = self.board.boards().results.try_recv() {
            use moth_platform::boards::Completion;
            match done {
                Completion::Loading {
                    commands,
                    sources,
                    bounds,
                } => {
                    let first = self.board.scene.is_empty();
                    if first {
                        self.board.motion.clear();
                    }
                    for command in commands {
                        if let Err(error) = self.board.apply_visual(command) {
                            self.board.message = error.to_string();
                        }
                    }
                    self.board.sources.extend(sources);
                    if bounds.is_some() {
                        self.board.loading_bounds = bounds;
                    }
                    if first {
                        self.board.loading_bounds = self
                            .board
                            .loading_bounds
                            .or_else(|| self.board.scene.bounds());
                        self.board.fit();
                        if active {
                            pump.events.push(Event::Opened);
                        }
                    }
                    self.board.message = moth_ui::text(moth_ui::Text::StorageBusy).into();
                }
                Completion::Recovery(paths) => self.recovery_paths = paths,
                Completion::Recent(paths) => {
                    if self.recent_paths.is_empty() {
                        self.recent_paths = paths;
                        self.refresh_library = true;
                    }
                }
                Completion::Preferences(values) => {
                    pump.events.push(Event::Preferences(values));
                }
                Completion::Settings(code) => {
                    if let Some(language) = moth_ui::Locale::from_code(&code) {
                        moth_ui::set_locale(language);
                    }
                }
                done => {
                    self.board.in_flight = self.board.in_flight.saturating_sub(1);
                    match done {
                        Completion::Applied(command, source) => {
                            match self.board.apply_visual(command) {
                                Ok(_) => {
                                    if let Some((hash, handle)) = source {
                                        self.board.sources.insert(hash, handle);
                                    }
                                    self.board.dirty = true;
                                }
                                Err(e) => self.board.message = e.to_string(),
                            }
                            self.board
                                .selection
                                .retain(|id| self.board.scene.get(*id).is_some());
                        }
                        Completion::AppliedBatch(commands, sources) => {
                            let affected: HashSet<_> = commands
                                .iter()
                                .map(|command| match command {
                                    Command::Insert(item) => item.id,
                                    Command::Remove(id)
                                    | Command::SetRect { id, .. }
                                    | Command::SetTransform { id, .. }
                                    | Command::SetText { id, .. } => *id,
                                })
                                .collect();
                            let inserted: HashSet<_> = commands
                                .iter()
                                .filter_map(|c| match c {
                                    Command::Insert(item)
                                        if self.board.scene.get(item.id).is_none() =>
                                    {
                                        Some(item.id)
                                    }
                                    _ => None,
                                })
                                .collect();
                            for command in commands {
                                if let Err(e) = self.board.apply_visual(command) {
                                    self.board.message = e.to_string();
                                }
                            }
                            for (hash, handle) in sources {
                                self.board.sources.insert(hash, handle);
                            }
                            if !inserted.is_empty() {
                                self.board.selection = inserted;
                            } else if self.board.selection.is_empty() {
                                self.board.selection = affected;
                            }
                            self.board.dirty = true;
                            self.board
                                .selection
                                .retain(|id| self.board.scene.get(*id).is_some());
                        }
                        Completion::Opened(scene, sources, recovered) => {
                            let progressed = self.board.loading_bounds.take().is_some();
                            self.board.loading = false;
                            self.board.motion.clear();
                            self.board.replace_scene(scene);
                            self.board.sources = sources;
                            if active {
                                pump.events.push(Event::Opened);
                            }
                            self.recovery_paths.clear();
                            self.board.dirty = recovered;
                            self.board.message = moth_ui::text(if recovered {
                                moth_ui::Text::Recovered
                            } else {
                                moth_ui::Text::Opened
                            })
                            .into();
                            if !progressed {
                                self.board.fit();
                            }
                        }
                        Completion::Imported(commands, sources, skipped) => {
                            self.board.loading = false;
                            self.board.loading_bounds = None;
                            self.board.motion.clear();
                            self.board.replace_scene(Scene::default());
                            if active {
                                pump.events.push(Event::Opened);
                            }
                            for command in commands {
                                if let Err(e) = self.board.apply_visual(command) {
                                    self.board.message = e.to_string();
                                }
                            }
                            self.board.sources = sources;
                            self.board.path = String::new();
                            self.board.dirty = true;
                            self.board.message = moth_ui::text(if skipped == 0 {
                                moth_ui::Text::Imported
                            } else {
                                moth_ui::Text::ImportSkipped
                            })
                            .into();
                            self.board.fit();
                        }
                        Completion::Exported(value) => {
                            self.board.message = if value.is_empty() {
                                moth_ui::text(moth_ui::Text::Exported).into()
                            } else {
                                value
                            };
                        }
                        Completion::Saved => {
                            self.board.dirty = false;
                            let path = std::path::PathBuf::from(&self.board.path);
                            self.recent_paths.retain(|p| p != &path);
                            self.recent_paths.insert(0, path);
                            self.recent_paths
                                .truncate(moth_platform::library::MAX_BOARDS);
                            self.refresh_library = true;
                            self.board.message = moth_ui::text(moth_ui::Text::Saved).into();
                            if self.board.close_after_save {
                                self.discard();
                            }
                        }
                        Completion::Failed(e) => {
                            if self.board.loading {
                                self.board.loading = false;
                                self.board.loading_bounds = None;
                                self.board.path.clear();
                                self.board.dirty = !self.board.scene.is_empty();
                            }
                            self.board.message = e;
                            self.board.close_after_save = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        while let Ok(done) = self.board.decode().results.try_recv() {
            self.complete(done, point, active, pump);
        }
        while let Ok(result) = self.board.clipboard().results.try_recv() {
            match result {
                moth_platform::ClipboardCompletion::Discarded(result) => match result {
                    Ok(()) => self.board.discarded = true,
                    Err(error) => {
                        self.board.discarding = false;
                        self.board.message = error.to_string();
                    }
                },
                moth_platform::ClipboardCompletion::Pasted(Ok((handle, image))) => {
                    self.board.pasting = self.board.pasting.saturating_sub(1);
                    if self.board.scene.len() + self.board.waiting.len() + self.board.pending.len()
                        < 1_000_000
                    {
                        self.board.pending.insert(handle, Pending::Paste);
                        self.complete(
                            moth_io::Completion {
                                request: moth_io::Request {
                                    tile: None,
                                    handle,
                                    side: 256,
                                    format: pump.format,
                                    blob: None,
                                },
                                result: Ok(image),
                                failure_hash: None,
                            },
                            point,
                            active,
                            pump,
                        );
                    } else {
                        self.board.message = moth_ui::text(moth_ui::Text::BoardFull).into();
                    }
                }
                moth_platform::ClipboardCompletion::Files(paths) => {
                    self.board.pasting = self.board.pasting.saturating_sub(1);
                    for path in paths {
                        let change = self.import(path, pump.format);
                        if active && matches!(change, TabChange::Switched) {
                            pump.events.push(Event::TabChanged);
                        }
                    }
                }
                moth_platform::ClipboardCompletion::Pasted(Err(
                    moth_platform::PlatformError::EmptyClipboard,
                )) => {
                    self.board.pasting = self.board.pasting.saturating_sub(1);
                    self.board.message = moth_ui::text(moth_ui::Text::ClipboardEmpty).into();
                }
                moth_platform::ClipboardCompletion::Pasted(Err(e)) => {
                    self.board.pasting = self.board.pasting.saturating_sub(1);
                    self.board.message = e.to_string();
                }
            }
        }
        self.queue_decodes(pump.format);
        if self.board.batching && self.board.idle() && self.board.arrange_prompt.is_none() {
            if !self.board.burst.is_empty()
                && (pump.auto_arrange || self.board.burst.len() > pump.threshold)
            {
                self.board.arrange_prompt = Some(std::mem::take(&mut self.board.burst));
            } else {
                self.board.burst.clear();
                self.end_batch();
            }
        }
    }
    pub(super) fn pump(&mut self, mut pump: Pump<'_>) -> Vec<Event> {
        for index in 0..self.tabs.len() {
            std::mem::swap(&mut self.board, &mut self.tabs[index]);
            self.drain_results(false, &mut pump);
            std::mem::swap(&mut self.board, &mut self.tabs[index]);
        }
        self.drain_results(true, &mut pump);
        pump.events
    }
}
