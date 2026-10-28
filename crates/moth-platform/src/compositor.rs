// SPDX-License-Identifier: GPL-3.0-or-later
//! Wayland gives clients no way to stack their own windows, so stacking requests go
//! to the compositor's own scripting surface over the session bus: KWin scripts on
//! KDE Plasma and the bundled MothRef Shell extension on GNOME.
//!
//! Everything here runs on one worker thread. The main thread only queues requests
//! and reads atomics, so a slow or missing compositor never stalls a frame.
use crate::{ShellIntegration, dbus};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

/// Bus name of the bundled GNOME Shell extension.
const GNOME_SERVICE: &str = "org.mothref.Shell";
/// Directory name the GNOME Shell extension is installed under.
pub(crate) const GNOME_UUID: &str = "mothref-windows@hlophlopgaming.github.io";
const GNOME_METADATA: &str = include_str!("../gnome-shell/metadata.json");
const GNOME_EXTENSION: &str = include_str!("../gnome-shell/extension.js");
/// Object path KWin scripts call back on with `callDBus`.
const CALLBACK_PATH: &str = "/org/mothref/Compositor";
const CALLBACK_INTERFACE: &str = "org.mothref.Compositor1";

enum Request {
    Probe,
    /// Keeps every MothRef window with this title above (or back in) the normal layer.
    Layer {
        title: String,
        above: bool,
    },
    /// Reports the next activated foreign window through `Picked`.
    Pick {
        title: String,
    },
    /// Keeps the board above `target` whenever it is active; `None` stops.
    Attach {
        title: String,
        target: Option<String>,
    },
    CancelPick,
    InstallGnome,
}

struct Shared {
    requests: crossbeam_channel::Sender<Request>,
    wake: Arc<dyn Fn() + Send + Sync>,
    kwin: AtomicBool,
    gnome: AtomicU8,
    error: Mutex<Option<String>>,
    picked: Mutex<Option<Option<String>>>,
    closed: Mutex<HashSet<String>>,
    kwin_owner: Mutex<Option<String>>,
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

/// Starts the worker once; later calls keep the first wake callback.
pub(crate) fn start(wake: Arc<dyn Fn() + Send + Sync>) {
    SHARED.get_or_init(|| {
        // A small queue is enough: requests follow explicit user actions.
        let (requests, receiver) = crossbeam_channel::bounded(16);
        let shared = Arc::new(Shared {
            requests,
            wake,
            kwin: AtomicBool::new(false),
            gnome: AtomicU8::new(0),
            error: Mutex::new(None),
            picked: Mutex::new(None),
            closed: Mutex::new(HashSet::new()),
            kwin_owner: Mutex::new(None),
        });
        let worker = shared.clone();
        let spawned = std::thread::Builder::new()
            .name("compositor".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Worker::new(worker.clone()).map(|mut w| w.run(receiver))
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::info!(%error, "Compositor integration is unavailable")
                    }
                    Err(_) => tracing::error!("Compositor integration stopped unexpectedly"),
                }
            });
        if let Err(error) = spawned {
            tracing::warn!(%error, "Cannot start compositor integration");
        } else {
            let _ = shared.requests.try_send(Request::Probe);
        }
        shared
    });
}

fn send(request: Request) {
    if let Some(shared) = SHARED.get()
        && shared.requests.try_send(request).is_err()
    {
        report(shared, "The window manager is not responding".into());
    }
}
fn report(shared: &Shared, error: String) {
    tracing::warn!(%error, "Compositor request failed");
    if let Ok(mut slot) = shared.error.lock() {
        *slot = Some(error);
    }
    (shared.wake)();
}

