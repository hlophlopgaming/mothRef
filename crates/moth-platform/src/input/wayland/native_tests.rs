// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{io::Write, sync::atomic::AtomicUsize};
use wayland_client::protocol::{wl_compositor, wl_surface};
use wayland_protocols::wp::tablet::zv2::server::{
    zwp_tablet_manager_v2 as tm, zwp_tablet_seat_v2 as ts, zwp_tablet_tool_v2 as tl,
    zwp_tablet_v2 as tt,
};
use wayland_server::protocol::{
    wl_compositor as sc, wl_data_device as sd, wl_data_device_manager as sm, wl_data_offer as so,
    wl_seat as st, wl_surface as ss,
};
use wayland_server::{Client, DataInit, Display, DisplayHandle, GlobalDispatch, New, Resource};
delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_surface::WlSurface);
struct Host;
delegate_noop!(Host: ignore wl_callback::WlCallback);

struct Server {
    surface: Option<ss::WlSurface>,
    device: Option<sd::WlDataDevice>,
    sent: bool,
    mime: &'static str,
    finished: Arc<AtomicUsize>,
}
impl Server {
    fn offer(&mut self, client: &Client, dh: &DisplayHandle) {
        if self.mime == "tablet" {
            return;
        }
        if self.sent {
            return;
        }
        if let (Some(surface), Some(device)) = (&self.surface, &self.device) {
            let offer = client
                .create_resource::<so::WlDataOffer, (), Self>(dh, 3, ())
                .unwrap();
            device.data_offer(&offer);
            offer.offer(self.mime.into());
            offer.source_actions(sm::DndAction::Copy);
            device.enter(42, surface, 12., 34., Some(&offer));
            self.sent = true;
        }
    }
}
macro_rules! global {
    ($ty:ty) => {
        impl GlobalDispatch<$ty, ()> for Server {
            fn bind(
                _: &mut Self,
                _: &DisplayHandle,
                _: &Client,
                resource: New<$ty>,
                _: &(),
                init: &mut DataInit<'_, Self>,
            ) {
                init.init(resource, ());
            }
        }
    };
}
global!(sc::WlCompositor);
global!(st::WlSeat);
global!(sm::WlDataDeviceManager);
global!(tm::ZwpTabletManagerV2);
impl wayland_server::Dispatch<tm::ZwpTabletManagerV2, ()> for Server {
    fn request(
        s: &mut Self,
        client: &Client,
        _: &tm::ZwpTabletManagerV2,
        request: tm::Request,
        _: &(),
        dh: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let tm::Request::GetTabletSeat { tablet_seat, .. } = request {
            let seat = init.init(tablet_seat, ());
            let tablet = client
                .create_resource::<tt::ZwpTabletV2, (), Self>(dh, 1, ())
                .unwrap();
            seat.tablet_added(&tablet);
            tablet.name("Test pen".into());
            tablet.done();
            let pen = client
                .create_resource::<tl::ZwpTabletToolV2, (), Self>(dh, 1, ())
                .unwrap();
            seat.tool_added(&pen);
            pen._type(tl::Type::Pen);
            pen.capability(tl::Capability::Pressure);
            pen.done();
            pen.proximity_in(10, &tablet, s.surface.as_ref().unwrap());
            pen.motion(12.5, 34.25);
            pen.down(11);
            pen.pressure(16384);
            pen.frame(1);
            pen.motion(18.5, 40.25);
            pen.pressure(49151);
            pen.frame(2);
            pen.up();
            pen.proximity_out();
            pen.frame(3);
            pen.removed();
            tablet.removed();
        }
    }
}
impl wayland_server::Dispatch<sc::WlCompositor, ()> for Server {
    fn request(
        s: &mut Self,
        client: &Client,
        _: &sc::WlCompositor,
        request: sc::Request,
        _: &(),
        dh: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let sc::Request::CreateSurface { id } = request {
            s.surface = Some(init.init(id, ()));
            s.offer(client, dh);
        }
    }
}
impl wayland_server::Dispatch<sm::WlDataDeviceManager, ()> for Server {
    fn request(
        s: &mut Self,
        client: &Client,
        _: &sm::WlDataDeviceManager,
        request: sm::Request,
        _: &(),
        dh: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let sm::Request::GetDataDevice { id, .. } = request {
            s.device = Some(init.init(id, ()));
            s.offer(client, dh);
        }
    }
}
impl wayland_server::Dispatch<so::WlDataOffer, ()> for Server {
    fn request(
        s: &mut Self,
        _: &Client,
        offer: &so::WlDataOffer,
        request: so::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        match request {
            so::Request::Accept { serial, mime_type } => {
                assert_eq!(serial, 42);
                assert_eq!(
                    mime_type.as_deref(),
                    (s.mime == URI_LIST).then_some(URI_LIST)
                );
            }
            so::Request::SetActions {
                dnd_actions,
                preferred_action,
            } => {
                if s.mime == URI_LIST {
                    assert_eq!(dnd_actions, WEnum::Value(sm::DndAction::Copy));
                    assert_eq!(preferred_action, WEnum::Value(sm::DndAction::Copy));
                    offer.action(sm::DndAction::Copy);
                    s.device.as_ref().unwrap().drop();
                }
                s.device.as_ref().unwrap().leave();
            }
            so::Request::Receive { mime_type, fd } => {
                assert_eq!(mime_type, URI_LIST);
                std::fs::File::from(fd).write_all(b"# Native drop\r\nfile:///tmp/first%20image.png\r\nfile:///tmp/second.png\r\n").unwrap();
            }
            so::Request::Finish => {
                s.finished.fetch_add(1, Ordering::Release);
            }
            so::Request::Destroy => {
                s.finished.fetch_add(10, Ordering::Release);
            }
            _ => (),
        }
    }
}
macro_rules! ignore_requests {
    ($ty:ty) => {
        impl wayland_server::Dispatch<$ty, ()> for Server {
            fn request(
                _: &mut Self,
                _: &Client,
                _: &$ty,
                _: <$ty as Resource>::Request,
                _: &(),
                _: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
            }
        }
    };
}
ignore_requests!(st::WlSeat);
ignore_requests!(sd::WlDataDevice);
ignore_requests!(ss::WlSurface);
ignore_requests!(ts::ZwpTabletSeatV2);
ignore_requests!(tt::ZwpTabletV2);
ignore_requests!(tl::ZwpTabletToolV2);

