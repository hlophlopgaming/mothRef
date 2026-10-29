// SPDX-License-Identifier: GPL-3.0-or-later
//! Wayland gives clients no way to stack their own windows, so stacking requests go
//! to the compositor's own scripting surface over the session bus: KWin scripts on
//! KDE Plasma and the bundled MothRef Shell extension on GNOME.
//!
//! Everything here runs on one worker thread that talks to the bus through the
//! system libdbus. The main thread only queues requests and reads atomics, so a
//! slow or missing compositor never stalls a frame. The worker sleeps in `poll`
//! on the bus socket and a wake socket, so an idle app does no work.
use crate::ShellIntegration;
use dbus::blocking::{Connection, Proxy};
use dbus::channel::{Channel, MatchingReceiver, Sender as _};
use dbus::message::MatchRule;
use std::collections::{HashMap, HashSet};
use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU8, Ordering},
};
use std::time::Duration;

const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Bus name of the bundled GNOME Shell extension.
const GNOME_SERVICE: &str = "org.mothref.Shell";
/// Directory name the GNOME Shell extension is installed under.
pub(crate) const GNOME_UUID: &str = "mothref-windows@hlophlopgaming.github.io";
/// Matches `Version` in `gnome-shell/extension.js`; bump both on behavior changes.
const GNOME_VERSION: u32 = 5;
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
    /// One byte per request wakes the worker from `poll`.
    notify: UnixStream,
    wake: Arc<dyn Fn() + Send + Sync>,
    kwin: AtomicBool,
    gnome: AtomicU8,
    error: Mutex<Option<String>>,
    picked: Mutex<Option<Option<String>>>,
    closed: Mutex<HashSet<String>>,
    reporter: Mutex<Option<String>>,
}

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

/// Starts the worker once; later calls keep the first wake callback.
pub(crate) fn start(wake: Arc<dyn Fn() + Send + Sync>) {
    if SHARED.get().is_some() {
        return;
    }
    match spawn(wake, || {
        Channel::get_private(dbus::channel::BusType::Session)
    }) {
        Ok(shared) => {
            let _ = SHARED.set(shared);
            send(Request::Probe);
        }
        Err(error) => tracing::warn!(%error, "Cannot start compositor integration"),
    }
}
/// Starts a worker on the bus `open` connects to; tests pass a private bus.
fn spawn(
    wake: Arc<dyn Fn() + Send + Sync>,
    open: impl FnOnce() -> Result<Channel, dbus::Error> + Send + 'static,
) -> std::io::Result<Arc<Shared>> {
    // A small queue is enough: requests follow explicit user actions.
    let (requests, receiver) = crossbeam_channel::bounded(16);
    let (notify, wakeup) = UnixStream::pair()?;
    notify.set_nonblocking(true)?;
    wakeup.set_nonblocking(true)?;
    let shared = Arc::new(Shared {
        requests,
        notify,
        wake,
        kwin: AtomicBool::new(false),
        gnome: AtomicU8::new(0),
        error: Mutex::new(None),
        picked: Mutex::new(None),
        closed: Mutex::new(HashSet::new()),
        reporter: Mutex::new(None),
    });
    let worker = shared.clone();
    std::thread::Builder::new()
        .name("compositor".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Worker::new(worker.clone(), open).and_then(|mut w| w.run(&receiver, wakeup))
            }));
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::info!(%error, "Compositor integration is unavailable")
                }
                Err(_) => tracing::error!("Compositor integration stopped unexpectedly"),
            }
            // Later requests fail visibly instead of waiting forever.
            worker.kwin.store(false, Ordering::Release);
            worker.gnome.store(0, Ordering::Release);
            (worker.wake)();
        })?;
    Ok(shared)
}

