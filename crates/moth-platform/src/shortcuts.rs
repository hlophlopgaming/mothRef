// SPDX-License-Identifier: GPL-3.0-or-later
//! Fixed, opt-in recovery keys; native registrations never poll the main loop.
use std::sync::Arc;
use winit::window::Window;
/// Matches the shared action inventory without depending on the UI crate.
#[derive(Clone, Copy, Debug)]
pub enum Shortcut {
    /// Toggle the board overlay.
    Overlay,
    /// Restore or disable mouse input.
    ClickThrough,
    /// Increase whole-window opacity.
    OpacityUp,
    /// Decrease whole-window opacity.
    OpacityDown,
}
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
const ACTIONS: [Shortcut; 4] = [
    Shortcut::Overlay,
    Shortcut::ClickThrough,
    Shortcut::OpacityUp,
    Shortcut::OpacityDown,
];
/// Compositors which forbid native key grabs use the interactive Over controller.
pub(crate) fn available(window: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window.window_handle().is_ok_and(|h| {
        matches!(
            h.as_raw(),
            RawWindowHandle::Xlib(_)
                | RawWindowHandle::Xcb(_)
                | RawWindowHandle::Win32(_)
                | RawWindowHandle::AppKit(_)
        )
    })
}
/// Owns all registrations so disabling the preference releases every key.
pub struct Shortcuts {
    #[cfg(target_os = "linux")]
    _shutdown: std::os::unix::net::UnixStream,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    _manager: global_hotkey::GlobalHotKeyManager,
    /// Worker registration failures are reported without blocking a frame.
    pub errors: crossbeam_channel::Receiver<String>,
}
impl Shortcuts {
    /// Must be created on the desktop event-loop thread on Windows and macOS.
    pub fn new(
        window: &Window,
        wake: Arc<dyn Fn(Option<Shortcut>) + Send + Sync>,
    ) -> Result<Self, String> {
        if !available(window) {
            return Err("System shortcuts are unavailable on this compositor".into());
        }
        let (errors, receiver) = crossbeam_channel::bounded(1);
        #[cfg(target_os = "linux")]
        {
            let (shutdown, quit) =
                std::os::unix::net::UnixStream::pair().map_err(|e| e.to_string())?;
            std::thread::Builder::new()
                .name("global-shortcuts".into())
                .spawn(move || {
                    if let Err(error) = x11(quit, wake.clone()) {
                        let _ = errors.try_send(error);
                        wake(None);
                    }
                })
                .map_err(|e| e.to_string())?;
            Ok(Self {
                _shutdown: shutdown,
                errors: receiver,
            })
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            use global_hotkey::{
                GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
                hotkey::{Code, HotKey, Modifiers},
            };
            let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
            let keys = [Code::KeyT, Code::KeyC, Code::ArrowUp, Code::ArrowDown].map(|code| {
                HotKey::new(
                    Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
                    code,
                )
            });
            manager.register_all(&keys).map_err(|e| e.to_string())?;
            GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
                if event.state == HotKeyState::Pressed
                    && let Some(index) = keys.iter().position(|key| key.id() == event.id)
                {
                    wake(Some(ACTIONS[index]));
                }
            }));
            drop(errors);
            Ok(Self {
                _manager: manager,
                errors: receiver,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            let _ = (errors, receiver, wake);
            Err("System shortcuts are unavailable".into())
        }
    }
}

