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

/// Where a firmware cuts frames of one opcode; see `Decoder::with_cut`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameCut {
    /// Wire bytes the device sends of a longer frame, header included.
    pub wire_bytes: usize,
    /// The only message kind that is cut (opcode as on the wire, e.g. 229).
    pub opcode: u16,
}

/// Bounded incremental framing. Corrupt data poisons this connection; the owner
/// must reconnect rather than guessing where an audio control message begins.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
    expected: Option<usize>,
    failed: bool,
    cut: Option<FrameCut>,
    /// Requested frames of the cut opcode still to come; they arrive whole.
    requested: usize,
}
impl Decoder {
    /// A decoder for firmware that cuts frames of one opcode.
    ///
    /// The OPTIMOD 5700i 3.0.1.20 announces the full length of the processing
    /// document (opcode 229) it pushes after a preset change made elsewhere
    /// (terminal `RP`, front panel), but sends only the first 4096 bytes of
    /// that frame and then starts the next one. The same document requested
    /// with opcode 239 arrives complete. So a longer frame of that opcode
    /// counts as cut only when a valid header and message prefix start exactly
    /// at the cut; any other frame is read to its announced length.
    pub fn with_cut(cut: FrameCut) -> Self {
        Self {
            cut: Some(cut),
            ..Self::default()
        }
    }
    /// The client requested a frame of the cut opcode (the 5700i answers
    /// opcode 239 with a 229 archive). The next such frame is read to its
    /// announced length; no cut is inferred from its body.
    pub fn expect_requested(&mut self) {
        self.requested += 1;
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
                continue;
            };
            if let Some(FrameCut {
                wire_bytes: limit,
                opcode,
            }) = self.cut.filter(|cut| total > cut.wire_bytes)
            {
                let kind = format!("{:8}", opcode.saturating_sub(204));
                let cut = if !self.buffer[HEADER_SIZE..].starts_with(kind.as_bytes()) {
                    Some(false)
                } else if self.requested > 0 {
                    if self.buffer.len() >= total {
                        self.requested -= 1;
                    }
                    Some(false)
                } else {
                    self.cut_at(limit)
                };
                match cut {
                    None => break,
                    Some(true) => {
                        let rest = self.buffer.split_off(limit);
                        frames.push(Frame::Truncated {
                            payload: self.buffer[HEADER_SIZE..].to_vec(),
                            declared: total - HEADER_SIZE,
                        });
                        self.buffer = rest;
                        self.expected = None;
                        continue;
                    }
                    Some(false) => {}
                }
            }
            if self.buffer.len() < total {
                break;
            }
            let rest = self.buffer.split_off(total);
            frames.push(Frame::Complete(self.buffer[HEADER_SIZE..].to_vec()));
            self.buffer = rest;
            self.expected = None;
        }
        Ok(frames)
    }
    /// Whether the frame in the buffer was cut at `limit`: a real frame
    /// (valid header and message prefix) starts there. `None` until the bytes
    /// at `limit` decide it.
    fn cut_at(&self, limit: usize) -> Option<bool> {
        let header = self.buffer.get(limit..limit + HEADER_SIZE)?;
        if header_length(header).is_none() {
            return Some(false);
        }
        let prefix = self
            .buffer
            .get(limit + HEADER_SIZE..limit + HEADER_SIZE + PREFIX_SIZE)?;
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
