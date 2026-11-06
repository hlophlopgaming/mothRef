// SPDX-License-Identifier: GPL-3.0-or-later
//! Colors from anywhere on the screen, for the color picker.
//!
//! Two ways, by what the system allows:
//! - **Drag** (X11, Windows): while the picker's button is held outside the window, a
//!   worker reads the pixel under the pointer, so the color follows the pointer.
//! - **Interactive** (Linux through the desktop portal, which is the only way on
//!   Wayland; macOS through `NSColorSampler`): the system shows its own picker and
//!   answers with the chosen color.
//!
//! Screen reads never happen on the event loop: the worker owns the display
//! connection; the macOS sampler is only started on the main thread and answers there.
use std::io;

/// What the color picker can offer on this system and window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Reading pixels under the pointer while the button is held outside the window.
    pub drag: bool,
    /// A system color picker started from a button.
    pub interactive: bool,
}

/// What a window can do: Wayland forbids reading the screen, so only the portal remains.
pub fn capabilities(window: &winit::window::Window) -> Capabilities {
    let wayland = crate::wayland(window);
    Capabilities {
        drag: cfg!(target_os = "windows") || (cfg!(target_os = "linux") && !wayland),
        interactive: cfg!(any(target_os = "linux", target_os = "macos")),
    }
}

/// A color the picker produced.
#[derive(Debug)]
pub enum Picked {
    /// The pixel under the held pointer, outside the window (drag).
    Hover([u8; 3]),
    /// The system picker answered: a color, or `None` when it was cancelled.
    Chosen(io::Result<Option<[u8; 3]>>),
}

enum Job {
    At([i32; 2]),
    #[cfg(target_os = "linux")]
    Interactive,
}

/// Owns the worker that reads the screen; created when the picker is first used.
pub struct Sampler {
    jobs: crossbeam_channel::Sender<Job>,
    /// Hover samples and answers, newest last; drained by the UI.
    pub results: crossbeam_channel::Receiver<Picked>,
    #[cfg(target_os = "macos")]
    answers: crossbeam_channel::Sender<Picked>,
    #[cfg(target_os = "macos")]
    wake: std::sync::Arc<dyn Fn() + Send + Sync>,
}