fn send(request: Request) {
    if let Some(shared) = SHARED.get() {
        enqueue(shared, request);
    }
}
fn enqueue(shared: &Shared, request: Request) {
    if shared.requests.try_send(request).is_err() {
        report(shared, "The window manager is not responding".into());
        return;
    }
    // A full wake socket already holds an unread wake-up.
    let _ = (&shared.notify).write(&[1]);
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
/// The compositor can pin the board above a chosen window (KWin or the GNOME extension).
pub(crate) fn attach_supported() -> bool {
    kwin() || gnome()
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
fn callback(shared: &Shared, message: &dbus::Message) -> Result<(), String> {
    let owner = shared.reporter.lock().ok().and_then(|o| o.clone());
    let sender = message.sender().map(|s| s.to_string());
    if sender.is_none() || sender != owner {
        return Err("Only the compositor may report window events".into());
    }
    if message.path().as_deref() != Some(CALLBACK_PATH)
        || message
            .interface()
            .is_some_and(|i| &*i != CALLBACK_INTERFACE)
    {
        return Err("Unknown object".into());
    }
    let id: String = message
        .read1()
        .map_err(|_| "Expected one string".to_owned())?;
    match message.member().as_deref() {
        Some("Picked") => {
            if let Ok(mut picked) = shared.picked.lock() {
                *picked = Some((!id.is_empty()).then_some(id));
            }
        }
        Some("TargetClosed") => {
            if let Ok(mut closed) = shared.closed.lock() {
                // Ids are per-window UUIDs; a few hundred per session is plenty.
                if closed.len() >= 256 {
                    closed.clear();
                }
                closed.insert(id);
            }
        }
        _ => return Err("Unknown method".into()),
    }
    (shared.wake)();
    Ok(())
}

struct Worker {
    shared: Arc<Shared>,
    bus: Connection,
    /// Loaded script per slot, so a new request replaces the previous one.
    scripts: HashMap<String, PathBuf>,
    /// Titles raised in this process; lowering anything else is a no-op.
    raised: HashSet<String>,
    attached: Option<String>,
    serial: u64,
}

impl Worker {
    fn new(
        shared: Arc<Shared>,
        open: impl FnOnce() -> Result<Channel, dbus::Error>,
    ) -> Result<Self, String> {
        let mut channel = open().map_err(|e| e.to_string())?;
        // Exposes the socket so the worker can sleep in poll() between messages.
        channel.set_watch_enabled(true);
        let bus = Connection::from(channel);
        let handler = shared.clone();
        bus.start_receive(
            MatchRule::new_method_call(),
            Box::new(move |message, bus| {
                let answer = callback(&handler, &message);
                if !message.get_no_reply() {
                    let reply = match answer {
                        Ok(()) => message.method_return(),
                        Err(text) => message.error(
                            &"org.freedesktop.DBus.Error.Failed".into(),
                            &std::ffi::CString::new(text).unwrap_or_default(),
                        ),
                    };
                    let _ = bus.send(reply);
                }
                true
            }),
        );
        Ok(Self {
            shared,
            bus,
            scripts: HashMap::new(),
            raised: HashSet::new(),
            attached: None,
            serial: 0,
        })
    }
    /// Alternates between bus messages and queued requests, sleeping in between.
    fn run(
        &mut self,
        requests: &crossbeam_channel::Receiver<Request>,
        mut wakeup: UnixStream,
    ) -> Result<(), String> {
        use rustix::event::{PollFd, PollFlags, poll};
        let fd = self.bus.channel().watch().fd;
        loop {
            let mut busy = false;
            // Drains messages libdbus already buffered, then whatever the socket has.
            while self
                .bus
                .process(Duration::ZERO)
                .map_err(|e| e.to_string())?
            {
                busy = true;
            }
            while let Ok(request) = requests.try_recv() {
                busy = true;
                if let Err(error) = self.handle(request) {
                    report(&self.shared, error);
                }
            }
            if busy {
                continue;
            }
            self.bus.channel().flush();
            // SAFETY: the descriptor belongs to `self.bus`, which outlives this borrow.
            let socket = unsafe { std::os::fd::BorrowedFd::borrow_raw(fd) };
            let mut fds = [
                PollFd::new(&socket, PollFlags::IN),
                PollFd::new(&wakeup, PollFlags::IN),
            ];
            match poll(&mut fds, None) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.to_string()),
            }
            if fds[0].revents().intersects(PollFlags::ERR | PollFlags::HUP) {
                return Err("Session bus closed".into());
            }
            let mut drain = [0u8; 64];
            while matches!(wakeup.read(&mut drain), Ok(n) if n > 0) {}
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
                let result = if self.gnome_active() {
                    self.gnome_call("Pick", ())
                } else {
                    self.kwin_script(
                        "pick",
                        &pick_script(std::process::id(), &title, &self.service()),
                    )
                };
                if result.is_err()
                    && let Ok(mut picked) = self.shared.picked.lock()
                {
                    *picked = Some(None);
                }
                result
            }
            Request::CancelPick if self.gnome_active() => self.gnome_call("CancelPick", ()),
            Request::CancelPick => self.unload("pick"),
            Request::Attach { title, target } => self.attach(title, target),
            Request::InstallGnome => self.install_gnome(),
        }
    }
    fn service(&self) -> String {
        self.bus.unique_name().to_string()
    }
    fn proxy<'a>(&'a self, destination: &'a str, path: &'a str) -> Proxy<'a, &'a Connection> {
        self.bus.with_proxy(destination, path, CALL_TIMEOUT)
    }
    fn bus_proxy(&self) -> Proxy<'_, &Connection> {
        self.proxy("org.freedesktop.DBus", "/org/freedesktop/DBus")
    }
    fn has_owner(&self, name: &str) -> bool {
        self.bus_proxy()
            .method_call("org.freedesktop.DBus", "NameHasOwner", (name,))
            .is_ok_and(|(owned,): (bool,)| owned)
    }
    fn gnome_version(&self) -> u32 {
        use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
        self.proxy(GNOME_SERVICE, "/org/mothref/Shell")
            .get("org.mothref.Shell1", "Version")
            .unwrap_or(0)
    }
    fn probe(&mut self) {
        let kwin = self.has_owner("org.kde.KWin");
        self.shared.kwin.store(kwin, Ordering::Release);
        let gnome = if kwin || !self.has_owner("org.gnome.Shell") {
            0
        } else if self.has_owner(GNOME_SERVICE) {
            // An older bundled extension is offered for reinstalling.
            if self.gnome_version() >= GNOME_VERSION {
                3
            } else {
                1
            }
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
            let (count,): (u32,) = self
                .proxy(GNOME_SERVICE, "/org/mothref/Shell")
                .method_call("org.mothref.Shell1", "SetKeepAbove", (title, above))
                .map_err(|e| e.to_string())?;
            if count > 0 {
                return Ok(());
            }
        }
        if above {
            Err("GNOME did not find the MothRef window".into())
        } else {
            Ok(())
        }
    }
    fn gnome_active(&self) -> bool {
        !self.shared.kwin.load(Ordering::Acquire) && self.shared.gnome.load(Ordering::Acquire) == 3
    }
    /// Calls the GNOME extension and trusts its owner for the callbacks it sends.
    fn gnome_call<A: dbus::arg::AppendAll>(&self, method: &str, args: A) -> Result<(), String> {
        let (owner,): (String,) = self
            .bus_proxy()
            .method_call("org.freedesktop.DBus", "GetNameOwner", (GNOME_SERVICE,))
            .map_err(|e| e.to_string())?;
        if let Ok(mut slot) = self.shared.reporter.lock() {
            *slot = Some(owner);
        }
        self.proxy(GNOME_SERVICE, "/org/mothref/Shell")
            .method_call::<(), _, _, _>("org.mothref.Shell1", method, args)
            .map_err(|e| e.to_string())
    }
    fn attach(&mut self, title: String, target: Option<String>) -> Result<(), String> {
        if self.gnome_active() {
            self.raised.remove(&title);
            return match target {
                Some(target) => self.gnome_call("Attach", (title.as_str(), target.as_str())),
                None => self.gnome_call("Detach", ()),
            };
        }
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
    fn unload(&mut self, slot: &str) -> Result<(), String> {
        let name = plugin_name(slot);
        let _: (bool,) = self
            .proxy("org.kde.KWin", "/Scripting")
            .method_call("org.kde.kwin.Scripting", "unloadScript", (name.as_str(),))
            .map_err(|e| e.to_string())?;
        if let Some(path) = self.scripts.remove(slot) {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
    /// Replaces the script in `slot`. Scripts stay loaded until replaced, because
    /// KWin evaluates them asynchronously and waiting ones listen for new windows.
    fn kwin_script(&mut self, slot: &str, source: &str) -> Result<(), String> {
        let (owner,): (String,) = self
            .bus_proxy()
            .method_call("org.freedesktop.DBus", "GetNameOwner", ("org.kde.KWin",))
            .map_err(|e| e.to_string())?;
        if let Ok(mut slot) = self.shared.reporter.lock() {
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
        let (id,): (i32,) = self
            .proxy("org.kde.KWin", "/Scripting")
            .method_call(
                "org.kde.kwin.Scripting",
                "loadScript",
                (file, plugin_name(slot).as_str()),
            )
            .map_err(|e| e.to_string())?;
        if id < 0 {
            return Err("KWin refused the window script".into());
        }
        // `start` runs every loaded script that is not running yet. Running by
        // object path is unreliable: Plasma 5 numbers scripts by list length,
        // so after an unload a new id can name a script that is still loaded.
        let started: Result<(), dbus::Error> = self
            .proxy("org.kde.KWin", "/Scripting")
            .method_call("org.kde.kwin.Scripting", "start", ());
        started.map_err(|e| e.to_string())
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
function ours(w) {{ return !!w && w.pid === pid; }}
function mothref(w) {{ return !!w && String(w.resourceClass).toLowerCase().indexOf("mothref") >= 0; }}
// Sandboxed builds see a different PID; only then fall back to the window class,
// so a second MothRef instance is never moved by this one.
const sandboxed = !windows().some(ours);
function mine(w) {{ return ours(w) || (sandboxed && mothref(w)); }}
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
const chosen = windows().find(function (w) {{ return id(w) === target; }});
// Popups, dialogs and floating panels of the chosen app count as the app: they
// must not drop the board behind it.
function related(w) {{
    if (id(w) === target) return true;
    if (chosen && w.pid > 0 && w.pid === chosen.pid) return true;
    let parent = w.transientFor;
    for (let depth = 0; parent && depth < 8; depth++) {{
        if (id(parent) === target) return true;
        parent = parent.transientFor;
    }}
    return false;
}}
// Only another app's window or dialog lowers the board; panels, notifications,
// tooltips and the desktop do not.
function foreign(w) {{ return !mine(w) && (w.normalWindow || w.dialog); }}
// The active popup or dialog of the chosen app is lifted over the board while
// it stays active, so the board never hides it; any activation change undoes it.
let lifted = null;
function drop() {{
    if (lifted) {{
        try {{ lifted.keepAbove = false; }} catch (error) {{}}
        lifted = null;
    }}
}}
if (!chosen) {{
    report("TargetClosed", target);
}} else {{
    const follow = function (w) {{
        if (w !== lifted) drop();
        if (!w) return;
        if (related(w)) {{
            raise(true);
            if (id(w) !== target && !w.keepAbove) {{
                w.keepAbove = true;
                lifted = w;
            }}
        }} else if (foreign(w)) {{
            raise(false);
        }}
    }};
    const gone = function (w) {{
        if (!w) return;
        if (w === lifted) lifted = null;
        if (id(w) !== target) return;
        activated.disconnect(follow);
        removed.disconnect(gone);
        drop();
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
    use std::io::BufRead as _;

    fn private(address: &str) -> Connection {
        let mut channel = Channel::open_private(address).unwrap();
        channel.register().unwrap();
        Connection::from(channel)
    }
    /// The worker against a real `dbus-daemon` and a stand-in for KWin's
    /// scripting service, when `dbus-daemon` is installed.
    #[test]
    fn worker_loads_scripts_and_accepts_only_kwin_callbacks() {
        let Ok(mut daemon) = std::process::Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        let mut address = String::new();
        std::io::BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim().to_owned();

        // Stand-in KWin: answers the scripting calls and records the script.
        let (scripts, loaded) = crossbeam_channel::unbounded::<String>();
        let stop = Arc::new(AtomicBool::new(false));
        let kwin_stop = stop.clone();
        let kwin_address = address.clone();
        let (ready, kwin_ready) = crossbeam_channel::bounded(1);
        let kwin = std::thread::spawn(move || {
            let kwin = private(&kwin_address);
            kwin.request_name("org.kde.KWin", false, true, false)
                .unwrap();
            kwin.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |message, bus| {
                    let reply = match message.member().as_deref() {
                        Some("unloadScript") => message.method_return().append1(true),
                        Some("loadScript") => {
                            let (file, _name): (String, String) = message.read2().unwrap();
                            scripts
                                .send(std::fs::read_to_string(file).unwrap())
                                .unwrap();
                            message.method_return().append1(3i32)
                        }
                        Some("start") => message.method_return(),
                        _ => message.error(
                            &"org.freedesktop.DBus.Error.UnknownMethod".into(),
                            c"unknown",
                        ),
                    };
                    let _ = bus.send(reply);
                    true
                }),
            );
            ready.send(()).unwrap();
            while !kwin_stop.load(Ordering::Relaxed) {
                kwin.process(Duration::from_millis(20)).unwrap();
            }
        });
        kwin_ready.recv().unwrap();

        let (woke, wakes) = crossbeam_channel::unbounded();
        let worker_address = address.clone();
        let shared = spawn(
            Arc::new(move || {
                let _ = woke.send(());
            }),
            move || {
                let mut channel = Channel::open_private(&worker_address)?;
                channel.register()?;
                Ok(channel)
            },
        )
        .unwrap();
        let wait = || wakes.recv_timeout(Duration::from_secs(5)).unwrap();
        enqueue(&shared, Request::Probe);
        wait();
        assert!(shared.kwin.load(Ordering::Acquire));
        assert_eq!(shared.gnome.load(Ordering::Acquire), 0);

        enqueue(
            &shared,
            Request::Pick {
                title: "MothRef".into(),
            },
        );
        let script = loaded.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(script.contains("const title = \"MothRef\";"));
        let service = script
            .split("callDBus(\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .unwrap()
            .to_owned();

        // Another client cannot pretend to be KWin.
        let stranger = private(&address);
        let refused: Result<(), dbus::Error> = stranger
            .with_proxy(service.as_str(), CALLBACK_PATH, CALL_TIMEOUT)
            .method_call(CALLBACK_INTERFACE, "Picked", ("{evil}",));
        assert!(refused.is_err());
        assert_eq!(take_picked_from(&shared), None);

        // KWin's report reaches the app and wakes it.
        stop.store(true, Ordering::Relaxed);
        kwin.join().unwrap();
        let kwin = private(&address);
        // The stand-in released the name; claim it again for the callback.
        kwin.request_name("org.kde.KWin", false, true, false)
            .unwrap();
        *shared.reporter.lock().unwrap() = Some(kwin.unique_name().to_string());
        let picked: Result<(), dbus::Error> = kwin
            .with_proxy(service.as_str(), CALLBACK_PATH, CALL_TIMEOUT)
            .method_call(CALLBACK_INTERFACE, "Picked", ("{target}",));
        picked.unwrap();
        wait();
        assert_eq!(take_picked_from(&shared), Some(Some("{target}".into())));
        assert!(shared.error.lock().unwrap().is_none());

        drop((kwin, stranger));
        let _ = daemon.kill();
        let _ = daemon.wait();
    }
    fn take_picked_from(shared: &Shared) -> Option<Option<String>> {
        shared.picked.lock().unwrap().take()
    }
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
