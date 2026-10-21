use crate::{
    adapter::{AdapterDescriptor, AdapterRegistry, Capabilities, DeviceModel, Evidence, SkinId},
    auth,
    framing::{Decoder, Frame},
    message::{self, Message},
};
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Instant, timeout_at},
};
use zeroize::Zeroizing;
/// Longest wait for the unit to close its side after a disconnect.
const CLOSE_DRAIN: Duration = Duration::from_secs(1);
#[derive(Debug, serde::Serialize)]
pub struct Info {
    pub access_level: u8,
    pub firmware: String,
    pub adapter_id: &'static str,
    pub model: DeviceModel,
    pub skin: SkinId,
    pub capabilities: Capabilities,
    pub evidence: Evidence,
}
pub struct Session {
    pub info: Info,
    adapter: &'static AdapterDescriptor,
    stream: TcpStream,
    decoder: Decoder,
    pending: VecDeque<Result<Message, String>>,
    deadline: Duration,
    code: Zeroizing<String>,
}
async fn login_line(stream: &mut TcpStream, end: Instant) -> Result<String, String> {
    let mut bytes = Vec::new();
    loop {
        let byte = timeout_at(end, stream.read_u8())
            .await
            .map_err(|_| "Login timed out")?
            .map_err(|e| e.to_string())?;
        if byte == b'\n' || byte == 0 {
            break;
        }
        if bytes.len() == 255 {
            return Err("Oversized login response".into());
        }
        bytes.push(byte);
    }
    String::from_utf8(bytes).map_err(|_| "Invalid login response".into())
}
impl Session {
    pub async fn connect(
        address: SocketAddr,
        code: &str,
        deadline: Duration,
    ) -> Result<Self, String> {
        Self::connect_for_model(address, code, deadline, DeviceModel::Auto).await
    }

