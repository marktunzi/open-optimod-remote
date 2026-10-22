//! The OPTIMOD-FM 8400 3.0.5 PC Remote session (TCP 51200).
//!
//! The 8400 uses the frame header and `%8d%8d` body prefix of the later
//! PC Remote family, with its own message set. Captured from the original
//! 8400 PC Remote 3.0 and the original firmware (virtual 8400, 9 and
//! 21 October 2026); see docs/protocol/8400.md.
//!
//! | kind | type | direction | meaning |
//! |---|---|---|---|
//! | 204 | 0 | PC → unit | login: the passcode as plain text |
//! | 205 | 1 | unit → PC | login answer: station, software version, access level |
//! | 218 | 14 | PC → unit | logout |
//! | 219 | 15 | PC → unit | keepalive |
//! | 221 | 17 | unit → PC | keepalive answer |
//! | 228 | 24 | PC → unit | remote file call |
//! | 229 | 25 | unit → PC | its answer |
//! | 251 | 47 | PC → unit | a GUI event (`message;data;text;`), e.g. Recall |
//! | 252 | 48 | PC → unit | a parameter change (`NAME;index;Type:value;`) |
//!
//! `kind` is the body's message type plus 204, as in `crate::message`.

use crate::{
    document::{Field, Value},
    framing::Decoder,
};
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Instant, timeout_at},
};

pub const DEFAULT_PORT: u16 = 51200;
pub const LOGIN: u16 = 204;
pub const LOGIN_ANSWER: u16 = 205;
pub const LOGOUT: u16 = 218;
pub const KEEPALIVE: u16 = 219;
pub const KEEPALIVE_ANSWER: u16 = 221;
pub const CALL: u16 = 228;
pub const CALL_ANSWER: u16 = 229;
pub const EVENT: u16 = 251;
pub const CHANGE: u16 = 252;
/// The directory of the unit's on-air, system and user preset files.
pub const USER_DIR: &str = "c:\\8400\\user\\";
/// The PC's UDP port the unit sends meters and its clock to.
pub const METER_PORT: u16 = 16540;
/// Longest wait for the unit to close its side after a logout.
const CLOSE_DRAIN: Duration = Duration::from_secs(1);
/// How long the unit may take to read a new session's first message. The
/// unit serves one session at a time; a new connection waits in its listen
/// backlog until the previous session ends (a logout, or 30 s without a
/// byte). A keepalive answer shows that it is being served.
const SERVED_DEADLINE: Duration = Duration::from_secs(5);
/// The original client waits 10 s for the login answer; the unit answers at
/// once once the session is served.
const LOGIN_DEADLINE: Duration = Duration::from_secs(35);
/// A connection that was given up is still read by the unit later. It is
/// kept until the unit answers and then ended properly, never just closed:
/// an abandoned connection costs the unit at least 31 s, and a close that
/// races its own close can stop it from serving new sessions for minutes.
const ABANDONED_LIMIT: Duration = Duration::from_secs(400);
/// File calls answer within milliseconds; the original client waits forever.
const CALL_DEADLINE: Duration = Duration::from_secs(10);
/// Frames to the unit must stay within its 4096-byte receive buffer.
const MAX_FRAME: usize = 4096;

/// The login answer (type 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login {
    pub station: String,
    /// `3.0.5.7` for firmware 3.0.5.
    pub software: String,
    pub access_level: u8,
    pub hd_radio: bool,
}

/// Parse the login answer. The unit answers a wrong passcode with
/// `Invalid Passcode`.
pub fn parse_login(text: &str) -> Result<Login, String> {
    if text.trim() == "Invalid Passcode" {
        return Err("password failed".into());
    }
    let mut station = None;
    let mut software = None;
    let mut access_level = None;
    let mut hd_radio = false;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(v) = line.strip_prefix("8400 Station Name: ") {
            station = Some(v.to_owned());
        } else if let Some(v) = line.strip_prefix("8400 Software Version: ") {
            software = Some(v.to_owned());
        } else if let Some(v) = line.strip_prefix("Access Level: ") {
            access_level = Some(v.parse::<u8>().map_err(|_| "Invalid access level")?);
        } else if line == "HD Digital Radio Hardware Installed" {
            hd_radio = true;
        }
    }
    match (station, software, access_level) {
        (Some(station), Some(software), Some(access_level)) => Ok(Login {
            station,
            software,
            access_level,
            hd_radio,
        }),
        _ => Err("Unrecognized login response".into()),
    }
}

