//! Bounded terminal exchanges with a complete document marker as response boundary.
use crate::{
    adapter::{AdapterRegistry, DeviceModel},
    control::Scope,
    document::Document,
    presets::{Preset, delete_command, parse_list, recall_command, reply_refusal, store_command},
};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use zeroize::Zeroizing;

/// How far a terminal exchange got. A timeout names it, so a status port
/// that never accepts the connection is told apart from a slow reply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    Connect,
    Banner,
    Reply,
}

/// What a terminal command does: only a command that can change the
/// processor (RP, SP, DP) leaves an unknown outcome when its reply is late.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    Read,
    Change(&'static str),
}

const NOT_REACHED: &str = "did not accept a connection";
const UNREACHABLE: &str = "could not be reached";

/// Whether a terminal error means the status port never took the
/// connection, so nothing was sent and another exchange will not get further.
pub fn is_unreachable(error: &str) -> bool {
    error.contains(NOT_REACHED) || error.contains(UNREACHABLE)
}

fn timed_out(
    address: SocketAddr,
    deadline: Duration,
    stage: Stage,
    received: usize,
    effect: Effect,
) -> String {
    let port = address.port();
    let limit = format!("{} s", deadline.as_millis() as f64 / 1000.0);
    match (stage, effect) {
        (Stage::Connect, _) => {
            format!("The status port (TCP {port}) {NOT_REACHED} within {limit}; nothing was sent")
        }
        (Stage::Banner, _) => format!(
            "The status port (TCP {port}) accepted the connection but sent no terminal banner within {limit}; nothing was sent"
        ),
        (Stage::Reply, Effect::Read) => format!(
            "The status port (TCP {port}) did not finish its reply within {limit} ({received} bytes received); nothing was changed"
        ),
        (Stage::Reply, Effect::Change(what)) => format!(
            "The status port (TCP {port}) did not finish its reply within {limit}; {what} may have been applied"
        ),
    }
}

async fn open(address: SocketAddr) -> Result<TcpStream, String> {
    TcpStream::connect(address).await.map_err(|e| {
        format!(
            "The status port (TCP {}) {UNREACHABLE}: {e}",
            address.port()
        )
    })
}

async fn read_banner(stream: &mut TcpStream, model: DeviceModel) -> Result<(), String> {
    let mut banner = Vec::new();
    while !banner.ends_with(b"\n\r") && !banner.ends_with(b"\r\n") {
        if banner.len() > 255 {
            return Err("Oversized terminal banner".into());
        }
        banner.push(stream.read_u8().await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                "closed the connection without a terminal banner".to_owned()
            } else {
                e.to_string()
            }
        })?);
    }
    let banner = std::str::from_utf8(&banner).map_err(|_| "Invalid terminal banner")?;
    if AdapterRegistry::identify_terminal_banner(banner)? != model {
        return Err("Terminal model does not match the PC Remote session".into());
    }
    Ok(())
}

/// Ends a terminal session as the unit expects, also after a failure or a
/// timeout: a connection dropped mid-reply is reset, and the unit's
/// terminal server does not always recover from that.
async fn close(stream: &mut TcpStream) {
    let _ = timeout(Duration::from_millis(500), async {
        let _ = stream.write_all(b"disconnect\r\n").await;
        let _ = stream.shutdown().await;
        let mut tail = [0; 128];
        while let Ok(n) = stream.read(&mut tail).await {
            if n == 0 {
                break;
            }
        }
    })
    .await;
}

/// Runs `work` on a fresh terminal session within `deadline` and closes the
/// session normally whatever the outcome. A timeout names the stage reached.
async fn with_terminal<T>(
    address: SocketAddr,
    deadline: Duration,
    model: DeviceModel,
    effect: Effect,
    work: impl AsyncFnOnce(&mut TcpStream, &mut usize) -> Result<T, String>,
) -> Result<T, String> {
    let mut stage = Stage::Connect;
    let mut stream = None;
    let mut received = 0;
    let result = timeout(deadline, async {
        let stream = stream.insert(open(address).await?);
        stage = Stage::Banner;
        read_banner(stream, model).await?;
        stage = Stage::Reply;
        work(stream, &mut received).await
    })
    .await;
    if let Some(stream) = stream.as_mut() {
        close(stream).await;
    }
    result
        .unwrap_or_else(|_| Err(timed_out(address, deadline, stage, received, effect)))
        .map_err(|error| {
            if error.starts_with("The status port") {
                error
            } else {
                format!("The status port (TCP {}): {error}", address.port())
            }
        })
}

