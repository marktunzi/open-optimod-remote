use crate::{
    adapter::{AdapterDescriptor, AdapterRegistry, Capabilities, DeviceModel, Evidence, SkinId},
    auth,
    framing::Decoder,
    message::{self, Message},
};
use std::{collections::VecDeque, net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{Instant, timeout_at},
};
use zeroize::Zeroizing;
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
    pending: VecDeque<Message>,
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
        timeout_at(end, stream.write_all(&request))
            .await
            .map_err(|_| "Login timed out")?
            .map_err(|e| e.to_string())?;
        let status = login_line(&mut stream, end).await?;
        if status != "connect ok" {
            return Err(match status.as_str() {
                "password failed" | "no password set" | "in use" | "busy" => status,
                _ => "Unrecognized login response".into(),
            });
        }
        let access_level = login_line(&mut stream, end)
            .await?
            .parse::<u8>()
            .map_err(|_| "Invalid access level")?;
        let firmware = login_line(&mut stream, end).await?;
        let adapter = AdapterRegistry::identify_banner(&firmware)?;
        if model_hint != DeviceModel::Auto && model_hint != adapter.model {
            return Err(format!(
                "Detected {} does not match saved model {}",
                adapter.model.label(),
                model_hint.label()
            ));
        }
        login_line(&mut stream, end)
            .await?
            .parse::<u32>()
            .map_err(|_| "Invalid session seed")?;
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
            decoder: Decoder::default(),
            pending: VecDeque::new(),
            deadline,
            code: Zeroizing::new(code.to_ascii_uppercase()),
        })
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
                return Ok(message);
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
            for frame in self.decoder.feed(&buffer[..count])? {
                self.pending
                    .push_back(message::decode_for(&frame, self.adapter.meter_profile)?);
            }
        }
    }
    pub fn decrypt_file(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        crate::archive::decrypt(data, &self.code)
    }
    pub async fn disconnect(mut self) -> Result<(), String> {
        // Closing the local socket is authoritative for the app. The legacy
        // disconnect opcode is best effort because a peer that already closed
        // must never leave the UI stuck in a connected state.
        let _ = self.send(218, b"").await;
        self.stream.shutdown().await.map_err(|e| e.to_string())
    }
}
