const MAX_PAYLOAD: usize = u16::MAX as usize;
const HEADER_SIZE: usize = 5;

pub fn encode(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.is_empty() || payload.len() > MAX_PAYLOAD {
        return Err("Frame payload must contain 1 to 65535 bytes".into());
    }
    let [hi, lo] = (payload.len() as u16).to_be_bytes();
    let mut frame = Vec::with_capacity(HEADER_SIZE + payload.len());
    frame.extend_from_slice(&[
        hi ^ 0x1a,
        hi.wrapping_neg() ^ 0xc2,
        lo ^ 0x67,
        lo.wrapping_neg() ^ 0xa8,
    ]);
    frame.push(
        frame
            .iter()
            .fold(0_u8, |sum, &b| sum.wrapping_add(b))
            .wrapping_neg(),
    );
    frame.extend_from_slice(payload);
    Ok(frame)
}

/// Bounded incremental framing. Corrupt data poisons this connection; the owner
/// must reconnect rather than guessing where an audio control message begins.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    expected: Option<usize>,
    failed: bool,
}
impl Decoder {
    pub fn feed(&mut self, mut bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        if self.failed {
            return Err("Decoder is invalid; reconnect required".into());
        }
        let mut messages = Vec::new();
        while !bytes.is_empty() {
            let target = self.expected.unwrap_or(HEADER_SIZE);
            let count = (target - self.buffer.len()).min(bytes.len());
            self.buffer.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.buffer.len() != target {
                continue;
            }
            if self.expected.is_none() {
                let h = &self.buffer;
                let hi = h[0] ^ 0x1a;
                let lo = h[2] ^ 0x67;
                let n = u16::from_be_bytes([hi, lo]) as usize;
                if h.iter().fold(0_u8, |sum, &b| sum.wrapping_add(b)) != 0
                    || (h[1] ^ 0xc2).wrapping_add(hi) != 0
                    || (h[3] ^ 0xa8).wrapping_add(lo) != 0
                    || n == 0
                {
                    self.failed = true;
                    return Err("Invalid frame header".into());
                }
                self.expected = Some(HEADER_SIZE + n);
            } else {
                messages.push(self.buffer[HEADER_SIZE..].to_vec());
                self.buffer.clear();
                self.expected = None;
            }
        }
        Ok(messages)
    }
    pub fn finish(&self) -> Result<(), String> {
        if self.failed || !self.buffer.is_empty() {
            Err("Stream ended with an incomplete or corrupt frame".into())
        } else {
            Ok(())
        }
    }
}
