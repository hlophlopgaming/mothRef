// SPDX-License-Identifier: GPL-3.0-or-later
//! Test helper for the desktop stand: one X11 process with a main window "Krita"
//! and a transient "Brush popup" that is shown while the file `popup` exists in
//! the working directory, so popups of a chosen app can be exercised.
use x11rb::connection::Connection;
use x11rb::protocol::xproto::*;
use x11rb::wrapper::ConnectionExt as _;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (c, screen) = x11rb::connect(None)?;
    let root = c.setup().roots[screen].root;
    let white = c.setup().roots[screen].white_pixel;
    let atom = |n: &[u8]| -> Result<u32, Box<dyn std::error::Error>> {
        Ok(c.intern_atom(false, n)?.reply()?.atom)
    };
    let (pid, name, utf8) = (atom(b"_NET_WM_PID")?, atom(b"_NET_WM_NAME")?, atom(b"UTF8_STRING")?);
    let make = |title: &str, x: i16, w: u16, h: u16| -> Result<u32, Box<dyn std::error::Error>> {
        let id = c.generate_id()?;
        c.create_window(0, id, root, x, 60, w, h, 0, WindowClass::INPUT_OUTPUT, 0,
            &CreateWindowAux::new().background_pixel(white))?;
        c.change_property8(PropMode::REPLACE, id, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())?;
        c.change_property8(PropMode::REPLACE, id, name, utf8, title.as_bytes())?;
        c.change_property8(PropMode::REPLACE, id, AtomEnum::WM_CLASS, AtomEnum::STRING, b"krita\0krita\0")?;
        c.change_property32(PropMode::REPLACE, id, pid, AtomEnum::CARDINAL, &[std::process::id()])?;
        Ok(id)
    };
    let main = make("Krita", 40, 900, 700)?;
    c.map_window(main)?;
    let popup = make("Brush popup", 300, 300, 200)?;
    c.change_property32(PropMode::REPLACE, popup, AtomEnum::WM_TRANSIENT_FOR, AtomEnum::WINDOW, &[main])?;
    c.flush()?;
    let mut shown = false;
    loop {
        let want = std::path::Path::new("popup").exists();
        if want != shown {
            if want { c.map_window(popup)?; } else { c.unmap_window(popup)?; }
            c.flush()?;
            shown = want;
        }
        while c.poll_for_event()?.is_some() {}
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
