// SPDX-License-Identifier: GPL-3.0-or-later
//! Watch the window manager on a worker; idle pins never poll or reparent user windows.
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
use x11rb::{
    connection::Connection,
    protocol::{Event, xproto::*},
    rust_connection::RustConnection,
};

#[derive(Default)]
pub(super) struct Pin {
    target: u32,
    process: Option<u32>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    valid: Arc<AtomicBool>,
    quit: Option<UnixStream>,
    selection: Option<crossbeam_channel::Receiver<(u32, Option<u32>)>>,
}
impl Pin {
    pub(super) fn new(wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            wake: Some(wake),
            ..Self::default()
        }
    }
    pub(super) fn available(&self) -> bool {
        self.target != 0 && self.valid.load(Ordering::Acquire)
    }
    pub(super) fn enable(&mut self, window: &Window, enabled: bool) -> Result<(), String> {
        self.quit = None;
        self.selection = None;
        if !enabled {
            return Ok(());
        }
        let board = handle(window).ok_or("Window attachment requires X11")?;
        if self.target == 0 {
            return Err("No target window selected".into());
        }
        self.start(board, false)
    }
    pub(super) fn pick(&mut self, window: &Window) -> Option<bool> {
        if self.selection.is_none() {
            let Some(board) = handle(window) else {
                return Some(false);
            };
            if let Err(error) = self.start(board, true) {
                tracing::warn!(%error, "Cannot select X11 window");
                return Some(false);
            }
        }
        match self.selection.as_ref()?.try_recv() {
            Ok((target, process)) => {
                if target != 0 {
                    self.target = target;
                    self.process = process;
                    self.valid.store(true, Ordering::Release);
                }
                self.selection = None;
                self.quit = None;
                Some(target != 0)
            }
            Err(crossbeam_channel::TryRecvError::Empty) => None,
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.selection = None;
                self.quit = None;
                Some(false)
            }
        }
    }
    fn start(&mut self, board: u32, picking: bool) -> Result<(), String> {
        let (quit, worker_quit) = UnixStream::pair().map_err(|e| e.to_string())?;
        let (send, receive) = crossbeam_channel::bounded(1);
        let target = self.target;
        let process = self.process;
        self.valid = Arc::new(AtomicBool::new(self.target != 0));
        let valid = self.valid.clone();
        let wake = self.wake.clone();
        std::thread::Builder::new()
            .name("window-pin".into())
            .spawn(move || {
                if let Err(error) = run(worker_quit, board, target, process, picking, send) {
                    valid.store(false, Ordering::Release);
                    tracing::warn!(%error, "X11 window pin stopped");
                }
                if let Some(wake) = wake {
                    wake();
                }
            })
            .map_err(|e| e.to_string())?;
        self.quit = Some(quit);
        self.selection = picking.then_some(receive);
        Ok(())
    }
}
fn handle(window: &Window) -> Option<u32> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Xlib(h) => u32::try_from(h.window).ok(),
        RawWindowHandle::Xcb(h) => Some(h.window.get()),
        _ => None,
    }
}
fn property(
    connection: &RustConnection,
    window: u32,
    atom: u32,
) -> Result<Option<u32>, Box<dyn std::error::Error>> {
    Ok(connection
        .get_property(false, window, atom, AtomEnum::ANY, 0, 1)?
        .reply()?
        .value32()
        .and_then(|mut values| values.next()))
}
fn run(
    quit: UnixStream,
    board: u32,
    target: u32,
    process: Option<u32>,
    picking: bool,
    selection: crossbeam_channel::Sender<(u32, Option<u32>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    let active = connection
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
        .reply()?
        .atom;
    let pid = connection.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
    let mut layers = Layers {
        root,
        state: connection
            .intern_atom(false, b"_NET_WM_STATE")?
            .reply()?
            .atom,
        above: connection
            .intern_atom(false, b"_NET_WM_STATE_ABOVE")?
            .reply()?
            .atom,
        board,
        raised: false,
        lifted: None,
    };
    connection
        .change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?
        .check()?;
    let escape = if picking {
        let setup = connection.setup();
        let count = setup
            .max_keycode
            .checked_sub(setup.min_keycode)
            .and_then(|count| count.checked_add(1))
            .ok_or("Invalid X11 keycode range")?;
        let map = connection
            .get_keyboard_mapping(setup.min_keycode, count)?
            .reply()?;
        let key = escape_key(setup.min_keycode, map.keysyms_per_keycode, &map.keysyms)
            .ok_or("Escape key is unavailable")?;
        connection
            .grab_key(
                false,
                root,
                ModMask::ANY,
                key,
                GrabMode::ASYNC,
                GrabMode::ASYNC,
            )?
            .check()?;
        Some(key)
    } else {
        if property(&connection, target, pid)? != process {
            return Err("The chosen window has closed".into());
        }
        connection
            .change_window_attributes(
                target,
                &ChangeWindowAttributesAux::new().event_mask(EventMask::STRUCTURE_NOTIFY),
            )?
            .check()?;
        None
    };
    connection.flush()?;
    if picking {
        let foreground = property(&connection, root, active)?.unwrap_or(0);
        let process = if foreground != 0 {
            property(&connection, foreground, pid)?
        } else {
            None
        };
        if foreground != 0 && foreground != board && process != Some(std::process::id()) {
            let _ = selection.try_send((foreground, process));
            return Ok(());
        }
    }
    // Some(false) for the chosen window, Some(true) for another window of its app.
    type Related = Result<Option<bool>, Box<dyn std::error::Error>>;
    let related = |connection: &RustConnection, window: u32| -> Related {
        Ok(if window == target {
            Some(false)
        } else if window != 0 && process.is_some() && property(connection, window, pid)? == process
        {
            Some(true)
        } else {
            None
        })
    };
    let own =
        |connection: &RustConnection, window: u32| -> Result<bool, Box<dyn std::error::Error>> {
            Ok(window == board || property(connection, window, pid)? == Some(std::process::id()))
        };
    if !picking {
        let foreground = property(&connection, root, active)?.unwrap_or(0);
        layers.follow(
            &connection,
            foreground,
            related(&connection, foreground)?,
            foreground == 0 || own(&connection, foreground)?,
        )?;
        connection.flush()?;
    }
    let result = watch(&connection, &quit, |event| {
        match event {
            Event::KeyPress(event) if Some(event.detail) == escape => {
                let _ = selection.try_send((0, None));
                return Ok(true);
            }
            Event::DestroyNotify(event) if event.window == target && !picking => {
                return Err("The chosen window has closed".into());
            }
            Event::PropertyNotify(event) if event.window == root && event.atom == active => {
                let foreground = property(&connection, root, active)?.unwrap_or(0);
                if picking
                    && foreground != 0
                    && foreground != board
                    && property(&connection, foreground, pid)? != Some(std::process::id())
                {
                    let _ =
                        selection.try_send((foreground, property(&connection, foreground, pid)?));
                    return Ok(true);
                }
                if !picking {
                    layers.follow(
                        &connection,
                        foreground,
                        related(&connection, foreground)?,
                        foreground == 0 || own(&connection, foreground)?,
                    )?;
                    connection.flush()?;
                }
            }
            _ => {}
        }
        Ok(false)
    });
    // Leaving the pin must not leave the board, or the app's popup, on top.
    if !picking {
        let _ = layers.release(&connection);
        let _ = connection.flush();
    }
    result
}
/// Handles events until `handle` returns true, the quit descriptor fires or the
/// connection fails; waits in poll() in between, without timers.
fn watch(
    connection: &RustConnection,
    quit: &UnixStream,
    mut handle: impl FnMut(Event) -> Result<bool, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    use rustix::event::{PollFd, PollFlags, poll};
    loop {
        while let Some(event) = connection.poll_for_event()? {
            if handle(event)? {
                return Ok(());
            }
        }
        let mut fds = [
            PollFd::new(connection.stream(), PollFlags::IN),
            PollFd::new(quit, PollFlags::IN),
        ];
        match poll(&mut fds, None) {
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
        if !fds[1].revents().is_empty() {
            return Ok(());
        }
        if fds[0].revents().intersects(PollFlags::ERR | PollFlags::HUP) {
            return Err("X11 connection closed".into());
        }
    }
}
/// Window managers keep a dialog directly above its main window, so the board
/// cannot be restacked between them. Instead it uses the EWMH "above" state
/// while the chosen app is active, and lifts the app's active popup or dialog
/// into the same layer above it until something else is activated. Requests go
/// through the window manager as a pager would send them.
struct Layers {
    root: u32,
    state: u32,
    above: u32,
    board: u32,
    raised: bool,
    lifted: Option<u32>,
}
impl Layers {
    fn follow(
        &mut self,
        connection: &RustConnection,
        foreground: u32,
        related: Option<bool>,
        own: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(lifted) = self.lifted.filter(|&w| w != foreground) {
            self.lifted = None;
            self.set(connection, lifted, false)?;
        }
        if let Some(popup) = related {
            self.raise(connection, true)?;
            // The chosen window stays under the board; only its popups are lifted.
            if popup && self.lifted.is_none() && !self.has_above(connection, foreground)? {
                self.set(connection, foreground, true)?;
                self.lifted = Some(foreground);
            }
        } else if !own {
            self.raise(connection, false)?;
        }
        Ok(())
    }
    fn release(&mut self, connection: &RustConnection) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(lifted) = self.lifted.take() {
            self.set(connection, lifted, false)?;
        }
        self.raise(connection, false)
    }
    fn raise(
        &mut self,
        connection: &RustConnection,
        on: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.raised != on {
            self.raised = on;
            self.set(connection, self.board, on)?;
        }
        Ok(())
    }
    fn has_above(
        &self,
        connection: &RustConnection,
        window: u32,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(connection
            .get_property(false, window, self.state, AtomEnum::ATOM, 0, 32)?
            .reply()?
            .value32()
            .is_some_and(|mut states| states.any(|s| s == self.above)))
    }
    fn set(
        &self,
        connection: &RustConnection,
        window: u32,
        on: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        const PAGER: u32 = 2;
        let event = ClientMessageEvent::new(
            32,
            window,
            self.state,
            [u32::from(on), self.above, 0, PAGER, 0],
        );
        connection
            .send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )?
            .check()?;
        Ok(())
    }
}
fn escape_key(first: u8, width: u8, symbols: &[u32]) -> Option<u8> {
    if width == 0 || !symbols.len().is_multiple_of(width as usize) {
        return None;
    }
    let index = symbols
        .chunks(width as usize)
        .position(|row| row.contains(&0xff1b))?;
    first.checked_add(u8::try_from(index).ok()?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_key_validates_server_mapping() {
        assert_eq!(escape_key(8, 2, &[0, 0, 0xff1b, 0]), Some(9));
        assert_eq!(escape_key(8, 0, &[]), None);
        assert_eq!(escape_key(8, 2, &[0xff1b]), None);
        assert_eq!(escape_key(255, 1, &[0, 0xff1b]), None);
        assert_eq!(escape_key(8, 1, &[0]), None);
    }
}
