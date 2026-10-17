// SPDX-License-Identifier: GPL-3.0-or-later
//! Supplements winit on its existing connection; never opens a second Wayland client.
use super::{Kind, NativeEvent as Event, Phase, Pointer};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::{
    collections::HashMap,
    io::{self, Read},
    os::{fd::AsFd, unix::net::UnixStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop, event_created_child,
    protocol::{
        wl_callback, wl_data_device, wl_data_device_manager, wl_data_offer, wl_registry, wl_seat,
    },
};
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2 as manager, zwp_tablet_pad_group_v2 as group,
    zwp_tablet_pad_ring_v2 as ring, zwp_tablet_pad_strip_v2 as strip, zwp_tablet_pad_v2 as pad,
    zwp_tablet_seat_v2 as seat, zwp_tablet_tool_v2 as tool, zwp_tablet_v2 as tablet,
};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle},
    window::Window,
};

const LIMIT: usize = 64;
const URI_LIST: &str = "text/uri-list";

pub struct Native {
    pub events: Receiver<Event>,
    stop: Arc<AtomicBool>,
    _shutdown: Sender<()>,
}
impl Native {
    pub fn new(
        window: Arc<Window>,
        owned_display: winit::event_loop::OwnedDisplayHandle,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Option<Self>> {
        let display = match window.display_handle().map_err(io::Error::other)?.as_raw() {
            RawDisplayHandle::Wayland(h) => h.display.as_ptr() as usize,
            _ => return Ok(None),
        };
        let surface = match window.window_handle().map_err(io::Error::other)?.as_raw() {
            RawWindowHandle::Wayland(h) => h.surface.as_ptr() as usize,
            _ => return Ok(None),
        };
        let (send, events) = bounded(256);
        let stop = Arc::new(AtomicBool::new(false));
        let (shutdown, shutting_down) = bounded(0);
        let native = Self {
            events,
            stop: stop.clone(),
            _shutdown: shutdown,
        };
        std::thread::Builder::new()
            .name("wayland-input".into())
            .spawn(move || {
                // SAFETY: the owned display and window outlive every guest proxy and worker.
                let backend = unsafe {
                    wayland_backend::client::Backend::from_foreign_display(display as *mut _)
                };
                let conn = Connection::from_backend(backend);
                let mut queue = conn.new_event_queue();
                let qh = queue.handle();
                let registry = conn.display().get_registry(&qh, ());
                let mut state = match State::new(
                    surface,
                    send.clone(),
                    wake.clone(),
                    conn.clone(),
                    Some(owned_display.clone()),
                    shutting_down,
                ) {
                    Ok(state) => state,
                    Err(error) => {
                        let _ = send.send(Event::Error(error.to_string()));
                        wake();
                        return;
                    }
                };
                while !stop.load(Ordering::Acquire) {
                    if let Err(error) = queue.blocking_dispatch(&mut state) {
                        state.emit(Event::Error(error.to_string()));
                        break;
                    }
                }
                state.cleanup(&conn);
                let _ = conn.backend().destroy_object(&registry.id());
                let _ = conn.flush();
                drop(state);
                drop(queue);
                drop(conn);
                drop(window);
                drop(owned_display);
            })?;
        Ok(Some(native))
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[derive(Default)]
struct Tool {
    position: [f32; 2],
    pressure: f32,
    pressure_capable: bool,
    kind: Option<Kind>,
    focused: bool,
    down: bool,
    phases: std::collections::VecDeque<Phase>,
}
impl Tool {
    fn phase(&mut self, phase: Phase) {
        if self.phases.back() == Some(&Phase::Move) {
            self.phases.pop_back();
        }
        if self.phases.len() < 8 {
            self.phases.push_back(phase);
        } else {
            self.phases.clear();
            self.phases.push_back(Phase::Cancel);
            self.focused = false;
            self.down = false;
        }
    }
    fn cancel(&mut self) {
        self.down = false;
        self.phase(Phase::Cancel);
    }
    fn frame(&mut self, id: u64) -> Option<Pointer> {
        let kind = self.kind?;
        let phase = self.phases.pop_front()?;
        Some(Pointer {
            id,
            kind,
            position: self.position,
            pressure: if matches!(phase, Phase::Up | Phase::Cancel)
                || (phase == Phase::Move && !self.down)
            {
                0.
            } else if self.pressure_capable {
                self.pressure
            } else {
                1.
            },
            phase,
            pan: false,
        })
    }
}
struct Offer {
    proxy: wl_data_offer::WlDataOffer,
    uri: bool,
    copy: bool,
}
struct State {
    surface: usize,
    send: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
    manager: Option<manager::ZwpTabletManagerV2>,
    data_manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    seats: Vec<(
        u32,
        wl_seat::WlSeat,
        Option<seat::ZwpTabletSeatV2>,
        Option<wl_data_device::WlDataDevice>,
    )>,
    tools: HashMap<tool::ZwpTabletToolV2, Tool>,
    tablets: Vec<tablet::ZwpTabletV2>,
    offers: HashMap<wl_data_offer::WlDataOffer, Offer>,
    drags: HashMap<wl_data_device::WlDataDevice, wl_data_offer::WlDataOffer>,
    transfer: Sender<(wl_data_offer::WlDataOffer, UnixStream)>,
    drop_thread: Option<std::thread::JoinHandle<()>>,
}
impl State {
    fn new(
        surface: usize,
        send: Sender<Event>,
        wake: Arc<dyn Fn() + Send + Sync>,
        conn: Connection,
        display: Option<winit::event_loop::OwnedDisplayHandle>,
        shutdown: Receiver<()>,
    ) -> io::Result<Self> {
        let (transfer, requests) = bounded::<(wl_data_offer::WlDataOffer, UnixStream)>(1);
        let notify = wake.clone();
        let output = send.clone();
        let drop_thread = std::thread::Builder::new()
            .name("wayland-drop".into())
            .spawn(move || {
                let _display = display;
                loop {
                    let request = crossbeam_channel::select! {
                        recv(shutdown) -> _ => {
                            // Wake the input reader without polling or doing I/O on the UI thread.
                            let queue = conn.new_event_queue::<State>();
                            conn.display().sync(&queue.handle(), ());
                            let _ = conn.flush();
                            break;
                        }
                        recv(requests) -> request => request,
                    };
                    let Ok((offer, stream)) = request else {
                        break;
                    };
                    match read_uris(stream) {
                        Ok(paths) => {
                            if offer.version() >= 3 {
                                offer.finish();
                            }
                            for path in paths {
                                if output.send(Event::Drop(path)).is_err() {
                                    break;
                                }
                                notify();
                            }
                        }
                        Err(error) => {
                            let _ = output.send(Event::Error(error.to_string()));
                            notify();
                        }
                    }
                    offer.destroy();
                    let _ = conn.flush();
                }
            })?;
        Ok(Self {
            surface,
            send,
            wake,
            manager: None,
            data_manager: None,
            seats: Vec::new(),
            tools: HashMap::new(),
            tablets: Vec::new(),
            offers: HashMap::new(),
            drags: HashMap::new(),
            transfer,
            drop_thread: Some(drop_thread),
        })
    }
    fn emit(&self, event: Event) {
        if let Event::Pointer(p) = &event {
            tracing::trace!(?p, "native pointer");
        }
        if self.send.send(event).is_ok() {
            (self.wake)();
        }
    }
    fn bind_seats(&mut self, qh: &QueueHandle<Self>) {
        for (_, seat, tablet, data) in &mut self.seats {
            if tablet.is_none()
                && let Some(manager) = &self.manager
            {
                *tablet = Some(manager.get_tablet_seat(seat, qh, ()));
            }
            if data.is_none()
                && let Some(manager) = &self.data_manager
            {
                *data = Some(manager.get_data_device(seat, qh, ()));
            }
        }
    }
    fn cleanup(&mut self, conn: &Connection) {
        self.drags.clear();
        for (tool, _) in self.tools.drain() {
            tool.destroy();
        }
        for tablet in self.tablets.drain(..) {
            tablet.destroy();
        }
        for (_, offer) in self.offers.drain() {
            offer.proxy.destroy();
        }
        for (_, seat, tablet, data) in self.seats.drain(..) {
            if let Some(tablet) = tablet {
                tablet.destroy();
            }
            if let Some(data) = data {
                if data.version() >= 2 {
                    data.release();
                } else {
                    let _ = conn.backend().destroy_object(&data.id());
                }
            }
            if seat.version() >= 5 {
                seat.release();
            } else {
                let _ = conn.backend().destroy_object(&seat.id());
            }
        }
        if let Some(manager) = self.data_manager.take() {
            let _ = conn.backend().destroy_object(&manager.id());
        }
        if let Some(manager) = self.manager.take() {
            manager.destroy();
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        let (closed, _) = bounded(0);
        drop(std::mem::replace(&mut self.transfer, closed));
        if let Some(thread) = self.drop_thread.take()
            && thread.join().is_err()
        {
            tracing::error!("Wayland drop worker panicked");
        }
    }
}
fn read_uris(mut stream: UnixStream) -> io::Result<Vec<PathBuf>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    loop {
        stream.set_read_timeout(Some(
            deadline
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(1)),
        ))?;
        let mut block = [0; 4096];
        let n = match stream.read(&mut block) {
            Ok(n) => n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Wayland drop timed out",
                ));
            }
            Err(error) => return Err(error),
        };
        if n == 0 {
            break;
        }
        if bytes.len() + n > 1024 * 1024 {
            return Err(io::Error::other("Wayland URI list exceeds 1 MiB"));
        }
        bytes.extend_from_slice(&block[..n]);
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Wayland drop timed out",
            ));
        }
    }
    let text = std::str::from_utf8(&bytes).map_err(io::Error::other)?;
    let mut paths = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let url = url::Url::parse(line).map_err(io::Error::other)?;
        if url.scheme() != "file"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(io::Error::other("Drop requires plain local file URLs"));
        }
        let path = url
            .to_file_path()
            .map_err(|_| io::Error::other("Drop requires local file URLs"))?;
        use std::os::unix::ffi::OsStrExt;
        if path.as_os_str().as_bytes().contains(&0) {
            return Err(io::Error::other("Drop path contains NUL"));
        }
        if paths.len() == 1024 {
            return Err(io::Error::other("Wayland drop exceeds 1024 files"));
        }
        paths.push(path);
    }
    Ok(paths)
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        s: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_seat" if s.seats.len() < LIMIT => {
                    s.seats.push((
                        name,
                        registry.bind(name, version.min(7), qh, ()),
                        None,
                        None,
                    ));
                }
                "wl_data_device_manager" if s.data_manager.is_none() => {
                    s.data_manager = Some(registry.bind(name, version.min(3), qh, ()))
                }
                "zwp_tablet_manager_v2" if s.manager.is_none() => {
                    s.manager = Some(registry.bind(name, 1, qh, ()))
                }
                _ => (),
            },
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(i) = s.seats.iter().position(|(n, ..)| *n == name) {
                    let (_, seat, tablet, data) = s.seats.swap_remove(i);
                    if let Some(tablet) = tablet {
                        tablet.destroy();
                    }
                    if let Some(data) = data {
                        if let Some(offer) = s.drags.remove(&data) {
                            s.offers.remove(&offer);
                            offer.destroy();
                        }
                        if data.version() >= 2 {
                            data.release();
                        } else {
                            let _ = conn.backend().destroy_object(&data.id());
                        }
                    }
                    if seat.version() >= 5 {
                        seat.release();
                    } else {
                        let _ = conn.backend().destroy_object(&seat.id());
                    }
                }
            }
            _ => (),
        }
        s.bind_seats(qh);
    }
}
impl Dispatch<seat::ZwpTabletSeatV2, ()> for State {
    fn event(
        s: &mut Self,
        _: &seat::ZwpTabletSeatV2,
        event: seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            seat::Event::ToolAdded { id } => {
                if s.tools.len() < LIMIT {
                    s.tools.insert(id, Tool::default());
                } else {
                    id.destroy();
                }
            }
            seat::Event::TabletAdded { id } => {
                if s.tablets.len() < LIMIT {
                    s.tablets.push(id);
                } else {
                    id.destroy();
                }
            }
            seat::Event::PadAdded { id } => id.destroy(),
            _ => (),
        }
    }
    event_created_child!(State, seat::ZwpTabletSeatV2, [0 => (tablet::ZwpTabletV2, ()), 1 => (tool::ZwpTabletToolV2, ()), 2 => (pad::ZwpTabletPadV2, ())]);
}
impl Dispatch<tool::ZwpTabletToolV2, ()> for State {
    fn event(
        s: &mut Self,
        proxy: &tool::ZwpTabletToolV2,
        event: tool::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(t) = s.tools.get_mut(proxy) else {
            return;
        };
        match event {
            tool::Event::Type {
                tool_type: WEnum::Value(kind),
            } => {
                t.kind = Some(match kind {
                    tool::Type::Mouse | tool::Type::Lens => Kind::Mouse,
                    tool::Type::Finger => Kind::Touch,
                    _ => Kind::Pen,
                });
            }
            tool::Event::Capability {
                capability: WEnum::Value(tool::Capability::Pressure),
            } => t.pressure_capable = true,
            tool::Event::ProximityIn { surface, .. } => {
                t.focused = surface.id().as_ptr() as usize == s.surface;
                t.down = false;
                t.pressure = 0.;
            }
            tool::Event::Motion { x, y } if t.focused => {
                t.position = [x as f32, y as f32];
                if t.phases.is_empty() {
                    t.phase(Phase::Move);
                }
            }
            tool::Event::Pressure { pressure } if t.focused => {
                t.pressure = pressure.min(65535) as f32 / 65535.;
                if t.phases.is_empty() {
                    t.phase(Phase::Move);
                }
            }
            tool::Event::Down { .. } if t.focused => {
                t.down = true;
                t.phase(Phase::Down);
            }
            tool::Event::Up if t.focused => {
                t.down = false;
                t.phase(Phase::Up);
            }
            tool::Event::ProximityOut => {
                if t.focused {
                    t.cancel();
                }
                t.focused = false;
                t.down = false;
            }
            tool::Event::Frame { .. } => {
                let mut pointers = Vec::new();
                while let Some(p) = t.frame(u64::from(proxy.id().protocol_id())) {
                    pointers.push(p);
                }
                for p in pointers {
                    s.emit(Event::Pointer(p));
                }
            }
            tool::Event::Removed => {
                if t.focused || t.down {
                    t.cancel();
                }
                let mut pointers = Vec::new();
                while let Some(p) = t.frame(u64::from(proxy.id().protocol_id())) {
                    pointers.push(p);
                }
                for p in pointers {
                    s.emit(Event::Pointer(p));
                }
                s.tools.remove(proxy);
                proxy.destroy();
            }
            _ => (),
        }
    }
}
impl Dispatch<tablet::ZwpTabletV2, ()> for State {
    fn event(
        s: &mut Self,
        proxy: &tablet::ZwpTabletV2,
        event: tablet::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, tablet::Event::Removed) {
            s.tablets.retain(|t| t != proxy);
            proxy.destroy();
        }
    }
}
impl Dispatch<wl_data_offer::WlDataOffer, ()> for State {
    fn event(
        s: &mut Self,
        proxy: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(offer) = s.offers.get_mut(proxy) {
            match event {
                wl_data_offer::Event::Offer { mime_type } if mime_type == URI_LIST => {
                    offer.uri = true
                }
                wl_data_offer::Event::Action {
                    dnd_action: WEnum::Value(action),
                } => offer.copy = action == wl_data_device_manager::DndAction::Copy,
                _ => (),
            }
        }
    }
}
impl Dispatch<wl_data_device::WlDataDevice, ()> for State {
    fn event(
        s: &mut Self,
        device: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { id } => {
                if s.offers.len() < LIMIT {
                    s.offers.insert(
                        id.clone(),
                        Offer {
                            proxy: id,
                            uri: false,
                            copy: false,
                        },
                    );
                } else {
                    id.destroy();
                }
            }
            wl_data_device::Event::Enter {
                serial,
                surface,
                id,
                ..
            } => {
                if let Some(offer) = id.as_ref() {
                    let accept = surface.id().as_ptr() as usize == s.surface
                        && s.offers.get(offer).is_some_and(|o| o.uri);
                    offer.accept(serial, accept.then(|| URI_LIST.into()));
                    if offer.version() >= 3 {
                        let action = if accept {
                            wl_data_device_manager::DndAction::Copy
                        } else {
                            wl_data_device_manager::DndAction::empty()
                        };
                        offer.set_actions(action, action);
                    }
                    if !accept && let Some(offer) = s.offers.get_mut(offer) {
                        offer.uri = false;
                    }
                }
                if let Some(offer) = id {
                    s.drags.insert(device.clone(), offer);
                }
            }
            wl_data_device::Event::Leave => {
                if let Some(offer) = s.drags.remove(device)
                    && s.offers.remove(&offer).is_some()
                {
                    offer.destroy();
                }
            }
            wl_data_device::Event::Drop => {
                if let Some(offer) = s.drags.remove(device) {
                    let supported = s
                        .offers
                        .remove(&offer)
                        .is_some_and(|o| o.uri && (offer.version() < 3 || o.copy));
                    if !supported {
                        offer.destroy();
                        return;
                    }
                    match UnixStream::pair() {
                        Ok((read, write)) => {
                            offer.receive(URI_LIST.into(), write.as_fd());
                            if s.transfer.try_send((offer.clone(), read)).is_err() {
                                offer.destroy();
                                s.emit(Event::Error("Wayland drop worker is busy".into()));
                            }
                        }
                        Err(error) => {
                            offer.destroy();
                            s.emit(Event::Error(error.to_string()));
                        }
                    }
                }
            }
            wl_data_device::Event::Selection { id: Some(offer) } => {
                s.offers.remove(&offer);
                offer.destroy();
            }
            _ => (),
        }
    }
    event_created_child!(State, wl_data_device::WlDataDevice, [0 => (wl_data_offer::WlDataOffer, ())]);
}
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(State: ignore manager::ZwpTabletManagerV2);
impl Dispatch<pad::ZwpTabletPadV2, ()> for State {
    fn event(
        _: &mut Self,
        _: &pad::ZwpTabletPadV2,
        event: pad::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let pad::Event::Group { pad_group } = event {
            pad_group.destroy();
        }
    }
    event_created_child!(State, pad::ZwpTabletPadV2, [0 => (group::ZwpTabletPadGroupV2, ())]);
}
impl Dispatch<group::ZwpTabletPadGroupV2, ()> for State {
    fn event(
        _: &mut Self,
        _: &group::ZwpTabletPadGroupV2,
        event: group::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            group::Event::Ring { ring } => ring.destroy(),
            group::Event::Strip { strip } => strip.destroy(),
            _ => (),
        }
    }
    event_created_child!(State, group::ZwpTabletPadGroupV2, [1 => (ring::ZwpTabletPadRingV2, ()), 2 => (strip::ZwpTabletPadStripV2, ())]);
}
delegate_noop!(State: ignore ring::ZwpTabletPadRingV2);
delegate_noop!(State: ignore strip::ZwpTabletPadStripV2);
delegate_noop!(State: ignore wl_callback::WlCallback);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn native_uri_list_preserves_local_paths_and_rejects_remote_or_malformed_data() {
        fn decode(bytes: &[u8]) -> io::Result<Vec<PathBuf>> {
            let (read, mut write) = UnixStream::pair()?;
            write.write_all(bytes)?;
            drop(write);
            read_uris(read)
        }
        assert_eq!(
            decode(b"# files\r\nfile:///tmp/a%20b.png\r\nfile://localhost/tmp/c.png\r\n").unwrap(),
            vec![PathBuf::from("/tmp/a b.png"), PathBuf::from("/tmp/c.png")]
        );
        for input in [
            b"https://example.com/a.png".as_slice(),
            b"file://other/tmp/a",
            b"file:///tmp/%00",
            b"file:///tmp/a?query",
            b"\xff",
            b"not a URL",
        ] {
            assert!(decode(input).is_err(), "{input:?}");
        }
    }
    #[test]
    fn uri_transfer_has_a_hard_byte_limit_and_empty_transfer_has_no_paths() {
        let (read, mut write) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            let _ = write.write_all(&vec![b'a'; 1024 * 1024 + 1]);
        });
        assert!(read_uris(read).unwrap_err().to_string().contains("1 MiB"));
        writer.join().unwrap();
        let (read, write) = UnixStream::pair().unwrap();
        read.shutdown(std::net::Shutdown::Read).unwrap();
        drop(write);
        // An empty transfer has no paths and never fabricates an import.
        assert!(read_uris(read).unwrap().is_empty());
    }
    #[test]
    fn an_unresponsive_drop_source_times_out() {
        let (read, _source) = UnixStream::pair().unwrap();
        assert_eq!(read_uris(read).unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
    #[test]
    fn tablet_frame_preserves_pressure_and_contact_end_before_proximity_out() {
        let mut tool = Tool {
            focused: true,
            down: true,
            pressure_capable: true,
            kind: Some(Kind::Pen),
            pressure: 0.25,
            phases: [Phase::Down].into(),
            ..Tool::default()
        };
        let p = tool.frame(42).unwrap();
        assert_eq!(
            (p.kind, p.id, p.pressure, p.phase),
            (Kind::Pen, 42, 0.25, Phase::Down)
        );
        assert!(super::super::Gestures::default().update(p).is_none());
        // An up and proximity-out may be delivered in the same protocol frame.
        tool.phase(Phase::Up);
        tool.cancel();
        assert_eq!(tool.frame(42).unwrap().phase, Phase::Up);
        assert_eq!(tool.frame(42).unwrap().phase, Phase::Cancel);
    }
}

#[cfg(test)]
mod native_tests;
