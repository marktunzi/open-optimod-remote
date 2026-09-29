#[derive(Debug, PartialEq)]
pub enum Message {
    Heartbeat,
    Meters {
        bank: u8,
        values: Vec<u8>,
    },
    File {
        kind: u16,
        name: String,
        data: Vec<u8>,
    },
    Other {
        kind: u16,
        data: Vec<u8>,
    },
}

fn decimal(bytes: &[u8]) -> Result<u32, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Non-ASCII number")?
        .trim_start_matches(' ');
    if text.is_empty() || !text.bytes().all(|x| x.is_ascii_digit()) {
        return Err("Invalid decimal".into());
    }
    text.parse().map_err(|_| "Decimal overflow".into())
}
fn line<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], String> {
    let end = bytes
        .iter()
        .position(|b| *b == b'\n')
        .filter(|n| *n <= 255)
        .ok_or("Missing or oversized line")?;
    let result = &bytes[..end];
    *bytes = &bytes[end + 1..];
    Ok(result)
}
pub fn encode(kind: u16, data: &[u8]) -> Result<Vec<u8>, String> {
    let opcode = kind.checked_sub(204).ok_or("Invalid opcode")?;
    let mut bytes = format!("{opcode:8}{:8}", 6).into_bytes();
    bytes.extend_from_slice(data);
    crate::framing::encode(&bytes)
}
pub fn decode(bytes: &[u8]) -> Result<Message, String> {
    decode_for(
        bytes,
        Some(crate::adapter::MeterProfile {
            banks: &[1, 2],
            min_values: 112,
            max_values: 112,
        }),
    )
}

pub fn decode_for(
    bytes: &[u8],
    meter_profile: Option<crate::adapter::MeterProfile>,
) -> Result<Message, String> {
    if bytes.len() < 16 || bytes.len() > 65535 {
        return Err("Invalid message length".into());
    }
    let kind = u16::try_from(
        decimal(&bytes[..8])?
            .checked_add(204)
            .ok_or("Opcode overflow")?,
    )
    .map_err(|_| "Opcode overflow")?;
    if decimal(&bytes[8..16])? != 6 {
        return Err("Unsupported protocol type".into());
    }
    let mut data = &bytes[16..];
    match kind {
        220 => {
            if data.is_empty() {
                Ok(Message::Heartbeat)
            } else {
                Err("Heartbeat contains payload".into())
            }
        }
        228 => {
            let Some(profile) = meter_profile else {
                return Ok(Message::Other {
                    kind,
                    data: data.to_vec(),
                });
            };
            let bank = *data.first().ok_or("Missing meter bank")?;
            if !profile.banks.contains(&bank) {
                return Err("Unsupported meter bank".into());
            }
            data = &data[1..];
            let count = decimal(line(&mut data)?)? as usize;
            if !(profile.min_values..=profile.max_values).contains(&count) || data.len() != count {
                return Err("Invalid meter count".into());
            }
            Ok(Message::Meters {
                bank,
                values: data.to_vec(),
            })
        }
        229..=235 => {
            decimal(line(&mut data)?)?;
            decimal(line(&mut data)?)?;
            let name = std::str::from_utf8(line(&mut data)?).map_err(|_| "Invalid filename")?;
            // This is device metadata, not a local filesystem path. PC Remote
            // events may contain an absolute Windows path. Keep it verbatim so
            // an otherwise valid event cannot tear down the live session. Any
            // future file export must choose its own local filename.
            // The 5700i sends a valid empty filename on preset-change events.
            // It is metadata from the processor, not a path we ever open. An
            // empty value must therefore not invalidate the live PC Remote
            // session. Control characters still indicate a malformed frame.
            if name.chars().any(char::is_control) {
                return Err("Invalid filename metadata".into());
            }
            let count = decimal(line(&mut data)?)? as usize;
            if count != data.len() {
                return Err("Invalid file length".into());
            }
            Ok(Message::File {
                kind,
                name: name.into(),
                data: data.to_vec(),
            })
        }
        _ => Ok(Message::Other {
            kind,
            data: data.to_vec(),
        }),
    }
}