#[cfg(target_os = "linux")]
fn x11(
    quit: std::os::unix::net::UnixStream,
    wake: Arc<dyn Fn(Option<Shortcut>) + Send + Sync>,
) -> Result<(), String> {
    use rustix::event::{PollFd, PollFlags, poll};
    use x11rb::{
        connection::Connection,
        protocol::{
            Event,
            xproto::{ConnectionExt, GrabMode, ModMask},
        },
    };
    let run = || -> Result<(), Box<dyn std::error::Error>> {
        let (connection, screen) = x11rb::connect(None)?;
        let setup = connection.setup();
        let root = setup.roots[screen].root;
        let count = setup
            .max_keycode
            .checked_sub(setup.min_keycode)
            .and_then(|count| count.checked_add(1))
            .ok_or("Invalid X11 keycode range")?;
        let mapping = connection
            .get_keyboard_mapping(setup.min_keycode, count)?
            .reply()?;
        // shortcut: grabs use this keymap snapshot, rebuild them if runtime remapping is required.
        let keys = keycodes(
            setup.min_keycode,
            count,
            mapping.keysyms_per_keycode,
            &mapping.keysyms,
        )?;
        let modifiers = ModMask::CONTROL | ModMask::M1 | ModMask::SHIFT;
        // NumLock's modifier is derived from the server, rather than assuming Mod2.
        let modifiers_map = connection.get_modifier_mapping()?.reply()?;
        let mut ignored = u16::from(ModMask::LOCK);
        if modifiers_map.keycodes.len() < 8 || modifiers_map.keycodes.len() % 8 != 0 {
            return Err("Invalid X11 modifier mapping".into());
        }
        for (modifier, codes) in modifiers_map
            .keycodes
            .chunks(modifiers_map.keycodes.len() / 8)
            .enumerate()
        {
            for code in codes {
                if *code < setup.min_keycode {
                    continue;
                }
                let offset =
                    (*code - setup.min_keycode) as usize * mapping.keysyms_per_keycode as usize;
                if mapping
                    .keysyms
                    .get(offset..offset + mapping.keysyms_per_keycode as usize)
                    .is_some_and(|codes| codes.contains(&0xff7f))
                {
                    ignored |= 1 << modifier;
                }
            }
        }
        for key in keys {
            for extra in 0u16..=255 {
                if extra & !ignored == 0 {
                    connection
                        .grab_key(
                            false,
                            root,
                            modifiers | ModMask::from(extra),
                            key,
                            GrabMode::ASYNC,
                            GrabMode::ASYNC,
                        )?
                        .check()?;
                }
            }
        }
        use x11rb::protocol::xkb;
        let repeat = xkb::ConnectionExt::xkb_per_client_flags(
            &connection,
            xkb::ID::USE_CORE_KBD.into(),
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT,
            0u32.into(),
            0u32.into(),
            0u32.into(),
        )?
        .reply()?;
        if u32::from(repeat.value) & u32::from(xkb::PerClientFlag::DETECTABLE_AUTO_REPEAT) == 0 {
            return Err("X11 cannot suppress repeated shortcut toggles".into());
        }
        connection.flush()?;
        let mut pressed = std::collections::HashSet::new();
        loop {
            while let Some(event) = connection.poll_for_event()? {
                if let Event::KeyRelease(event) = &event {
                    pressed.remove(&event.detail);
                }
                if let Event::KeyPress(event) = event
                    && pressed.insert(event.detail)
                    && u16::from(event.state) & !ignored & 255 == u16::from(modifiers)
                    && let Some(index) = keys.iter().position(|key| *key == event.detail)
                {
                    wake(Some(ACTIONS[index]));
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
                return Err("X11 shortcut connection closed".into());
            }
        }
    };
    run().map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn keycodes(first: u8, count: u8, width: u8, symbols: &[u32]) -> Result<[u8; 4], String> {
    if width == 0 || count == 0 || symbols.len() != count as usize * width as usize {
        return Err("Invalid X11 keyboard mapping".into());
    }
    let mut keys = [0; 4];
    for (index, symbol) in [0x74, 0x63, 0xff52, 0xff54].iter().enumerate() {
        let offset = symbols
            .chunks(width as usize)
            .position(|row| row.contains(symbol))
            .ok_or("Shortcut key is unavailable")?;
        keys[index] = u8::try_from(offset)
            .ok()
            .and_then(|offset| first.checked_add(offset))
            .ok_or("Invalid X11 keycode range")?;
    }
    Ok(keys)
}
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn keyboard_mapping_rejects_missing_and_invalid_server_data() {
        assert_eq!(
            keycodes(10, 4, 2, &[0x74, 0x54, 0x63, 0x43, 0xff52, 0, 0xff54, 0]).unwrap(),
            [10, 11, 12, 13]
        );
        for (first, count, width, symbols) in [
            (10, 0, 0, vec![]),
            (10, 4, 0, vec![]),
            (10, 4, 2, vec![0x74]),
            (10, 4, 1, vec![0; 4]),
            (254, 4, 1, vec![0x74, 0x63, 0xff52, 0xff54]),
        ] {
            assert!(keycodes(first, count, width, &symbols).is_err());
        }
    }
}