/// One message body: `%8d` type, `%8d` protocol 6, payload.
pub fn encode(kind: u16, payload: &[u8]) -> Result<Vec<u8>, String> {
    crate::message::encode(kind, payload)
}

/// The kind and payload of a frame body.
pub fn decode(body: &[u8]) -> Result<(u16, &[u8]), String> {
    let number = |bytes: &[u8]| -> Result<u32, String> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "Non-ASCII number")?
            .trim_start_matches(' ');
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Invalid decimal".into());
        }
        text.parse().map_err(|_| "Decimal overflow".into())
    };
    if body.len() < 16 {
        return Err("Invalid message length".into());
    }
    if number(&body[8..16])? != 6 {
        return Err("Unsupported protocol type".into());
    }
    let kind = number(&body[..8])?
        .checked_add(204)
        .and_then(|k| u16::try_from(k).ok())
        .ok_or("Opcode overflow")?;
    Ok((kind, &body[16..]))
}

/// A remote file call: the function number, then each argument as a
/// little-endian length and its bytes, then a zero length.
pub fn call(function: u16, arguments: &[&[u8]]) -> Result<Vec<u8>, String> {
    let mut bytes = function.to_le_bytes().to_vec();
    for argument in arguments {
        let length = u16::try_from(argument.len())
            .ok()
            .filter(|n| *n != 0 && *n != u16::MAX)
            .ok_or("Invalid call argument")?;
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(argument);
    }
    bytes.extend_from_slice(&[0, 0]);
    Ok(bytes)
}

/// A call whose last argument is the rest of the message (length 0xFFFF),
/// as the original client writes a file (function 27). The unit stores the
/// data and the closing zero length that follows it.
pub fn call_with_tail(function: u16, arguments: &[&[u8]], tail: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = call(function, arguments)?;
    bytes.truncate(bytes.len() - 2);
    bytes.extend_from_slice(&u16::MAX.to_le_bytes());
    bytes.extend_from_slice(tail);
    bytes.extend_from_slice(&[0, 0]);
    Ok(bytes)
}

/// A call answer: its function number and values, in the encoding of `call`.
pub fn parse_answer(bytes: &[u8]) -> Result<(u16, Vec<Vec<u8>>), String> {
    let take = |at: usize| -> Result<u16, String> {
        bytes
            .get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| "Truncated call answer".to_string())
    };
    let function = take(0)?;
    let mut at = 2;
    let mut values = Vec::new();
    loop {
        let length = usize::from(take(at)?);
        at += 2;
        if length == 0 {
            break;
        }
        values.push(
            bytes
                .get(at..at + length)
                .ok_or("Truncated call answer")?
                .to_vec(),
        );
        at += length;
    }
    Ok((function, values))
}

/// A NUL-terminated string argument.
pub fn text_argument(text: &str) -> Result<Vec<u8>, String> {
    if !text.is_ascii() || text.bytes().any(|b| b == 0) {
        return Err("Unsupported characters in a file name".into());
    }
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    Ok(bytes)
}

/// The 32-bit value of a four-byte answer.
pub fn word(value: &[u8]) -> Result<i32, String> {
    <[u8; 4]>::try_from(value)
        .map(i32::from_le_bytes)
        .map_err(|_| "Invalid call answer value".into())
}

/// The file name in a `_finddata_t` (280 bytes: attributes, three times,
/// size, then the NUL-terminated name).
pub fn found_name(record: &[u8]) -> Result<String, String> {
    let name = record.get(20..).ok_or("Invalid file record")?;
    let end = name
        .iter()
        .position(|b| *b == 0)
        .ok_or("Invalid file record")?;
    std::str::from_utf8(&name[..end])
        .map(str::to_owned)
        .map_err(|_| "Invalid file name".into())
}

/// One meter packet (type 1, 34 bytes): the bar heights 0-100 of the unit's
/// own meter screen and four status bytes. The value at index k is byte k.
pub fn meter_values(packet: &[u8]) -> Option<&[u8]> {
    (packet.len() == 34 && packet[0] == 1).then_some(packet)
}

