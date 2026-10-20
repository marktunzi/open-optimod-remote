# Preset Recall keeps the PC Remote session (5700i)

> **Evidence status:** hardware. Reference OPTIMOD 5700i, firmware 3.0.1.20, on 20 October 2026. The unit was not on air. Its system and processing documents were saved first and restored afterwards.

## Symptom

After a Recall, the app lost its connection to the 5700i. The Recall itself succeeded: the processor showed the new preset. About 14 seconds later the app reported `Receive timed out` and closed the session. Live meters stopped as soon as the Recall finished.

Before the fix (`main` at 1b3286e), driven through the backend's own HTTP API: Recall `CLASSICAL-5 BAND`; the session was lost 14.3 s later; one meter event arrived in those 14 seconds.

## Cause

A terminal `RP` makes the 5700i push the new on-air processing document on the open PC Remote session, as an unsolicited file message (opcode 229, empty file name). The frame header announces the whole document, but **the unit sends only the first 4096 bytes of the frame** and then starts the next frame.

Measured from the raw byte stream of the PC Remote connection:

| Recalled preset | Announced body | Sent (whole frame) | Next frame at body byte 4091 |
|---|---:|---:|---|
| CLASSICAL-5 BAND | 7417 | 4096 of 7422 | meter frame (228) |
| GREGG | 7381 | 4096 of 7386 | meter frame (228) |
| ROCK-MEDIUM | 7400 | 4096 of 7405 | meter frame (228) |
| UFBS 15 09 2022 (user) | 7453 | 4096 of 7458 | meter frame (228) |

The `CLASSICAL-2 BAND` document is shorter: its frame (4029 bytes) fits and arrives whole. That is why some Recalls appeared to work.

The decoder trusted the announced length. It took the following meter replies, response boundaries and clock frames as the rest of the document. No `251` boundary reached the session owner, so three 50 ms ticks in a row timed out after 4 s each, and the owner closed the session. Kept open longer, the decoder reached the announced length after about 30 seconds and then found a corrupt header.

What was **not** the cause:

- **Pausing the PC Remote polls during the terminal exchange.** The unit kept the session open and answered again as soon as polls resumed.
- **The meter subscription.** After a Recall whose document fits in one frame, meters continued without a new `222`. A duplicate `222` without a Recall changed nothing.

A document the client requests (opcode 239) is not cut: the on-air archive arrived whole as a 7718-byte frame. The limit applies to the pushed document, not to every frame.

## How the original PC Remote avoids it

From static analysis of `5700iPC.exe` 3.0.1.20; no Windows program was run.

- Its Recall does not use the terminal. It uploads the selected preset file as an opcode 229 message with an empty file name on its own PC Remote session, sends `251`, then sends `250` and waits for the unit's `251`.
- Polling continues during the Recall, and no `222` follows it.
- Its receive loop resynchronises on the next valid five-byte header.

An `RP` from the terminal, the front panel or automation would still make the unit push the cut document to any open session, so the app must handle it whatever its own Recall path is.

## Fix

The verified 5700i adapter declares a frame cut of 4096 wire bytes for opcode 229. Its decoder treats a 229 frame announcing more than 4096 wire bytes as cut only when a valid frame header **and** the `%8d%8d` opcode/protocol-6 prefix start exactly at byte 4096. While an on-air archive requested with opcode 239 is outstanding, the next 229 frame is read whole without that check. Every other frame is read to its announced length, as before.

- A cut frame becomes `Message::Truncated` and is never decrypted or interpreted.
- Corrupt headers still poison the connection.
- Other adapters are unchanged (`frame_cut: None`), because the cut has only been observed on this firmware.

Regression tests:

- `crates/orban-protocol/tests/wire.rs`: a cut push followed by meter and clock frames, at every split point; requested archives longer than 4096 bytes arrive whole; a header without a message prefix at byte 4096 does not end the frame; without the cut the old behaviour is unchanged.
- `crates/orban-web` `recall_survives_the_cut_document_the_5700i_pushes`: a synthetic 5700i pushes the cut document during the terminal Recall; five later ticks must each reach their response boundary. This test fails without the fix.

## Session health

The same failure path exposed three weaknesses in the session owner, fixed separately:

- **Late boundaries.** A `251` that arrived after its tick timed out was taken as the next tick's boundary, and could confirm a later write before the unit had processed it. The owner now counts outstanding boundaries: every `250` is matched to exactly one `251`, oldest first.
- **Disconnect rule.** Three failed ticks in a row (about 12 seconds of slow replies) closed the session. A session now ends only on transport loss (end of stream, reset), corrupt framing, or a `250` left unanswered for 20 seconds. The original client tolerates about 42 seconds without any byte. A slow tick during terminal work no longer fails the terminal read either.
- **Malformed messages.** One undecodable message in a TCP segment discarded the frames behind it, possibly including a `251`. Each frame is now decoded and queued on its own.

After a Recall the owner still sends `222` once, as before. A failed send is now reported instead of discarded.

## Hardware result after the fix

The same backend API, one session, five consecutive Recalls, each followed by 30 seconds of observation:

| Recall | Duration | Session after 30 s | Meter events in 30 s | Seconds without meters |
|---|---:|---|---:|---:|
| CLASSICAL-2 BAND | 1.40 s | connected | 561 | 0 |
| GREGG | 2.16 s | connected | 537 | 0 |
| ROCK-MEDIUM | 2.05 s | connected | 558 | 0 |
| CLASSICAL-5 BAND | 2.27 s | connected | 557 | 0 |
| UFBS 15 09 2022 | 2.16 s | connected | 557 | 0 |

There was no reconnect, error or retry. The confirmed preset was the app's on-air document after each Recall.

## Other observations

- After a session that ended in the corrupt state, the unit refused new TCP connections on 6201 and 23 for about a minute, while still answering ping. It then recovered without intervention.
- The unit sends opcode 247 roughly every 0.85 s on an open session.
- The original client sends a heartbeat at least every 2 s and declares the connection lost only after about 42 s without any received byte.