impl Sampler {
    /// Starts the worker; `wake` redraws the app when a color arrives.
    pub fn new(wake: std::sync::Arc<dyn Fn() + Send + Sync>) -> io::Result<Self> {
        // One pending position is enough: a newer one replaces what was not read yet.
        let (jobs, inbox) = crossbeam_channel::bounded::<Job>(1);
        let (answers, results) = crossbeam_channel::bounded(16);
        let worker_answers = answers.clone();
        let worker_wake = wake.clone();
        std::thread::Builder::new()
            .name("screen-color".into())
            .spawn(move || {
                let mut screen = backend::Screen::new();
                while let Ok(job) = inbox.recv() {
                    let picked =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
                            // Nothing readable under the pointer (another app's Wayland
                            // window, a gap) just shows no color.
                            Job::At(position) => screen.at(position).ok().map(Picked::Hover),
                            #[cfg(target_os = "linux")]
                            Job::Interactive => Some(Picked::Chosen(backend::portal_pick())),
                        }))
                        .unwrap_or_else(|_| {
                            Some(Picked::Chosen(Err(io::Error::other(
                                "color picker panicked",
                            ))))
                        });
                    if let Some(picked) = picked {
                        if worker_answers.try_send(picked).is_err() {
                            continue;
                        }
                        worker_wake();
                    }
                }
            })?;
        #[cfg(not(target_os = "macos"))]
        drop((answers, wake));
        Ok(Self {
            jobs,
            results,
            #[cfg(target_os = "macos")]
            answers,
            #[cfg(target_os = "macos")]
            wake,
        })
    }
    /// Asks for the pixel at a physical screen position; skipped while one is pending.
    pub fn sample(&self, position: [i32; 2]) {
        let _ = self.jobs.try_send(Job::At(position));
    }
    /// Starts the system picker. On macOS this must run on the main thread, which is
    /// where the app calls it; elsewhere the worker waits for the portal's answer.
    pub fn interactive(&self) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.jobs
                .send(Job::Interactive)
                .map_err(|_| io::Error::other("color picker stopped"))
        }
        #[cfg(target_os = "macos")]
        {
            backend::sampler(self.answers.clone(), self.wake.clone())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

#[cfg(target_os = "linux")]
mod backend {
    use std::{io, time::Duration};
    use x11rb::{
        connection::Connection as _,
        protocol::xproto::{ConnectionExt as _, ImageFormat},
    };

    /// The X connection, opened on first use and kept for the next samples.
    #[derive(Default)]
    pub(super) struct Screen {
        x11: Option<(x11rb::rust_connection::RustConnection, u32)>,
    }
    impl Screen {
        pub(super) fn new() -> Self {
            Self::default()
        }
        pub(super) fn at(&mut self, [x, y]: [i32; 2]) -> io::Result<[u8; 3]> {
            if self.x11.is_none() {
                let (connection, screen) = x11rb::connect(None).map_err(io::Error::other)?;
                let root = connection.setup().roots[screen].root;
                self.x11 = Some((connection, root));
            }
            let Some((connection, root)) = &self.x11 else {
                return Err(io::ErrorKind::NotConnected.into());
            };
            let (Ok(x), Ok(y)) = (i16::try_from(x), i16::try_from(y)) else {
                return Err(io::ErrorKind::InvalidInput.into());
            };
            match read(connection, *root, x, y) {
                Ok(rgb) => Ok(rgb),
                // Rootless servers (XWayland) refuse reading the root window; the
                // top-level window under the point can still be read.
                Err(_) => {
                    let found = connection
                        .translate_coordinates(*root, *root, x, y)
                        .map_err(io::Error::other)?
                        .reply()
                        .map_err(io::Error::other)?;
                    if found.child == x11rb::NONE {
                        return Err(io::Error::other("no window under the pointer"));
                    }
                    let local = connection
                        .translate_coordinates(*root, found.child, x, y)
                        .map_err(io::Error::other)?
                        .reply()
                        .map_err(io::Error::other)?;
                    read(connection, found.child, local.dst_x, local.dst_y)
                }
            }
        }
    }
    fn read(
        connection: &x11rb::rust_connection::RustConnection,
        drawable: u32,
        x: i16,
        y: i16,
    ) -> io::Result<[u8; 3]> {
        let reply = connection
            .get_image(ImageFormat::Z_PIXMAP, drawable, x, y, 1, 1, !0)
            .map_err(io::Error::other)?
            .reply()
            .map_err(io::Error::other)?;
        // 24- and 32-bit visuals store a pixel as little-endian BGRX.
        match (reply.depth, reply.data.as_slice()) {
            (24 | 32, [b, g, r, ..]) => Ok([*r, *g, *b]),
            _ => Err(io::Error::other("unsupported screen depth")),
        }
    }

    /// `org.freedesktop.portal.Screenshot.PickColor`: the desktop shows its picker and
    /// answers on a Request object; the handle token lets us listen before calling.
    pub(super) fn portal_pick() -> io::Result<Option<[u8; 3]>> {
        use dbus::{
            arg::{PropMap, RefArg, Variant},
            blocking::Connection,
            message::MatchRule,
        };
        const WAIT: Duration = Duration::from_secs(300);
        let connection = Connection::new_session().map_err(io::Error::other)?;
        let sender = connection
            .unique_name()
            .trim_start_matches(':')
            .replace('.', "_");
        let token = format!("mothref_{}", uuid::Uuid::now_v7().simple());
        let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        let answer = std::sync::Arc::new(std::sync::Mutex::new(None));
        let slot = answer.clone();
        let rule = MatchRule::new_signal("org.freedesktop.portal.Request", "Response")
            .with_path(path.clone());
        connection
            .add_match(rule, move |(response, results): (u32, PropMap), _, _| {
                let color = (response == 0)
                    .then(|| results.get("color"))
                    .flatten()
                    .and_then(|color| {
                        let mut parts = color.0.as_iter()?.filter_map(|part| part.as_f64());
                        let mut channel =
                            || parts.next().map(|v| (v.clamp(0., 1.) * 255.).round() as u8);
                        Some([channel()?, channel()?, channel()?])
                    });
                if let Ok(mut slot) = slot.lock() {
                    *slot = Some(color);
                }
                false
            })
            .map_err(io::Error::other)?;
        let mut options = PropMap::new();
        options.insert("handle_token".into(), Variant(Box::new(token)));
        let proxy = connection.with_proxy(
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            Duration::from_secs(10),
        );
        let (_handle,): (dbus::Path,) = proxy
            .method_call(
                "org.freedesktop.portal.Screenshot",
                "PickColor",
                ("", options),
            )
            .map_err(|error| {
                io::Error::other(format!("the desktop offers no color picker ({error})"))
            })?;
        let started = std::time::Instant::now();
        while started.elapsed() < WAIT {
            connection
                .process(Duration::from_millis(250))
                .map_err(io::Error::other)?;
            if let Some(color) = answer.lock().ok().and_then(|mut slot| slot.take()) {
                return Ok(color);
            }
        }
        Ok(None)
    }
}

#[cfg(target_os = "windows")]
mod backend {
    use std::io;
    pub(super) struct Screen;
    impl Screen {
        pub(super) fn new() -> Self {
            Self
        }
        pub(super) fn at(&mut self, [x, y]: [i32; 2]) -> io::Result<[u8; 3]> {
            use windows_sys::Win32::Graphics::Gdi::{CLR_INVALID, GetDC, GetPixel, ReleaseDC};
            // SAFETY: a null window asks for the whole-screen DC, released below on the
            // same thread; GetPixel only reads through it.
            let color = unsafe {
                let dc = GetDC(std::ptr::null_mut());
                if dc.is_null() {
                    return Err(io::Error::last_os_error());
                }
                let color = GetPixel(dc, x, y);
                ReleaseDC(std::ptr::null_mut(), dc);
                color
            };
            if color == CLR_INVALID {
                return Err(io::Error::other("no pixel at this position"));
            }
            // COLORREF is 0x00BBGGRR.
            Ok([color as u8, (color >> 8) as u8, (color >> 16) as u8])
        }
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use super::Picked;
    use std::io;
    pub(super) struct Screen;
    impl Screen {
        pub(super) fn new() -> Self {
            Self
        }
        pub(super) fn at(&mut self, _: [i32; 2]) -> io::Result<[u8; 3]> {
            // Reading the screen needs a recording permission; the sampler needs none.
            Err(io::ErrorKind::Unsupported.into())
        }
    }
    /// Shows `NSColorSampler`; AppKit calls the handler on the main thread.
    pub(super) fn sampler(
        answers: crossbeam_channel::Sender<Picked>,
        wake: std::sync::Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<()> {
        use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};
        let handler = block2::RcBlock::new(move |color: *mut NSColor| {
            // SAFETY: AppKit passes a valid color or nil for the duration of the call.
            let color = unsafe { color.as_ref() };
            let rgb = color
                .and_then(|color| color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()))
                .map(|color| {
                    let channel = |v: f64| (v.clamp(0., 1.) * 255.).round() as u8;
                    [
                        channel(color.redComponent()),
                        channel(color.greenComponent()),
                        channel(color.blueComponent()),
                    ]
                });
            let _ = answers.try_send(Picked::Chosen(Ok(rgb)));
            wake();
        });
        let sampler = NSColorSampler::new();
        // SAFETY: called on the main thread (the app's event loop); the sampler keeps
        // itself and the handler alive until the session ends.
        unsafe { sampler.showSamplerWithSelectionHandler(&handler) };
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod backend {
    use std::io;
    pub(super) struct Screen;
    impl Screen {
        pub(super) fn new() -> Self {
            Self
        }
        pub(super) fn at(&mut self, _: [i32; 2]) -> io::Result<[u8; 3]> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}

/// HEX, RGB and HSV texts of a color, as the picker copies them.
pub fn texts(rgb: [u8; 3]) -> (String, String, String) {
    crate::export::color_texts(rgb)
}