/// A parameter change (type 48): `NAME;index;Type:value;`, as the original
/// client sends it (`SE AMOUNT;11;Cent:275;`, `PHASE ROTATOR;1;String:<Out>;`).
pub fn change_payload(name: &str, field: &Field) -> Result<Vec<u8>, String> {
    let plain = |text: &str| {
        !text.is_empty()
            && text.len() <= 99
            && text.is_ascii()
            && !text
                .bytes()
                .any(|b| b.is_ascii_control() || b";<>".contains(&b))
    };
    if !plain(name) {
        return Err("Invalid control name".into());
    }
    let value = match &field.value {
        Value::Int(v) => format!("Int:{v};"),
        Value::Cent(v) => format!("Cent:{v};"),
        Value::Choice(v) if plain(v) => format!("String:<{v}>;"),
        Value::Text(v) if plain(v) => format!("UserString:<{v}>;"),
        _ => return Err("Unsupported characters in control value".into()),
    };
    Ok(format!("{name};{};{value}", field.index).into_bytes())
}

/// A GUI event (type 47): `message;data;text;`. The original client recalls
/// a preset with message 16395, data 28 and the preset name.
pub fn event_payload(message: u16, data: i32, text: &str) -> Result<Vec<u8>, String> {
    if !text.is_ascii() || text.bytes().any(|b| b.is_ascii_control() || b == b';') {
        return Err("Unsupported characters in an event".into());
    }
    Ok(format!("{message};{data};{text};").into_bytes())
}

/// The document text of a file the unit stores: up to and including its
/// `End Preset` line. The unit's files continue with 0xFF padding.
pub fn document_text(file: &[u8]) -> Result<&[u8], String> {
    const END: &[u8] = b"End Preset\n";
    file.windows(END.len())
        .position(|w| w == END)
        .map(|at| &file[..at + END.len()])
        .ok_or_else(|| "Invalid or unsupported Optimod document".into())
}

pub struct Session {
    pub login: Login,
    stream: TcpStream,
    decoder: Decoder,
    pending: VecDeque<Vec<u8>>,
    deadline: Duration,
}

/// A connection given up during the login.
fn abandoned(stream: TcpStream, waiting_for: u16) {
    // Keep the connection until the unit answers what it was sent, then end
    // the session the way it expects. Bounded; nothing else waits for this.
    tokio::spawn(async move {
        let mut session = Session {
            login: Login {
                station: String::new(),
                software: String::new(),
                access_level: u8::MAX,
                hd_radio: false,
            },
            stream,
            decoder: Decoder::default(),
            pending: VecDeque::new(),
            deadline: CALL_DEADLINE,
        };
        let end = Instant::now() + ABANDONED_LIMIT;
        while let Ok(body) = session.receive_until(end).await {
            if decode(&body).is_ok_and(|(kind, _)| kind == waiting_for) {
                break;
            }
        }
        let _ = session.disconnect().await;
    });
}

impl Session {
    /// Log in as the original client does: one connection, and never closed
    /// before the unit has answered the login.
    pub async fn connect(
        address: SocketAddr,
        code: &str,
        deadline: Duration,
    ) -> Result<Self, String> {
        let code = code.to_ascii_uppercase();
        if code.is_empty() || !code.bytes().all(|b| b.is_ascii_graphic()) {
            return Err("Enter the processor's passcode".into());
        }
        let stream = timeout_at(Instant::now() + deadline, TcpStream::connect(address))
            .await
            .map_err(|_| "Connection timed out")?
            .map_err(|e| e.to_string())?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let mut session = Self {
            login: Login {
                station: String::new(),
                software: String::new(),
                access_level: u8::MAX,
                hd_radio: false,
            },
            stream,
            decoder: Decoder::default(),
            pending: VecDeque::new(),
            deadline,
        };
        // A keepalive first: its answer shows the unit is serving this
        // connection and not still ending an earlier session.
        session.send(KEEPALIVE, b"").await?;
        match session.wait_for(KEEPALIVE_ANSWER, SERVED_DEADLINE).await {
            Ok(_) => {}
            Err(error) if error == "Receive timed out" => {
                abandoned(session.stream, KEEPALIVE_ANSWER);
                return Err("The processor is busy. It is still ending an earlier PC Remote session; try again in a minute.".into());
            }
            Err(error) => return Err(error),
        }
        session.send(LOGIN, code.as_bytes()).await?;
        let answer = match session.wait_for(LOGIN_ANSWER, LOGIN_DEADLINE).await {
            Ok(payload) => String::from_utf8_lossy(&payload).into_owned(),
            Err(error) if error == "Receive timed out" => {
                abandoned(session.stream, LOGIN_ANSWER);
                return Err("Login timed out".into());
            }
            Err(error) => return Err(error),
        };
        match parse_login(&answer) {
            Ok(login) => {
                session.login = login;
                Ok(session)
            }
            Err(error) => {
                // A refused passcode leaves the session open on the unit.
                let _ = session.disconnect().await;
                Err(error)
            }
        }
    }

