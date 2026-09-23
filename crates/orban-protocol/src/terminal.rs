//! Bounded terminal exchanges with a complete document marker as response boundary.
use crate::{
    adapter::{AdapterRegistry, DeviceModel},
    control::Scope,
    document::Document,
    presets::{Preset, parse_list, recall_command},
};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};
use zeroize::Zeroizing;
async fn exchange(
    address: SocketAddr,
    command: Zeroizing<String>,
    deadline: Duration,
    model: DeviceModel,
) -> Result<String, String> {
    timeout(deadline, async {
        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|e| e.to_string())?;
        let mut banner = Vec::new();
        while !banner.ends_with(b"\n\r") && !banner.ends_with(b"\r\n") {
            if banner.len() > 255 {
                return Err("Oversized terminal banner".into());
            }
            banner.push(stream.read_u8().await.map_err(|e| e.to_string())?);
        }
        let banner = std::str::from_utf8(&banner).map_err(|_| "Invalid terminal banner")?;
        if AdapterRegistry::identify_terminal_banner(banner)? != model {
            return Err("Terminal model does not match the PC Remote session".into());
        }
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
            if buffer.len() > 131070 {
                return Err("Oversized terminal response".into());
            }
            if let Some(end) = buffer.windows(marker.len()).position(|w| w == marker) {
                buffer.truncate(end + marker.len());
                let text = std::str::from_utf8(&buffer)
                    .map_err(|_| "Unsupported terminal encoding")?
                    .replace("\n\r", "\n")
                    .replace("\r\n", "\n")
                    .replace('\n', "\r\n")
                    + "\r\n";
                let _ = stream.write_all(b"disconnect\r\n").await;
                let _ = stream.shutdown().await;
                let _ = timeout(Duration::from_millis(500), async {
                    let mut tail = [0; 128];
                    while let Ok(n) = stream.read(&mut tail).await {
                        if n == 0 {
                            break;
                        }
                    }
                })
                .await;
                return Ok(text);
            }
        }
    })
    .await
    .map_err(|_| "Terminal response timed out; a write may have been applied".to_owned())?
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
        exchange(address, command, deadline, model)
            .await?
            .as_bytes(),
        model,
    )
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
    let mut command = recall_command(name, code)?;
    command.push_str(&format!("AP [{}]??\r\n", code.to_ascii_uppercase()));
    let text = exchange(address, command, deadline, DeviceModel::Optimod5700i).await?;
    let (reply, document) = split_document(&text, DeviceModel::Optimod5700i)?;
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
