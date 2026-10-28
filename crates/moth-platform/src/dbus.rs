// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal session-bus client: method calls with basic arguments and replies to
//! incoming calls. Compositor integration needs nothing more, and a full D-Bus
//! library would bring an async runtime into the app for a handful of messages.
//!
//! Everything read from the socket is untrusted: lengths are bounded and checked,
//! and malformed messages end the connection instead of panicking.
use std::io::{self, BufRead, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

/// Messages beyond this are not something a compositor sends us.
const MAX_MESSAGE: usize = 1 << 20;
const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const NO_REPLY_EXPECTED: u8 = 1;

/// The basic argument types MothRef sends and accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Arg {
    Str(String),
    Bool(bool),
    I32(i32),
    U32(u32),
}
impl Arg {
    fn code(&self) -> char {
        match self {
            Arg::Str(_) => 's',
            Arg::Bool(_) => 'b',
            Arg::I32(_) => 'i',
            Arg::U32(_) => 'u',
        }
    }
}
impl From<&str> for Arg {
    fn from(value: &str) -> Self {
        Arg::Str(value.to_owned())
    }
}
impl From<bool> for Arg {
    fn from(value: bool) -> Self {
        Arg::Bool(value)
    }
}

/// Errors the caller can tell apart: a refused call versus a broken bus.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{name}: {message}")]
    Remote { name: String, message: String },
    #[error("Malformed D-Bus message")]
    Malformed,
    #[error("Unexpected D-Bus reply")]
    Unexpected,
    #[error("No reply from the session bus")]
    Timeout,
}

/// A decoded message; unsupported body types leave `args` empty.
#[derive(Clone, Debug, Default)]
pub(crate) struct Message {
    pub kind: u8,
    pub flags: u8,
    pub serial: u32,
    pub reply_serial: Option<u32>,
    pub path: Option<String>,
    pub interface: Option<String>,
    pub member: Option<String>,
    pub error_name: Option<String>,
    pub destination: Option<String>,
    pub sender: Option<String>,
    pub args: Vec<Arg>,
}

/// Answers an incoming method call; `Err` becomes a D-Bus error reply.
pub(crate) type Handler = Box<dyn Fn(&Message) -> Result<Vec<Arg>, String> + Send>;

/// A session-bus connection. A reader thread blocks on the socket, so incoming
/// calls are answered without polling while the owner waits on its own queue.
pub(crate) struct Bus {
    writer: Arc<Mutex<UnixStream>>,
    serial: Arc<AtomicU32>,
    replies: crossbeam_channel::Receiver<Message>,
    unique: String,
}