    async fn wait_for(&mut self, kind: u16, limit: Duration) -> Result<Vec<u8>, String> {
        let end = Instant::now() + limit;
        loop {
            let body = self.receive_until(end).await?;
            let (received, payload) = decode(&body)?;
            if received == kind {
                return Ok(payload.to_vec());
            }
        }
    }

    async fn send(&mut self, kind: u16, payload: &[u8]) -> Result<(), String> {
        let bytes = encode(kind, payload)?;
        if bytes.len() > MAX_FRAME {
            return Err("The message is too long for the processor".into());
        }
        timeout_at(
            Instant::now() + self.deadline,
            self.stream.write_all(&bytes),
        )
        .await
        .map_err(|_| "Send timed out; outcome unknown")?
        .map_err(|e| e.to_string())
    }

    async fn receive_until(&mut self, end: Instant) -> Result<Vec<u8>, String> {
        loop {
            if let Some(body) = self.pending.pop_front() {
                return Ok(body);
            }
            let mut buffer = [0; 8192];
            let count = timeout_at(end, self.stream.read(&mut buffer))
                .await
                .map_err(|_| "Receive timed out")?
                .map_err(|e| e.to_string())?;
            if count == 0 {
                self.decoder.finish()?;
                return Err("Device disconnected".into());
            }
            self.pending.extend(self.decoder.feed(&buffer[..count])?);
        }
    }

    async fn exchange(&mut self, request: Vec<u8>, function: u16) -> Result<Vec<Vec<u8>>, String> {
        self.send(CALL, &request).await?;
        let end = Instant::now() + CALL_DEADLINE;
        loop {
            let body = self.receive_until(end).await?;
            let (kind, payload) = decode(&body)?;
            match kind {
                CALL_ANSWER => {
                    let (answered, values) = parse_answer(payload)?;
                    if answered != function {
                        return Err(format!("File call {function} was answered as {answered}"));
                    }
                    return Ok(values);
                }
                KEEPALIVE_ANSWER => {}
                other => {
                    return Err(format!(
                        "Unexpected message {} during a file call",
                        other - 204
                    ));
                }
            }
        }
    }

    /// One remote file call and its answer. Keepalive answers that arrive in
    /// between are consumed. One call at a time, as the original client.
    pub async fn call(
        &mut self,
        function: u16,
        arguments: &[&[u8]],
    ) -> Result<Vec<Vec<u8>>, String> {
        self.exchange(call(function, arguments)?, function).await
    }

    async fn result(&mut self, function: u16, arguments: &[&[u8]]) -> Result<i32, String> {
        let values = self.call(function, arguments).await?;
        word(values.first().ok_or("Empty file call answer")?)
    }

    async fn chdir(&mut self, directory: &str) -> Result<(), String> {
        match self.result(4, &[&text_argument(directory)?]).await? {
            0 => Ok(()),
            _ => Err(format!("The processor has no folder {directory}")),
        }
    }

    /// The names of the files matching `pattern` in `directory`.
    pub async fn list(&mut self, directory: &str, pattern: &str) -> Result<Vec<String>, String> {
        let spec = text_argument(&format!("{directory}{pattern}"))?;
        let record = [0_u8; 280];
        let first = self.call(1, &[&spec, &record]).await?;
        let handle = word(first.first().ok_or("Empty file call answer")?)?;
        let mut names = Vec::new();
        if handle != -1 {
            names.push(found_name(first.get(1).ok_or("Missing file record")?)?);
            loop {
                let next = self.call(3, &[&handle.to_le_bytes(), &record]).await?;
                if word(next.first().ok_or("Empty file call answer")?)? != 0 {
                    break;
                }
                names.push(found_name(next.get(1).ok_or("Missing file record")?)?);
                if names.len() > 4096 {
                    return Err("Too many files".into());
                }
            }
        }
        self.result(2, &[&handle.to_le_bytes()]).await?;
        Ok(names)
    }