/// KWin accepts window scripts, so above-all and window attachment both work.
pub(crate) fn kwin() -> bool {
    SHARED.get().is_some_and(|s| s.kwin.load(Ordering::Acquire))
}
/// The GNOME extension is running and can keep windows above.
pub(crate) fn gnome() -> bool {
    shell_integration() == ShellIntegration::Active
}
/// Whether Settings should offer to install the GNOME extension.
pub(crate) fn shell_integration() -> ShellIntegration {
    match SHARED.get().map(|s| s.gnome.load(Ordering::Acquire)) {
        Some(1) => ShellIntegration::Missing,
        Some(2) => ShellIntegration::Pending,
        Some(3) => ShellIntegration::Active,
        _ => ShellIntegration::NotNeeded,
    }
}
/// Queues a keep-above change for every MothRef window with this title.
pub(crate) fn keep_above(title: String, above: bool) {
    send(Request::Layer { title, above });
}
/// Starts KWin window selection; the result arrives through [`take_picked`].
pub(crate) fn pick(title: String) {
    if let Some(shared) = SHARED.get()
        && let Ok(mut picked) = shared.picked.lock()
    {
        *picked = None;
    }
    send(Request::Pick { title });
}
/// Stops a running selection, for example when the user cancels inside MothRef.
pub(crate) fn cancel_pick() {
    send(Request::CancelPick);
}
/// `Some(None)` means selection ended without a window.
pub(crate) fn take_picked() -> Option<Option<String>> {
    SHARED.get()?.picked.lock().ok()?.take()
}
/// Keeps the board above `target` while it is active, or stops when `None`.
pub(crate) fn attach(title: String, target: Option<String>) {
    send(Request::Attach { title, target });
}
/// A target reported closed by KWin is never re-enabled.
pub(crate) fn target_closed(target: &str) -> bool {
    SHARED
        .get()
        .and_then(|s| s.closed.lock().ok().map(|c| c.contains(target)))
        .unwrap_or(false)
}
/// Copies the GNOME extension into the user's extension folder and enables it.
pub(crate) fn install_gnome() {
    send(Request::InstallGnome);
}
/// Asynchronous failures are shown once in the status bar.
pub(crate) fn take_error() -> Option<String> {
    SHARED.get()?.error.lock().ok()?.take()
}

/// Answers KWin's `callDBus` reports. Only KWin may report; other clients are refused.
fn callback(shared: &Shared, message: &dbus::Message) -> Result<Vec<dbus::Arg>, String> {
    let owner = shared.kwin_owner.lock().ok().and_then(|o| o.clone());
    if message.sender.is_none() || message.sender != owner {
        return Err("Only the compositor may report window events".into());
    }
    if message.path.as_deref() != Some(CALLBACK_PATH)
        || message
            .interface
            .as_deref()
            .is_some_and(|i| i != CALLBACK_INTERFACE)
    {
        return Err("Unknown object".into());
    }
    let [dbus::Arg::Str(id)] = message.args.as_slice() else {
        return Err("Expected one string".into());
    };
    match message.member.as_deref() {
        Some("Picked") => {
            if let Ok(mut picked) = shared.picked.lock() {
                *picked = Some((!id.is_empty()).then(|| id.clone()));
            }
        }
        Some("TargetClosed") => {
            if let Ok(mut closed) = shared.closed.lock() {
                // Ids are per-window UUIDs; a few hundred per session is plenty.
                if closed.len() >= 256 {
                    closed.clear();
                }
                closed.insert(id.clone());
            }
        }
        _ => return Err("Unknown method".into()),
    }
    (shared.wake)();
    Ok(Vec::new())
}

struct Worker {
    shared: Arc<Shared>,
    bus: dbus::Bus,
    /// Loaded script per slot, so a new request replaces the previous one.
    scripts: HashMap<String, PathBuf>,
    /// Titles raised in this process; lowering anything else is a no-op.
    raised: HashSet<String>,
    attached: Option<String>,
    serial: u64,
}

