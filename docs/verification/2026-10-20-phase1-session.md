# Phase 1: reliable single-device session (5700i)

> **Evidence status:** synthetic tests for every case below, plus read-only hardware runs on the reference OPTIMOD 5700i (firmware 3.0.1.20, not on air) on 20 October 2026. No setting was written and no preset was recalled during these runs. The Recall continuity fix is documented separately in [Recall keeps the session](2026-10-20-recall-session.md).

## What changed

| Area | Before | Now |
|---|---|---|
| Connecting | Login, then AS, AP and LP/AP from the status (terminal) port. Any failure closed the valid PC Remote session. | The PC Remote login is the connection. AS and LP/AP follow as a separate step. If that fails, the session stays connected with live meters and shows why; Refresh tries again. Writes and Recall stay refused without confirmed documents. |
| Connect duration | Up to 18 s; the macOS Connections window gave up after 8 s. | At most 14 s (4 s login, 4 s AS, 6 s LP/AP); a silent status port costs one 4 s deadline. The native clients wait at least 5 s longer than the service can take, checked by a test. |
| Ending a session | Three slow ticks in a row (about 12 s). | Only transport loss, corrupt framing, or a response boundary unanswered for 20 s. A late boundary is matched to its own request. |
| Closing a session | `218`, then the socket was closed with replies still unread. | `218`, half-close, then read until the unit closes (at most 1 s). |
| Saved access code | Deleted when the host or either port changed. | Kept when only the name or a port changes. Cleared when the IP address or the processor model changes, and the editors then ask for it. |

## The unit refuses connections after an abrupt close

During the first hardware run of 100 connect/disconnect cycles, cycles 0–47 succeeded. From cycle 48 on, every TCP connection to 6201 and 23 timed out. The unit recovered by itself about 1.5 minutes after the load stopped.

Isolated with raw loops, each started on a recovered unit:

| Loop | Result |
|---|---|
| 200 terminal connections (banner, `disconnect`, normal close), 0.5 s apart | all succeeded |
| 100 PC Remote logins with `218` and a close after reading to the end | all succeeded |
| 100 PC Remote sessions with meters on, 2 s of 50 ms polls, `218`, close after reading to the end | all succeeded |
| The app's pattern (session, polls, AS and LP/AP reads, meters, `218`), closed **without** reading the remaining replies | refused after 2 cycles |
| The same pattern, closed after reading until the unit closes | 60 of 60 succeeded |

Closing a socket with unread data makes the Mac answer with a reset while the unit is still sending meter replies. After a few of those, the 5700i stops accepting connections for about a minute. The app's disconnect now reads until the unit closes, bounded at 1 s.

The original PC Remote sends `218`, waits 100 ms and then closes with a reset (static analysis of `5700iPC.exe`). By then the unit has normally closed its side.

## Results against the Phase 1 gate

### Synthetic (every run)

The real session owner is driven through its command channel against a synthetic 5700i (`crates/orban-web`, `FakeUnit`):

- 100 connect/disconnect cycles: 100 logins, 100 meter subscriptions, 100 disconnects, no writes, a new session identity each time.
- Closed status port: connected with live meters and a `sync_error`; a write is refused before anything is sent; a failed Refresh keeps the session; a Refresh after the port returns loads the documents and clears the error.
- Silent status port: connected within the 4 s read deadline plus 1 s.
- A 14 s stall of the PC Remote replies: the session survives without a reconnect. The test fails with the old rule.
- A closed PC Remote port fails the connection cleanly, and the same owner then connects normally.
- Late response boundaries, malformed messages, the disconnect drain, the credential rules and the client timeout budgets each have their own test.

### Hardware (reference 5700i, read-only)

| Gate item | Result |
|---|---|
| 100 connect/disconnect cycles through the backend API (connect, 2 s of meters, disconnect, 1 s apart) | PASS after the disconnect fix: 100 of 100 connected and synchronized, connect 2.33 s average, 2.98 s maximum. Before the fix: 48, then refusals. |
| Independently configured ports (PC Remote on 16201, status port on 10023, through a local proxy) | PASS: connected and synchronized, 76 presets, live meters. |
| Silent status port, then Refresh | PASS: connected in 4.1 s with live meters and a `sync_error`; Refresh in the same session loaded the documents. |
| Closed status port | PASS: connected in 0.09 s with live meters and a `sync_error`. |
| 150 ms extra latency each way on both ports | PASS: connected and synchronized in 4.5 s; meters kept running at the lower rate the latency allows. |
| PC Remote path stalled for 14 s | PASS: connected every second, same session, meters resumed. |
| PC Remote path stalled for 30 s | PASS: the session ended after 21 s with "The processor has not answered for 20 seconds"; a new connection afterwards took 2.2 s. |
| Connection reset | The app detected it within 0.5 s ("Connection reset by peer") and ended the session. Reconnecting depends on the unit: see below. |

### After a connection reset

With a reset on both sides of the path (proxy `SO_LINGER 0` towards the app and the unit), the 5700i accepted TCP connections but did not answer the login for as long as the reconnect attempts continued every 9 seconds (4.5 minutes). Each failed login left another half-open session on the unit: our side showed `FIN_WAIT_2` connections that the unit never closed. After 2 minutes without any traffic, a single login succeeded at once.

A reset of only the app's side, while the unit's side was kept half-open, blocked new connections as well. It cleared about 50 seconds after that half-open connection was released.

Consequence for a future automatic reconnect: it must back off (tens of seconds, growing) rather than retry quickly, or it keeps the unit unavailable. Manual reconnect after a reset works once the unit has been left alone for about two minutes.

The unit's AS (178 fields) and AP (221 fields, `modif UFBS 15 09 2022`) documents were identical to the saved state after all runs.

## Still open

- **Automatic reconnect** with a growing backoff (see above). The Phase 1 brief asks for a bounded reconnect, but none is implemented: a lost session ends with a clear message and the user reconnects.
- **A long-duration soak** (the 8-hour run belongs to Phase 7).