    pub async fn connect_for_model(
        address: SocketAddr,
        code: &str,
        deadline: Duration,
        model_hint: DeviceModel,
    ) -> Result<Self, String> {
        let end = Instant::now() + deadline;
        let request = Zeroizing::new(auth::login_request(code)?);
        let mut stream = timeout_at(end, TcpStream::connect(address))
            .await
            .map_err(|_| "Connection timed out")?
            .map_err(|e| e.to_string())?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;
        let adapter_and_level = Self::login(&mut stream, &request, end, model_hint).await;
        let (access_level, firmware, adapter) = match adapter_and_level {
            Ok(login) => login,
            Err(error) => {
                // A login the unit did not finish must not linger as a
                // half-open session: the 5700i keeps those open and stops
                // answering new logins. Close with a reset, as the original
                // PC Remote closes every session.
                let _ = stream.set_zero_linger();
                return Err(error);
            }
        };
        Ok(Self {
            info: Info {
                access_level,
                firmware,
                adapter_id: adapter.id,
                model: adapter.model,
                skin: adapter.skin,
                capabilities: adapter.capabilities,
                evidence: adapter.evidence,
            },
            adapter,
            stream,
            decoder: adapter
                .frame_cut
                .map_or_else(Decoder::default, Decoder::with_cut),
            pending: VecDeque::new(),
            deadline,
            code: Zeroizing::new(code.to_ascii_uppercase()),
        })
    }
    async fn login(
        stream: &mut TcpStream,
        request: &[u8],
        end: Instant,
        model_hint: DeviceModel,
    ) -> Result<(u8, String, &'static AdapterDescriptor), String> {
        timeout_at(end, stream.write_all(request))
            .await
            .map_err(|_| "Login timed out")?
            .map_err(|e| e.to_string())?;
        let status = login_line(stream, end).await?;
        if status != "connect ok" {
            return Err(match status.as_str() {
                "password failed" | "no password set" => status,
                // The original client reports these as "<unit> is busy." and
                // "<unit> is in use."; the unit refuses only this login.
                "busy" => "The processor is busy. Try again in a few seconds.".into(),
                "in use" => "The processor is in use.".into(),
                _ => "Unrecognized login response".into(),
            });
        }
        let access_level = login_line(stream, end)
            .await?
            .parse::<u8>()
            .map_err(|_| "Invalid access level")?;
        let firmware = login_line(stream, end).await?;
        let adapter = AdapterRegistry::identify_banner(&firmware)?;
        if model_hint != DeviceModel::Auto && model_hint != adapter.model {
            return Err(format!(
                "Detected {} does not match saved model {}",
                adapter.model.label(),
                model_hint.label()
            ));
        }
        // The 2300 ends its login answer after the firmware line.
        if adapter.wire.login_seed {
            login_line(stream, end)
                .await?
                .parse::<u32>()
                .map_err(|_| "Invalid session seed")?;
        }
        Ok((access_level, firmware, adapter))
    }
    /// No automatic retry: a timed-out control write has an unknown outcome.
    pub async fn send(&mut self, kind: u16, data: &[u8]) -> Result<(), String> {
        let bytes = message::encode(kind, data)?;
        timeout_at(
            Instant::now() + self.deadline,
            self.stream.write_all(&bytes),
        )
        .await
        .map_err(|_| "Send timed out; outcome unknown")?
        .map_err(|e| e.to_string())
    }
    pub async fn change(
        &mut self,
        scope: crate::control::Scope,
        name: &str,
        field: &crate::document::Field,
    ) -> Result<(), String> {
        if !self.adapter.capabilities.parameter_writes {
            return Err(format!(
                "Parameter writes are not verified for {}",
                self.adapter.model.label()
            ));
        }
        if self.info.access_level != 0 {
            return Err("Write permissions for this access level have not been verified".into());
        }
        let bytes = crate::control::encode_change(scope, name, field)?;
        timeout_at(
            Instant::now() + self.deadline,
            self.stream.write_all(&bytes),
        )
        .await
        .map_err(|_| "Write timed out; outcome unknown")?
        .map_err(|e| e.to_string())
    }
    pub async fn next_event(&mut self) -> Result<Message, String> {
        let end = Instant::now() + self.deadline;
        loop {
            if let Some(message) = self.pending.pop_front() {
                return message;
            }
            let mut buffer = [0; 4096];
            let count = timeout_at(end, self.stream.read(&mut buffer))
                .await
                .map_err(|_| "Receive timed out")?
                .map_err(|e| e.to_string())?;
            if count == 0 {
                self.decoder.finish()?;
                return Err("Device disconnected".into());
            }
            // A malformed message does not lose the frames after it: framing
            // is intact, so each frame is decoded and queued on its own.
            for frame in self.decoder.feed_frames(&buffer[..count])? {
                self.pending.push_back(match frame {
                    Frame::Complete(payload) => {
                        message::decode_for(&payload, self.adapter.meter_profile)
                    }
                    Frame::Truncated { payload, declared } => {
                        message::decode_truncated(&payload, declared)
                    }
                });
            }
        }
    }
    /// The adapter selected from the login banner.
    pub fn adapter(&self) -> &'static AdapterDescriptor {
        self.adapter
    }
    /// How long one send or receive may take.
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
    /// Corrupt framing was received; only a new connection can recover.
    pub fn is_broken(&self) -> bool {
        self.decoder.is_failed()
    }
    pub fn decrypt_file(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        crate::archive::decrypt(data, &self.code)
    }
    pub async fn disconnect(mut self) -> Result<(), String> {
        // Closing the local socket is authoritative for the app. The legacy
        // disconnect opcode is best effort because a peer that already closed
        // must never leave the UI stuck in a connected state.
        let _ = self.send(218, b"").await;
        let shutdown = self.stream.shutdown().await.map_err(|e| e.to_string());
        // Read what the unit still sends until it closes. Closing with unread
        // data makes the local stack reset the connection while the unit is
        // still sending; the reference 5700i then refuses every new TCP
        // connection for about a minute. Bounded: the app never waits long.
        let _ = timeout_at(Instant::now() + CLOSE_DRAIN, async {
            let mut buffer = [0; 4096];
            while matches!(self.stream.read(&mut buffer).await, Ok(n) if n > 0) {}
        })
        .await;
        shutdown
    }
}