    /// The contents of one file, as the original client reads it (open,
    /// attach a stream, read the rest of the file, close). Text files come
    /// back with 0xFF padding; see `document_text`.
    pub async fn read(&mut self, directory: &str, name: &str) -> Result<Vec<u8>, String> {
        self.chdir(directory).await?;
        let handle = self.result(12, &[]).await?.to_le_bytes();
        let descriptor = self
            .result(5, &[&text_argument(name)?, &0_i32.to_le_bytes()])
            .await;
        let contents = async {
            let descriptor = descriptor?;
            if descriptor == -1 {
                return Err(format!("The processor has no file {name}"));
            }
            self.result(14, &[&handle, &descriptor.to_le_bytes()])
                .await?;
            let values = self.call(19, &[&handle]).await?;
            self.result(18, &[&handle]).await?;
            values
                .into_iter()
                .next()
                .ok_or_else(|| "Empty file".to_string())
        }
        .await;
        self.result(13, &[&handle]).await?;
        contents
    }

    /// Write one file, as the original client saves a preset (function 27
    /// after a text-mode open). `contents` must fit one message.
    pub async fn write(
        &mut self,
        directory: &str,
        name: &str,
        contents: &[u8],
    ) -> Result<(), String> {
        let handle = self.result(24, &[]).await?.to_le_bytes();
        let written = async {
            self.result(28, &[&handle, &0x1000_i32.to_le_bytes()])
                .await?;
            self.chdir(directory).await?;
            let file = text_argument(name)?;
            self.result(8, &[&file, &0x180_i32.to_le_bytes()]).await?;
            self.result(30, &[&handle, &file]).await?;
            self.exchange(call_with_tail(27, &[&handle], contents)?, 27)
                .await?;
            self.result(29, &[&handle]).await
        }
        .await;
        self.result(26, &[&handle]).await?;
        written.map(|_| ())
    }

    /// Delete one file.
    pub async fn remove(&mut self, directory: &str, name: &str) -> Result<(), String> {
        self.chdir(directory).await?;
        match self.result(9, &[&text_argument(name)?]).await? {
            0 => Ok(()),
            _ => Err(format!("The processor could not delete {name}")),
        }
    }

    /// The keepalive the original client sends every 2 s; the unit ends a
    /// session after 30 s without a byte.
    pub async fn keepalive(&mut self) -> Result<(), String> {
        self.send(KEEPALIVE, b"").await?;
        self.wait_for(KEEPALIVE_ANSWER, self.deadline)
            .await
            .map(|_| ())
    }

    /// Change one parameter. The unit does not answer; it saves the change to
    /// its on-air or system file a few seconds later.
    pub async fn change(&mut self, name: &str, field: &Field) -> Result<(), String> {
        if self.login.access_level != 0 {
            return Err("Write permissions for this access level have not been verified".into());
        }
        self.send(CHANGE, &change_payload(name, field)?).await
    }

    /// Post a GUI event to the unit (type 47). The unit does not answer.
    pub async fn event(&mut self, message: u16, data: i32, text: &str) -> Result<(), String> {
        self.send(EVENT, &event_payload(message, data, text)?).await
    }

    pub fn deadline(&self) -> Duration {
        self.deadline
    }

    pub fn peer(&self) -> Result<SocketAddr, String> {
        self.stream.peer_addr().map_err(|e| e.to_string())
    }

    pub fn is_broken(&self) -> bool {
        self.decoder.is_failed()
    }

    /// End the session: the logout and the end of sending back to back. The
    /// unit closes the connection 200 ms after a logout; if this side's end
    /// arrives later, the unit can stop serving new sessions for minutes.
    pub async fn disconnect(mut self) -> Result<(), String> {
        let logout = encode(LOGOUT, b"")?;
        let _ = self.stream.write_all(&logout).await;
        let shutdown = self.stream.shutdown().await.map_err(|e| e.to_string());
        let _ = timeout_at(Instant::now() + CLOSE_DRAIN, async {
            let mut buffer = [0; 4096];
            while matches!(self.stream.read(&mut buffer).await, Ok(n) if n > 0) {}
        })
        .await;
        shutdown
    }
}