impl Worker {
    fn new(shared: Arc<Shared>) -> Result<Self, dbus::Error> {
        let handler = shared.clone();
        let bus = dbus::Bus::session(Box::new(move |message| callback(&handler, message)))?;
        Ok(Self {
            shared,
            bus,
            scripts: HashMap::new(),
            raised: HashSet::new(),
            attached: None,
            serial: 0,
        })
    }
    fn run(&mut self, requests: crossbeam_channel::Receiver<Request>) {
        while let Ok(request) = requests.recv() {
            if let Err(error) = self.handle(request) {
                report(&self.shared, error);
            }
        }
    }
    fn handle(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Probe => {
                self.probe();
                Ok(())
            }
            Request::Layer { title, above } => self.layer(title, above),
            Request::Pick { title } => {
                let result = self.kwin_script(
                    "pick",
                    &pick_script(std::process::id(), &title, &self.service()),
                );
                if result.is_err()
                    && let Ok(mut picked) = self.shared.picked.lock()
                {
                    *picked = Some(None);
                }
                result
            }
            Request::CancelPick => self.unload("pick"),
            Request::Attach { title, target } => self.attach(title, target),
            Request::InstallGnome => self.install_gnome(),
        }
    }
    fn service(&self) -> String {
        self.bus.unique_name().to_owned()
    }
    fn has_owner(&self, name: &str) -> bool {
        self.bus.has_owner(name)
    }
    fn probe(&mut self) {
        let kwin = self.has_owner("org.kde.KWin");
        self.shared.kwin.store(kwin, Ordering::Release);
        let gnome = if kwin || !self.has_owner("org.gnome.Shell") {
            0
        } else if self.has_owner(GNOME_SERVICE) {
            3
        } else if gnome_directory().is_some_and(|d| d.join("extension.js").is_file()) {
            2
        } else {
            1
        };
        self.shared.gnome.store(gnome, Ordering::Release);
        tracing::info!(kwin, gnome, "Compositor integration probed");
        (self.shared.wake)();
    }
    fn layer(&mut self, title: String, above: bool) -> Result<(), String> {
        if !above && !self.raised.contains(&title) {
            return Ok(());
        }
        if self.shared.kwin.load(Ordering::Acquire) {
            let slot = format!("layer-{}", slot_hash(&title));
            self.kwin_script(&slot, &layer_script(std::process::id(), &title, above))?;
        } else if self.shared.gnome.load(Ordering::Acquire) == 3 {
            self.gnome_layer(&title, above)?;
        } else {
            return Err("This desktop cannot keep windows on top".into());
        }
        if above {
            self.raised.insert(title);
        } else {
            self.raised.remove(&title);
        }
        Ok(())
    }
    fn gnome_layer(&self, title: &str, above: bool) -> Result<(), String> {
        // A window created a moment ago may not be mapped yet; retry briefly.
        for delay in [0, 50, 100, 200, 400, 800, 1600] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            let reply = self
                .bus
                .call(
                    GNOME_SERVICE,
                    "/org/mothref/Shell",
                    "org.mothref.Shell1",
                    "SetKeepAbove",
                    &[title.into(), above.into()],
                )
                .map_err(|e| e.to_string())?;
            if !matches!(reply.as_slice(), [dbus::Arg::U32(0)]) {
                return Ok(());
            }
        }
        if above {
            Err("GNOME did not find the MothRef window".into())
        } else {
            Ok(())
        }
    }
    fn attach(&mut self, title: String, target: Option<String>) -> Result<(), String> {
        match target {
            Some(target) => {
                // Leave above-all first; the app's own lowering request arrives
                // after this one and would undo the attach script's raise.
                if self.raised.remove(&title) {
                    let slot = format!("layer-{}", slot_hash(&title));
                    self.kwin_script(&slot, &layer_script(std::process::id(), &title, false))?;
                }
                self.attached = Some(title.clone());
                self.kwin_script(
                    "attach",
                    &attach_script(std::process::id(), &title, &target, &self.service()),
                )
            }
            None => {
                self.unload("attach")?;
                // The script may have left the board above when it stopped.
                if let Some(title) = self.attached.take() {
                    let slot = format!("layer-{}", slot_hash(&title));
                    self.kwin_script(&slot, &layer_script(std::process::id(), &title, false))?;
                }
                Ok(())
            }
        }
    }
    fn kwin_call(
        &self,
        path: &str,
        interface: &str,
        method: &str,
        args: &[dbus::Arg],
    ) -> Result<Vec<dbus::Arg>, dbus::Error> {
        self.bus.call("org.kde.KWin", path, interface, method, args)
    }
    fn unload(&mut self, slot: &str) -> Result<(), String> {
        let name = plugin_name(slot);
        self.kwin_call(
            "/Scripting",
            "org.kde.kwin.Scripting",
            "unloadScript",
            &[name.as_str().into()],
        )
        .map_err(|e| e.to_string())?;
        if let Some(path) = self.scripts.remove(slot) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
    /// Replaces the script in `slot`. Scripts stay loaded until replaced, because
    /// KWin evaluates them asynchronously and waiting ones listen for new windows.
    fn kwin_script(&mut self, slot: &str, source: &str) -> Result<(), String> {
        let owner = self.bus.owner("org.kde.KWin").map_err(|e| e.to_string())?;
        if let Ok(mut slot) = self.shared.kwin_owner.lock() {
            *slot = Some(owner);
        }
        self.unload(slot)?;
        self.serial += 1;
        let path = script_directory().join(format!(
            "mothref-kwin-{}-{slot}-{}.js",
            std::process::id(),
            self.serial
        ));
        write_new(&path, source).map_err(|e| e.to_string())?;
        self.scripts.insert(slot.to_owned(), path.clone());
        let file = path.to_str().ok_or("Script path is not UTF-8")?;
        let reply = self
            .kwin_call(
                "/Scripting",
                "org.kde.kwin.Scripting",
                "loadScript",
                &[file.into(), plugin_name(slot).as_str().into()],
            )
            .map_err(|e| e.to_string())?;
        let [dbus::Arg::I32(id)] = reply.as_slice() else {
            return Err("Unexpected reply from KWin".into());
        };
        let id = *id;
        if id < 0 {
            return Err("KWin refused the window script".into());
        }
        // Plasma 6 and Plasma 5 export loaded scripts under different paths.
        self.kwin_call(
            &format!("/Scripting/Script{id}"),
            "org.kde.kwin.Script",
            "run",
            &[],
        )
        .or_else(|_| self.kwin_call(&format!("/{id}"), "org.kde.kwin.Script", "run", &[]))
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn install_gnome(&mut self) -> Result<(), String> {
        let directory = gnome_directory().ok_or("Cannot find the GNOME extension folder")?;
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        for (name, source) in [
            ("metadata.json", GNOME_METADATA),
            ("extension.js", GNOME_EXTENSION),
        ] {
            replace(&directory.join(name), source).map_err(|e| e.to_string())?;
        }
        let enabled = std::process::Command::new("gnome-extensions")
            .args(["enable", GNOME_UUID])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        self.probe();
        if !enabled && !self.has_owner(GNOME_SERVICE) {
            return Err(format!(
                "Installed. Enable “MothRef window integration” in the Extensions app ({GNOME_UUID})"
            ));
        }
        Ok(())
    }
}

fn plugin_name(slot: &str) -> String {
    format!("mothref-{}-{slot}", std::process::id())
}
fn slot_hash(title: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    title.hash(&mut hasher);
    hasher.finish()
}
/// The per-user runtime directory is private to the user, unlike `/tmp`.
fn script_directory() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(std::env::temp_dir)
}
fn gnome_directory() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|home| home.join(".local/share"))
        })?;
    Some(data.join("gnome-shell/extensions").join(GNOME_UUID))
}
fn write_new(path: &std::path::Path, source: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(source.as_bytes())
}
/// Writes beside the target and renames, so GNOME never reads a half-written file.
fn replace(path: &std::path::Path, source: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("mothref-new");
    let _ = std::fs::remove_file(&temporary);
    write_new(&temporary, source)?;
    std::fs::rename(temporary, path)
}