async fn exchange(
    address: SocketAddr,
    command: Zeroizing<String>,
    deadline: Duration,
    model: DeviceModel,
    effect: Effect,
) -> Result<String, String> {
    with_terminal(
        address,
        deadline,
        model,
        effect,
        async |stream, received| {
            stream
                .write_all(command.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            let marker = b"End Preset<end>";
            loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("Terminal closed before complete snapshot".into());
                }
                buffer.extend_from_slice(&chunk[..n]);
                *received = buffer.len();
                if buffer.len() > 131070 {
                    return Err("Oversized terminal response".into());
                }
                if let Some(end) = buffer.windows(marker.len()).position(|w| w == marker) {
                    buffer.truncate(end + marker.len());
                    return Ok(std::str::from_utf8(&buffer)
                        .map_err(|_| "Unsupported terminal encoding")?
                        .replace("\n\r", "\n")
                        .replace("\r\n", "\n")
                        .replace('\n', "\r\n")
                        + "\r\n");
                }
            }
        },
    )
    .await
}
fn split_document(text: &str, model: DeviceModel) -> Result<(&str, Document), String> {
    let start = text
        .find("OptimodVersion=<")
        .ok_or("Missing processing document")?;
    Ok((
        &text[..start],
        Document::parse_for_model(&text.as_bytes()[start..], model)?,
    ))
}
pub async fn read_snapshot(
    address: SocketAddr,
    code: &str,
    scope: Scope,
    deadline: Duration,
) -> Result<Document, String> {
    read_snapshot_for_model(address, code, scope, deadline, DeviceModel::Optimod5700i).await
}