fn drop_session(mime: &'static str) -> (Vec<Event>, usize) {
    let (client_socket, server_socket) = UnixStream::pair().unwrap();
    let mut server = Display::<Server>::new().unwrap();
    let mut dh = server.handle();
    dh.create_global::<Server, sc::WlCompositor, _>(1, ());
    dh.create_global::<Server, st::WlSeat, _>(7, ());
    dh.create_global::<Server, sm::WlDataDeviceManager, _>(3, ());
    if mime == "tablet" {
        dh.create_global::<Server, tm::ZwpTabletManagerV2, _>(1, ());
    }
    dh.insert_client(server_socket, Arc::new(())).unwrap();
    drop(dh);
    let finished = Arc::new(AtomicUsize::new(0));
    let status = finished.clone();
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut state = Server {
            surface: None,
            device: None,
            sent: false,
            mime,
            finished: status,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !stopping.load(Ordering::Acquire) && Instant::now() < deadline {
            server.dispatch_clients(&mut state).unwrap();
            server.flush_clients().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    let host = Connection::from_socket(client_socket).unwrap();
    let host_stop = Arc::new(AtomicBool::new(false));
    let host_stopping = host_stop.clone();
    let host_connection = host.clone();
    let (host_queue_send, host_queue_recv) = bounded(1);
    // winit concurrently reads this same display on its own queue.
    let host_reader = std::thread::spawn(move || {
        let mut queue = host_connection.new_event_queue::<Host>();
        host_queue_send.send(queue.handle()).unwrap();
        while !host_stopping.load(Ordering::Acquire) {
            if queue.blocking_dispatch(&mut Host).is_err() {
                break;
            }
        }
    });
    let host_queue = host_queue_recv.recv().unwrap();
    // SAFETY: host remains alive until the guest queue and transfer worker are dropped.
    let conn = Connection::from_backend(unsafe {
        wayland_backend::client::Backend::from_foreign_display(host.backend().display_ptr())
    });
    let mut queue = conn.new_event_queue::<State>();
    let qh = queue.handle();
    let registry = conn.display().get_registry(&qh, ());
    let compositor = registry.bind::<wl_compositor::WlCompositor, _, _>(1, 1, &qh, ());
    let surface = compositor.create_surface(&qh, ());
    let (send, events) = bounded(256);
    // The fixture host replaces winit's owned display lifetime guarantee.
    let (_shutdown, shutting_down) = bounded(0);
    let mut state = State::new(
        surface.id().as_ptr() as usize,
        send,
        Arc::new(|| {}),
        conn.clone(),
        None,
        shutting_down,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while (if mime == "tablet" {
        events.len() < 4
    } else {
        finished.load(Ordering::Acquire) < if mime == URI_LIST { 11 } else { 10 }
    }) && Instant::now() < deadline
    {
        queue.roundtrip(&mut state).unwrap();
    }
    drop(_shutdown);
    // The transfer worker wakes a reader even when the compositor is idle.
    queue.blocking_dispatch(&mut state).unwrap();
    state.cleanup(&conn);
    drop(state);
    conn.flush().unwrap();
    let result: Vec<_> = events.try_iter().collect();
    host_stop.store(true, Ordering::Release);
    host.display().sync(&host_queue, ());
    host.flush().unwrap();
    host_reader.join().unwrap();
    stop.store(true, Ordering::Release);
    thread.join().unwrap();
    drop(queue);
    drop(conn);
    drop(host);
    (result, finished.load(Ordering::Acquire))
}
#[test]
fn native_wayland_tablet_keeps_kind_pressure_logical_coordinates_and_cancellation() {
    let (events, _) = drop_session("tablet");
    let pointers: Vec<_> = events
        .into_iter()
        .map(|e| match e {
            Event::Pointer(p) => p,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(pointers.len(), 4);
    assert!(
        pointers
            .iter()
            .all(|p| p.kind == Kind::Pen && p.id == pointers[0].id)
    );
    assert_eq!(
        pointers.iter().map(|p| p.phase).collect::<Vec<_>>(),
        vec![Phase::Down, Phase::Move, Phase::Up, Phase::Cancel]
    );
    assert_eq!(pointers[0].position, [12.5, 34.25]);
    assert!((pointers[0].pressure - 0.25).abs() < 0.0001);
    assert!((pointers[1].pressure - 0.75).abs() < 0.0001);
    assert_eq!(pointers[2].pressure, 0.);
    assert_eq!(pointers[3].pressure, 0.);
}
#[test]
fn native_wayland_drop_negotiates_copy_reads_fd_and_finishes_after_leave() {
    let (events, status) = drop_session(URI_LIST);
    let paths: Vec<_> = events
        .into_iter()
        .map(|e| match e {
            Event::Drop(path) => path,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        paths,
        vec![
            PathBuf::from("/tmp/first image.png"),
            PathBuf::from("/tmp/second.png")
        ]
    );
    assert_eq!(status, 11);
}
#[test]
fn native_wayland_rejects_unsupported_offer_without_receiving_or_finishing() {
    let (events, status) = drop_session("text/plain");
    assert!(events.is_empty());
    assert_eq!(status, 10);
}