/// Encodes text as a JavaScript string literal for generated scripts.
fn js_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{2028}' | '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if c.is_control() || c == '<' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
/// Shared helpers; written for both the Plasma 6 and Plasma 5 scripting API.
fn prelude(pid: u32, title: &str, service: Option<&str>) -> String {
    // Layer scripts never call back, so they need no bus name.
    let report = match service {
        Some(service) => format!(
            "callDBus({}, {}, {}, method, value);",
            js_string(service),
            js_string(CALLBACK_PATH),
            js_string(CALLBACK_INTERFACE)
        ),
        None => String::new(),
    };
    format!(
        r#"const pid = {pid};
const title = {title};
// Copy into a plain array: Qt may hand out a list-like object without filter().
function windows() {{
    const list = workspace.windowList ? workspace.windowList() : workspace.clientList();
    const out = [];
    for (let i = 0; i < list.length; i++) out.push(list[i]);
    return out;
}}
const activated = workspace.windowActivated || workspace.clientActivated;
const added = workspace.windowAdded || workspace.clientAdded;
const removed = workspace.windowRemoved || workspace.clientRemoved;
// Sandboxed builds see a different PID, so the class is a fallback.
function mine(w) {{ return !!w && (w.pid === pid || String(w.resourceClass).toLowerCase().indexOf("mothref") >= 0); }}
function board(w) {{ return mine(w) && w.caption === title; }}
function boards() {{ return windows().filter(board); }}
function id(w) {{ return String(w.internalId); }}
function report(method, value) {{ {report} }}
"#,
        title = js_string(title),
    )
}
fn layer_script(pid: u32, title: &str, above: bool) -> String {
    format!(
        r#"{prelude}
const above = {above};
boards().forEach(function (w) {{ w.keepAbove = above; }});
// Windows with this title mapped while the script is loaded (a new floating
// view, a controller still being shown) follow the same choice.
if (above) {{
    added.connect(function (w) {{ if (board(w)) w.keepAbove = true; }});
}}
"#,
        prelude = prelude(pid, title, None),
    )
}
fn pick_script(pid: u32, title: &str, service: &str) -> String {
    format!(
        r#"{prelude}
const pick = function (w) {{
    if (!w || mine(w) || !w.normalWindow) return;
    activated.disconnect(pick);
    report("Picked", id(w));
}};
activated.connect(pick);
"#,
        prelude = prelude(pid, title, Some(service))
    )
}
fn attach_script(pid: u32, title: &str, target: &str, service: &str) -> String {
    format!(
        r#"{prelude}
const target = {target};
function raise(above) {{ boards().forEach(function (w) {{ w.keepAbove = above; }}); }}
if (!windows().some(function (w) {{ return id(w) === target; }})) {{
    report("TargetClosed", target);
}} else {{
    const follow = function (w) {{
        if (!w) return;
        if (id(w) === target) raise(true);
        else if (!mine(w)) raise(false);
    }};
    const gone = function (w) {{
        if (!w || id(w) !== target) return;
        activated.disconnect(follow);
        removed.disconnect(gone);
        raise(false);
        report("TargetClosed", target);
    }};
    activated.connect(follow);
    removed.connect(gone);
    follow(workspace.activeWindow || workspace.activeClient);
}}
"#,
        prelude = prelude(pid, title, Some(service)),
        target = js_string(target)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn script_strings_cannot_escape_their_literal() {
        assert_eq!(js_string("MothRef"), "\"MothRef\"");
        assert_eq!(js_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(js_string("x\ny\u{2028}"), "\"x\\u000ay\\u2028\"");
        assert_eq!(js_string("</script>"), "\"\\u003c/script>\"");
        assert_eq!(js_string("Наложение"), "\"Наложение\"");
    }
    #[test]
    fn scripts_embed_quoted_identity() {
        let layer = layer_script(42, "Mo\"th", true);
        assert!(layer.contains("const pid = 42;"));
        assert!(layer.contains("const title = \"Mo\\\"th\";"));
        assert!(layer.contains("const above = true;"));
        assert!(!layer.contains("callDBus"));
        let attach = attach_script(7, "MothRef", "{abc}", ":1.5");
        assert!(attach.contains("const target = \"{abc}\";"));
        assert!(attach.contains("callDBus(\":1.5\", \"/org/mothref/Compositor\""));
        assert!(pick_script(7, "MothRef", ":1.5").contains("report(\"Picked\""));
    }
}
