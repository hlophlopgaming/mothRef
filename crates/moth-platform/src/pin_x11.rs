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
    use rustix::event::{PollFd, PollFlags, poll};
    let (connection, screen) = x11rb::connect(None)?;
    let root = connection.setup().roots[screen].root;
    let active = connection
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
        .reply()?
        .atom;
    let pid = connection.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
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
        raise(&connection, board)?;
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
    loop {
        while let Some(event) = connection.poll_for_event()? {
            match event {
                Event::KeyPress(event) if Some(event.detail) == escape => {
                    let _ = selection.try_send((0, None));
                    return Ok(());
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
                        let _ = selection
                            .try_send((foreground, property(&connection, foreground, pid)?));
                        return Ok(());
                    }
                    if !picking && foreground == target {
                        raise(&connection, board)?;
                        connection.flush()?;
                    }
                }
                _ => {}
            }
        }
        let mut fds = [
            PollFd::new(connection.stream(), PollFlags::IN),
            PollFd::new(&quit, PollFlags::IN),
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
fn raise(connection: &RustConnection, board: u32) -> Result<(), Box<dyn std::error::Error>> {
    // shortcut: normal-layer targets only, extend stacking layers after native qualification.
    connection
        .configure_window(
            board,
            &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
        )?
        .check()?;
    Ok(())
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