impl Bus {
    /// Connects to the user's session bus and answers incoming calls with `handler`.
    pub(crate) fn session(handler: Handler) -> Result<Self, Error> {
        let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
        Self::connect(address.as_deref(), handler)
    }
    fn connect(address: Option<&str>, handler: Handler) -> Result<Self, Error> {
        let mut stream = connect(address)?;
        authenticate(&mut stream)?;
        let serial = Arc::new(AtomicU32::new(1));
        let hello = encode(
            &Outgoing {
                kind: METHOD_CALL,
                destination: Some("org.freedesktop.DBus"),
                path: Some("/org/freedesktop/DBus"),
                interface: Some("org.freedesktop.DBus"),
                member: Some("Hello"),
                ..Outgoing::default()
            },
            1,
            &[],
        )?;
        stream.set_read_timeout(Some(REPLY_TIMEOUT))?;
        stream.write_all(&hello)?;
        let unique = loop {
            let message = read_message(&mut stream)?;
            if message.reply_serial == Some(1) {
                match (message.kind, message.args.first()) {
                    (METHOD_RETURN, Some(Arg::Str(name))) => break name.clone(),
                    _ => return Err(remote(message)),
                }
            }
        };
        stream.set_read_timeout(None)?;
        let writer = Arc::new(Mutex::new(stream.try_clone()?));
        let (sender, replies) = crossbeam_channel::bounded(16);
        let reply_writer = writer.clone();
        let reply_serial = serial.clone();
        std::thread::Builder::new()
            .name("session-bus".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    read_loop(stream, &sender, &reply_writer, &reply_serial, &handler)
                }));
                if let Ok(Err(error)) = result {
                    tracing::debug!(%error, "Session bus connection closed");
                }
            })?;
        Ok(Self {
            writer,
            serial,
            replies,
            unique,
        })
    }
    /// Our unique bus name, which compositor scripts use to call back.
    pub(crate) fn unique_name(&self) -> &str {
        &self.unique
    }
    /// Calls a method and waits for its reply on the calling (worker) thread.
    pub(crate) fn call(
        &self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        args: &[Arg],
    ) -> Result<Vec<Arg>, Error> {
        let serial = next_serial(&self.serial);
        let bytes = encode(
            &Outgoing {
                kind: METHOD_CALL,
                destination: Some(destination),
                path: Some(path),
                interface: Some(interface),
                member: Some(member),
                ..Outgoing::default()
            },
            serial,
            args,
        )?;
        write(&self.writer, &bytes)?;
        let deadline = std::time::Instant::now() + REPLY_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            let message = match self.replies.recv_timeout(left) {
                Ok(message) => message,
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => return Err(Error::Timeout),
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    return Err(Error::Io(io::ErrorKind::BrokenPipe.into()));
                }
            };
            // Late replies to calls that already timed out are dropped here.
            if message.reply_serial != Some(serial) {
                continue;
            }
            return match message.kind {
                METHOD_RETURN => Ok(message.args),
                _ => Err(remote(message)),
            };
        }
    }
    /// `true` when some connection owns `name`.
    pub(crate) fn has_owner(&self, name: &str) -> bool {
        matches!(
            self.call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "NameHasOwner",
                &[name.into()],
            )
            .as_deref(),
            Ok([Arg::Bool(true)])
        )
    }
    /// The unique name currently owning `name`.
    pub(crate) fn owner(&self, name: &str) -> Result<String, Error> {
        match self
            .call(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "GetNameOwner",
                &[name.into()],
            )?
            .as_slice()
        {
            [Arg::Str(owner)] => Ok(owner.clone()),
            _ => Err(Error::Unexpected),
        }
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        // Ends the reader thread too, since it shares the socket.
        if let Ok(stream) = self.writer.lock() {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

fn remote(message: Message) -> Error {
    Error::Remote {
        name: message
            .error_name
            .unwrap_or_else(|| "org.freedesktop.DBus.Error.Failed".into()),
        message: match message.args.first() {
            Some(Arg::Str(text)) => text.clone(),
            _ => String::new(),
        },
    }
}
fn next_serial(serial: &AtomicU32) -> u32 {
    // Zero is not a valid serial.
    loop {
        let value = serial.fetch_add(1, Ordering::Relaxed);
        if value != 0 {
            return value;
        }
    }
}
fn write(writer: &Mutex<UnixStream>, bytes: &[u8]) -> io::Result<()> {
    writer
        .lock()
        .map_err(|_| io::Error::other("Session bus writer poisoned"))?
        .write_all(bytes)
}
fn read_loop(
    mut stream: UnixStream,
    replies: &crossbeam_channel::Sender<Message>,
    writer: &Mutex<UnixStream>,
    serial: &AtomicU32,
    handler: &Handler,
) -> Result<(), Error> {
    loop {
        let message = read_message(&mut stream)?;
        match message.kind {
            METHOD_RETURN | ERROR => {
                // A full queue means nobody waits for these replies any more.
                let _ = replies.try_send(message);
            }
            METHOD_CALL => {
                let answer = handler(&message);
                if message.flags & NO_REPLY_EXPECTED != 0 {
                    continue;
                }
                let (kind, error_name, args) = match answer {
                    Ok(args) => (METHOD_RETURN, None, args),
                    Err(text) => (
                        ERROR,
                        Some("org.freedesktop.DBus.Error.Failed"),
                        vec![Arg::Str(text)],
                    ),
                };
                let bytes = encode(
                    &Outgoing {
                        kind,
                        destination: message.sender.as_deref(),
                        reply_serial: Some(message.serial),
                        error_name,
                        ..Outgoing::default()
                    },
                    next_serial(serial),
                    &args,
                )?;
                write(writer, &bytes)?;
            }
            // Signals such as NameAcquired are not used.
            _ => {}
        }
    }
}

fn connect(address: Option<&str>) -> io::Result<UnixStream> {
    for entry in address.unwrap_or("").split(';') {
        let Some(rest) = entry.strip_prefix("unix:") else {
            continue;
        };
        for pair in rest.split(',') {
            let Some((key, value)) = pair.split_once('=') else {
                continue;
            };
            let Some(value) = unescape(value) else {
                continue;
            };
            match key {
                "path" => {
                    use std::os::unix::ffi::OsStrExt;
                    if let Ok(stream) = UnixStream::connect(std::ffi::OsStr::from_bytes(&value)) {
                        return Ok(stream);
                    }
                }
                "abstract" => {
                    use std::os::linux::net::SocketAddrExt;
                    let socket = std::os::unix::net::SocketAddr::from_abstract_name(&value)?;
                    if let Ok(stream) = UnixStream::connect_addr(&socket) {
                        return Ok(stream);
                    }
                }
                _ => {}
            }
        }
    }
    // The systemd user bus is the default when no address is exported.
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No session bus"))?;
    UnixStream::connect(std::path::Path::new(&runtime).join("bus"))
}
/// Undoes D-Bus address escaping (`%xx`).
fn unescape(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}
fn authenticate(stream: &mut UnixStream) -> Result<(), Error> {
    use std::fmt::Write as _;
    let uid = rustix::process::getuid().as_raw().to_string();
    let mut hex = String::new();
    for byte in uid.bytes() {
        let _ = write!(hex, "{byte:02x}");
    }
    stream.set_read_timeout(Some(REPLY_TIMEOUT))?;
    stream.write_all(format!("\0AUTH EXTERNAL {hex}\r\n").as_bytes())?;
    let mut line = Vec::new();
    // Read byte by byte: the binary protocol starts right after BEGIN.
    let mut reader = io::BufReader::with_capacity(1, &mut *stream);
    reader.read_until(b'\n', &mut line)?;
    if line.len() > 512 || !line.starts_with(b"OK ") {
        return Err(Error::Remote {
            name: "org.freedesktop.DBus.Error.AuthFailed".into(),
            message: String::from_utf8_lossy(&line).trim().to_owned(),
        });
    }
    stream.write_all(b"BEGIN\r\n")?;
    Ok(())
}

#[derive(Default)]
struct Outgoing<'a> {
    kind: u8,
    path: Option<&'a str>,
    interface: Option<&'a str>,
    member: Option<&'a str>,
    error_name: Option<&'a str>,
    destination: Option<&'a str>,
    reply_serial: Option<u32>,
}

struct Writer(Vec<u8>);
impl Writer {
    fn align(&mut self, n: usize) {
        while !self.0.len().is_multiple_of(n) {
            self.0.push(0);
        }
    }
    fn u32(&mut self, value: u32) {
        self.align(4);
        self.0.extend_from_slice(&value.to_le_bytes());
    }
    fn string(&mut self, value: &str) -> Result<(), Error> {
        if value.contains('\0') {
            return Err(Error::Malformed);
        }
        self.u32(u32::try_from(value.len()).map_err(|_| Error::Malformed)?);
        self.0.extend_from_slice(value.as_bytes());
        self.0.push(0);
        Ok(())
    }
    fn signature(&mut self, value: &str) -> Result<(), Error> {
        self.0
            .push(u8::try_from(value.len()).map_err(|_| Error::Malformed)?);
        self.0.extend_from_slice(value.as_bytes());
        self.0.push(0);
        Ok(())
    }
    fn arg(&mut self, arg: &Arg) -> Result<(), Error> {
        match arg {
            Arg::Str(value) => self.string(value)?,
            Arg::Bool(value) => self.u32(u32::from(*value)),
            Arg::I32(value) => self.u32(*value as u32),
            Arg::U32(value) => self.u32(*value),
        }
        Ok(())
    }
    /// One `(yv)` header field holding a string-like value.
    fn field(&mut self, code: u8, kind: &str, value: &str) -> Result<(), Error> {
        self.align(8);
        self.0.push(code);
        self.signature(kind)?;
        if kind == "g" {
            self.signature(value)
        } else {
            self.string(value)
        }
    }
}

fn encode(message: &Outgoing<'_>, serial: u32, args: &[Arg]) -> Result<Vec<u8>, Error> {
    let mut body = Writer(Vec::new());
    for arg in args {
        body.arg(arg)?;
    }
    let signature: String = args.iter().map(Arg::code).collect();
    let mut out = Writer(vec![b'l', message.kind, 0, 1]);
    out.u32(u32::try_from(body.0.len()).map_err(|_| Error::Malformed)?);
    out.u32(serial);
    out.u32(0); // Header-field array length, patched below.
    out.align(8);
    let start = out.0.len();
    for (code, kind, value) in [
        (1, "o", message.path),
        (2, "s", message.interface),
        (3, "s", message.member),
        (4, "s", message.error_name),
        (6, "s", message.destination),
    ] {
        if let Some(value) = value {
            out.field(code, kind, value)?;
        }
    }
    if let Some(reply) = message.reply_serial {
        out.align(8);
        out.0.push(5);
        out.signature("u")?;
        out.u32(reply);
    }
    if !signature.is_empty() {
        out.field(8, "g", &signature)?;
    }
    let length = u32::try_from(out.0.len() - start).map_err(|_| Error::Malformed)?;
    out.0[12..16].copy_from_slice(&length.to_le_bytes());
    out.align(8);
    out.0.extend_from_slice(&body.0);
    if out.0.len() > MAX_MESSAGE {
        return Err(Error::Malformed);
    }
    Ok(out.0)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    big: bool,
}
impl Reader<'_> {
    fn align(&mut self, n: usize) -> Result<(), Error> {
        let next = self.pos.next_multiple_of(n);
        if next > self.bytes.len() {
            return Err(Error::Malformed);
        }
        self.pos = next;
        Ok(())
    }
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let end = self.pos.checked_add(n).ok_or(Error::Malformed)?;
        let slice = self.bytes.get(self.pos..end).ok_or(Error::Malformed)?;
        self.pos = end;
        Ok(slice)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        self.align(4)?;
        let raw: [u8; 4] = self.take(4)?.try_into().map_err(|_| Error::Malformed)?;
        Ok(if self.big {
            u32::from_be_bytes(raw)
        } else {
            u32::from_le_bytes(raw)
        })
    }
    fn text(&mut self, length: usize) -> Result<String, Error> {
        let bytes = self.take(length)?.to_vec();
        if self.u8()? != 0 {
            return Err(Error::Malformed);
        }
        String::from_utf8(bytes).map_err(|_| Error::Malformed)
    }
    fn string(&mut self) -> Result<String, Error> {
        let length = self.u32()? as usize;
        self.text(length)
    }
    fn signature(&mut self) -> Result<String, Error> {
        let length = self.u8()? as usize;
        self.text(length)
    }
    /// Reads one value of a basic type; containers are not needed and rejected.
    fn value(&mut self, code: char) -> Result<Arg, Error> {
        Ok(match code {
            's' | 'o' => Arg::Str(self.string()?),
            'g' => Arg::Str(self.signature()?),
            'b' => match self.u32()? {
                0 => Arg::Bool(false),
                1 => Arg::Bool(true),
                _ => return Err(Error::Malformed),
            },
            'i' => Arg::I32(self.u32()? as i32),
            'u' => Arg::U32(self.u32()?),
            _ => return Err(Error::Unexpected),
        })
    }
}

