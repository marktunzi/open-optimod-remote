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

/// One frame from the wire. A `Truncated` frame announced more bytes than the
/// device actually sent before it started the next frame.
#[derive(Debug, PartialEq)]
pub enum Frame {
    Complete(Vec<u8>),
    Truncated { payload: Vec<u8>, declared: usize },
}

/// Bytes after the header that identify a real frame: `%8d%8d` opcode and
/// protocol type 6 (see message.rs).
const PREFIX_SIZE: usize = 16;

fn header_length(h: &[u8]) -> Option<usize> {
    let hi = h[0] ^ 0x1a;
    let lo = h[2] ^ 0x67;
    let n = u16::from_be_bytes([hi, lo]) as usize;
    (h.iter().fold(0_u8, |sum, &b| sum.wrapping_add(b)) == 0
        && (h[1] ^ 0xc2).wrapping_add(hi) == 0
        && (h[3] ^ 0xa8).wrapping_add(lo) == 0
        && n != 0)
        .then_some(n)
}

fn is_message_prefix(bytes: &[u8]) -> bool {
    let (opcode, kind) = bytes.split_at(8);
    let digits = opcode.trim_ascii_start();
    kind == b"       6" && !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

/// The firmware can end a frame early; see `Decoder::with_cut`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCut;

/// Bounded incremental framing. Corrupt data poisons this connection; the owner
/// must reconnect rather than guessing where an audio control message begins.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    expected: Option<usize>,
    failed: bool,
    cut: bool,
    /// Bytes of the current frame already searched for a frame start.
    scanned: usize,
}
impl Decoder {
    /// A decoder for firmware that ends frames early.
    ///
    /// The OPTIMOD 5700i 3.0.1.20 announces a frame's full length but can stop
    /// sending it and start the next frame. The reference unit cut the
    /// processing document it pushes after a terminal `RP` at 4096 wire bytes;
    /// the virtual 5700i with the same original firmware cut it at 4075 bytes,
    /// with a 21-byte `251` before the next frame at 4096, and also cut meter
    /// records at arbitrary points while it switched presets. So a frame ends
    /// where a real frame (valid header and `%8d%8d` protocol-6 prefix) starts
    /// inside it, whatever its opcode. Every other frame is read to its
    /// announced length, and a frame not followed by a real frame is corrupt.
    pub fn with_cut(_: FrameCut) -> Self {
        Self {
            cut: true,
            ..Self::default()
        }
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        Ok(self
            .feed_frames(bytes)?
            .into_iter()
            .map(|frame| match frame {
                Frame::Complete(payload) | Frame::Truncated { payload, .. } => payload,
            })
            .collect())
    }
    pub fn feed_frames(&mut self, bytes: &[u8]) -> Result<Vec<Frame>, String> {
        if self.failed {
            return Err("Decoder is invalid; reconnect required".into());
        }
        self.buffer.extend_from_slice(bytes);
        let mut frames = Vec::new();
        loop {
            let Some(total) = self.expected else {
                if self.buffer.len() < HEADER_SIZE {
                    break;
                }
                let Some(n) = header_length(&self.buffer[..HEADER_SIZE]) else {
                    self.failed = true;
                    return Err("Invalid frame header".into());
                };
                self.expected = Some(HEADER_SIZE + n);
                self.scanned = HEADER_SIZE;
                continue;
            };
            if self.cut {
                let end = total.min(self.buffer.len());
                let mut at = self.scanned;
                let mut cut = None;
                while at < end {
                    match self.frame_starts_at(at) {
                        Some(true) => {
                            cut = Some(at);
                            break;
                        }
                        Some(false) => at += 1,
                        None => break,
                    }
                }
                self.scanned = at;
                if let Some(at) = cut {
                    let rest = self.buffer.split_off(at);
                    frames.push(Frame::Truncated {
                        payload: self.buffer[HEADER_SIZE..].to_vec(),
                        declared: total - HEADER_SIZE,
                    });
                    self.buffer = rest;
                    self.expected = None;
                    continue;
                }
            }
            if self.buffer.len() < total {
                break;
            }
            // Searched to the end with no frame start inside: complete, unless
            // the bytes after it are there and are not a frame either.
            if self.cut && self.scanned >= total && self.frame_starts_at(total) == Some(false) {
                self.failed = true;
                return Err("Invalid frame header".into());
            }
            let rest = self.buffer.split_off(total);
            frames.push(Frame::Complete(self.buffer[HEADER_SIZE..].to_vec()));
            self.buffer = rest;
            self.expected = None;
        }
        Ok(frames)
    }
    /// Whether a real frame (valid header and message prefix) starts at `at`.
    /// `None` until the bytes there decide it.
    fn frame_starts_at(&self, at: usize) -> Option<bool> {
        let header = self.buffer.get(at..at + HEADER_SIZE)?;
        if header_length(header).is_none() {
            return Some(false);
        }
        let prefix = self
            .buffer
            .get(at + HEADER_SIZE..at + HEADER_SIZE + PREFIX_SIZE)?;
        Some(is_message_prefix(prefix))
    }
    /// Corrupt framing was seen; this connection can carry no more messages.
    pub fn is_failed(&self) -> bool {
        self.failed
    }
    pub fn finish(&self) -> Result<(), String> {
        if self.failed || !self.buffer.is_empty() {
            Err("Stream ended with an incomplete or corrupt frame".into())
        } else {
            Ok(())
        }
    }
}