pub async fn read_snapshot_for_model(
    address: SocketAddr,
    code: &str,
    scope: Scope,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Document, String> {
    let _ = Zeroizing::new(crate::auth::login_request(code)?);
    let command = Zeroizing::new(format!(
        "{} [{}]??\r\n",
        if scope == Scope::System { "AS" } else { "AP" },
        code.to_ascii_uppercase()
    ));
    Document::parse_for_model(
        exchange(address, command, deadline, model, Effect::Read)
            .await?
            .as_bytes(),
        model,
    )
}
/// The on-air processing document as the terminal's `AP` prints it, for a
/// model that saves presets by uploading that document (5700i).
pub async fn read_processing_text_for_model(
    address: SocketAddr,
    code: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Zeroizing<String>, String> {
    let _ = Zeroizing::new(crate::auth::login_request(code)?);
    let command = Zeroizing::new(format!("AP [{}]??\r\n", code.to_ascii_uppercase()));
    let text = Zeroizing::new(exchange(address, command, deadline, model, Effect::Read).await?);
    let start = text
        .find("OptimodVersion=<")
        .ok_or("Missing processing document")?;
    Document::parse_for_model(&text.as_bytes()[start..], model)?;
    Ok(Zeroizing::new(text[start..].to_owned()))
}
pub async fn read_presets(
    address: SocketAddr,
    code: &str,
    deadline: Duration,
) -> Result<(Vec<Preset>, Document), String> {
    read_presets_for_model(address, code, deadline, DeviceModel::Optimod5700i).await
}

pub async fn read_presets_for_model(
    address: SocketAddr,
    code: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<(Vec<Preset>, Document), String> {
    let _ = Zeroizing::new(crate::auth::login_request(code)?);
    let code = Zeroizing::new(code.to_ascii_uppercase());
    let text = exchange(
        address,
        Zeroizing::new(format!("LP [{}]\r\nAP [{}]??\r\n", *code, *code)),
        deadline,
        model,
        Effect::Read,
    )
    .await?;
    let (list, document) = split_document(&text, model)?;
    Ok((parse_list(list)?, document))
}
pub async fn recall_preset(
    address: SocketAddr,
    code: &str,
    name: &str,
    deadline: Duration,
) -> Result<Document, String> {
    recall_preset_for_model(address, code, name, deadline, DeviceModel::Optimod5700i).await
}

pub async fn recall_preset_for_model(
    address: SocketAddr,
    code: &str,
    name: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Document, String> {
    let mut command = recall_command(name, code)?;
    command.push_str(&format!("AP [{}]??\r\n", code.to_ascii_uppercase()));
    let text = exchange(
        address,
        command,
        deadline,
        model,
        Effect::Change("the recall"),
    )
    .await?;
    let (reply, document) = split_document(&text, model)?;
    if !reply.lines().any(|l| {
        l.trim()
            .strip_prefix("ON AIR:")
            .is_some_and(|n| n.trim() == name)
    }) || document.name != name
    {
        return Err("Preset recall was not confirmed by the processor".into());
    }
    Ok(document)
}

/// Sends `SP` or `DP` followed by `AP` in one terminal transaction. The AP
/// document marks the end of the reply. A refusal from the processor is an
/// error; success must still be confirmed with the preset list.
async fn preset_file_command(
    address: SocketAddr,
    code: &str,
    mut command: Zeroizing<String>,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Document, String> {
    command.push_str(&format!("AP [{}]??\r\n", code.to_ascii_uppercase()));
    let text = exchange(
        address,
        command,
        deadline,
        model,
        Effect::Change("the change"),
    )
    .await?;
    let (reply, document) = split_document(&text, model)?;
    if let Some(refusal) = reply_refusal(reply) {
        return Err(format!("The processor refused: {refusal}"));
    }
    Ok(document)
}

pub async fn store_preset_for_model(
    address: SocketAddr,
    code: &str,
    name: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Document, String> {
    preset_file_command(address, code, store_command(name, code)?, deadline, model).await
}

pub async fn delete_preset_for_model(
    address: SocketAddr,
    code: &str,
    name: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<Document, String> {
    preset_file_command(address, code, delete_command(name, code)?, deadline, model).await
}

/// Sends `RP NAME[CODE]` alone and returns the unit's reply. The caller
/// confirms the result itself: the 8700HD answers without a document.
pub async fn send_recall_for_model(
    address: SocketAddr,
    code: &str,
    name: &str,
    deadline: Duration,
    model: DeviceModel,
) -> Result<String, String> {
    let command = recall_command(name, code)?;
    let reply = with_terminal(
        address,
        deadline,
        model,
        Effect::Change("the recall"),
        async |stream, _| {
            stream
                .write_all(command.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            // The reply is one short line; wait briefly for it.
            let mut reply = Vec::new();
            let _ = timeout(Duration::from_millis(1500), async {
                let mut chunk = [0; 512];
                while let Ok(n) = stream.read(&mut chunk).await {
                    if n == 0 {
                        break;
                    }
                    reply.extend_from_slice(&chunk[..n]);
                    if reply.len() > 4096 || reply.ends_with(b"\r\n") || reply.ends_with(b"\n\r") {
                        break;
                    }
                }
            })
            .await;
            Ok(String::from_utf8_lossy(&reply).trim().to_owned())
        },
    )
    .await?;
    match reply_refusal(&reply) {
        Some(refusal) => Err(format!("The processor refused: {refusal}")),
        None => Ok(reply),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The physical 5700i of issue #18 (29 October 2026): ping and the PC
    /// Remote port answer, TCP 23 does not take the connection at all.
    #[test]
    fn a_connection_that_is_never_accepted_is_unreachable_and_sent_nothing() {
        let address: SocketAddr = "192.168.1.115:23".parse().unwrap();
        let error = timed_out(
            address,
            Duration::from_secs(4),
            Stage::Connect,
            0,
            Effect::Read,
        );
        assert_eq!(
            error,
            "The status port (TCP 23) did not accept a connection within 4 s; nothing was sent"
        );
        assert!(is_unreachable(&error));
        let error = timed_out(
            address,
            Duration::from_secs(4),
            Stage::Connect,
            0,
            Effect::Change("the recall"),
        );
        assert!(!error.contains("applied"), "{error}");
    }
}