fn read_message(stream: &mut impl Read) -> Result<Message, Error> {
    let mut fixed = [0u8; 16];
    stream.read_exact(&mut fixed)?;
    let big = match fixed[0] {
        b'l' => false,
        b'B' => true,
        _ => return Err(Error::Malformed),
    };
    let number = |range: std::ops::Range<usize>| {
        let raw: [u8; 4] = fixed[range].try_into().unwrap_or_default();
        if big {
            u32::from_be_bytes(raw)
        } else {
            u32::from_le_bytes(raw)
        }
    };
    let body = number(4..8) as usize;
    let fields = number(12..16) as usize;
    let header = 16usize
        .checked_add(fields)
        .map(|n| n.next_multiple_of(8))
        .ok_or(Error::Malformed)?;
    let total = header.checked_add(body).ok_or(Error::Malformed)?;
    if total > MAX_MESSAGE {
        return Err(Error::Malformed);
    }
    let mut bytes = vec![0; total];
    bytes[..16].copy_from_slice(&fixed);
    stream.read_exact(&mut bytes[16..])?;
    parse(&bytes, header)
}

fn parse(bytes: &[u8], header: usize) -> Result<Message, Error> {
    let big = bytes.first() == Some(&b'B');
    let mut reader = Reader {
        bytes: bytes.get(..header).ok_or(Error::Malformed)?,
        pos: 0,
        big,
    };
    reader.take(1)?;
    let mut message = Message {
        kind: reader.u8()?,
        flags: reader.u8()?,
        ..Message::default()
    };
    if reader.u8()? != 1 {
        return Err(Error::Malformed);
    }
    reader.u32()?;
    message.serial = reader.u32()?;
    let fields = reader.u32()? as usize;
    reader.align(8)?;
    let end = reader.pos.checked_add(fields).ok_or(Error::Malformed)?;
    if end > header {
        return Err(Error::Malformed);
    }
    let mut signature = String::new();
    while reader.pos < end {
        reader.align(8)?;
        let code = reader.u8()?;
        let kind = reader.signature()?;
        let mut chars = kind.chars();
        let (Some(kind), None) = (chars.next(), chars.next()) else {
            return Err(Error::Malformed);
        };
        // Unknown fields must be skipped; every basic header value parses here.
        let value = reader.value(kind)?;
        match (code, value) {
            (1, Arg::Str(v)) => message.path = Some(v),
            (2, Arg::Str(v)) => message.interface = Some(v),
            (3, Arg::Str(v)) => message.member = Some(v),
            (4, Arg::Str(v)) => message.error_name = Some(v),
            (5, Arg::U32(v)) => message.reply_serial = Some(v),
            (6, Arg::Str(v)) => message.destination = Some(v),
            (7, Arg::Str(v)) => message.sender = Some(v),
            (8, Arg::Str(v)) => signature = v,
            _ => {}
        }
    }
    if reader.pos != end {
        return Err(Error::Malformed);
    }
    let mut body = Reader {
        bytes: &bytes[header..],
        pos: 0,
        big,
    };
    let mut args = Vec::new();
    for code in signature.chars() {
        match body.value(code) {
            Ok(arg) => args.push(arg),
            // Bodies with containers are valid D-Bus; MothRef just ignores them.
            Err(Error::Unexpected) => {
                args.clear();
                break;
            }
            Err(error) => return Err(error),
        }
    }
    message.args = args;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn round_trip(outgoing: &Outgoing<'_>, args: &[Arg]) -> Message {
        let bytes = encode(outgoing, 7, args).unwrap();
        read_message(&mut bytes.as_slice()).unwrap()
    }
    #[test]
    fn calls_round_trip_through_the_wire_format() {
        let args = [
            Arg::Str("/run/user/1000/a.js".into()),
            Arg::Bool(true),
            Arg::I32(-3),
            Arg::U32(9),
        ];
        let message = round_trip(
            &Outgoing {
                kind: METHOD_CALL,
                destination: Some("org.kde.KWin"),
                path: Some("/Scripting"),
                interface: Some("org.kde.kwin.Scripting"),
                member: Some("loadScript"),
                ..Outgoing::default()
            },
            &args,
        );
        assert_eq!(message.serial, 7);
        assert_eq!(message.path.as_deref(), Some("/Scripting"));
        assert_eq!(message.member.as_deref(), Some("loadScript"));
        assert_eq!(message.destination.as_deref(), Some("org.kde.KWin"));
        assert_eq!(message.args, args);
    }
    #[test]
    fn replies_carry_their_serial() {
        let message = round_trip(
            &Outgoing {
                kind: ERROR,
                reply_serial: Some(41),
                error_name: Some("org.example.Error"),
                ..Outgoing::default()
            },
            &[Arg::Str("no".into())],
        );
        assert_eq!(message.reply_serial, Some(41));
        assert!(matches!(remote(message), Error::Remote { name, message }
            if name == "org.example.Error" && message == "no"));
    }
    #[test]
    fn malformed_messages_are_rejected_without_panicking() {
        let bytes = encode(
            &Outgoing {
                kind: METHOD_CALL,
                path: Some("/a"),
                member: Some("B"),
                ..Outgoing::default()
            },
            1,
            &[Arg::Str("text".into())],
        )
        .unwrap();
        for cut in 0..bytes.len() {
            assert!(read_message(&mut &bytes[..cut]).is_err());
        }
        let mut seed = 0x2545_f491_u32;
        for _ in 0..2000 {
            let mut copy = bytes.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let index = seed as usize % copy.len();
                copy[index] = (seed >> 8) as u8;
            }
            let _ = read_message(&mut copy.as_slice());
        }
        let mut huge = bytes.clone();
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_message(&mut huge.as_slice()).is_err());
    }
    /// Talks to a real `dbus-daemon` when one is installed, as compositors do.
    #[test]
    fn calls_and_replies_through_a_real_bus() {
        let Ok(mut daemon) = std::process::Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address"])
            .stdout(std::process::Stdio::piped())
            .spawn()
        else {
            return;
        };
        let mut address = String::new();
        io::BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = seen.clone();
        let service = Bus::connect(
            Some(address),
            Box::new(move |message| {
                record.lock().unwrap().push(message.clone());
                match message.member.as_deref() {
                    Some("Picked") => Ok(vec![Arg::U32(5)]),
                    _ => Err("refused".into()),
                }
            }),
        )
        .unwrap();
        let client = Bus::connect(Some(address), Box::new(|_| Ok(Vec::new()))).unwrap();
        assert!(client.has_owner("org.freedesktop.DBus"));
        assert!(client.has_owner(service.unique_name()));
        assert!(!client.has_owner("org.example.Missing"));
        assert_eq!(
            client.owner(service.unique_name()).unwrap(),
            service.unique_name()
        );
        let reply = client.call(
            service.unique_name(),
            "/org/mothref/Compositor",
            "org.mothref.Compositor1",
            "Picked",
            &["{0b1c}".into()],
        );
        assert_eq!(reply.unwrap(), [Arg::U32(5)]);
        let call = seen.lock().unwrap()[0].clone();
        assert_eq!(call.sender.as_deref(), Some(client.unique_name()));
        assert_eq!(call.path.as_deref(), Some("/org/mothref/Compositor"));
        assert_eq!(call.args, [Arg::Str("{0b1c}".into())]);
        let refused = client.call(service.unique_name(), "/a", "org.example.A", "Other", &[]);
        assert!(matches!(refused, Err(Error::Remote { message, .. }) if message == "refused"));
        assert!(matches!(
            client.call("org.example.Missing", "/a", "org.example.A", "B", &[]),
            Err(Error::Remote { .. })
        ));
        drop(service);
        drop(client);
        let _ = daemon.kill();
        let _ = daemon.wait();
    }
    #[test]
    fn addresses_unescape() {
        assert_eq!(unescape("/tmp/a%2cb").unwrap(), b"/tmp/a,b");
        assert!(unescape("%zz").is_none());
        assert!(unescape("%2").is_none());
    }
}
