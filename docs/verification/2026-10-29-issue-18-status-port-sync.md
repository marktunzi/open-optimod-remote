# Issue #18 — 5700i sync when the status port (TCP 23) does not answer

> **Evidence status:** 29 October 2026. Read-only diagnosis on the physical 5700i 3.0.1.20 (192.168.1.115): two TCP connection attempts to port 23 and the app's state read over its local API. Nothing was written, recalled or sent to the processor. The fix is tested against scripted units only.

## Where the timeout happened

The app was connected to the unit over PC Remote (TCP 6201, meters running) and reported:

> Connected, but the status port (TCP 23) did not answer, so current settings and presets are unavailable. Use Refresh to try again. Terminal response timed out; a write may have been applied

A timed probe of one `AS` read showed the failure is in the **TCP connection itself**: ping and port 6201 answer, but port 23 never accepts the connection (no reply to the connection request; 75 s at 11:10, again 15 s at 11:17). No banner is received and no command is sent. The 4 s system-read deadline covered connection, banner and reply in one, so the stage was not visible, and the message suggested a write that never happened. Why the unit's terminal server stopped accepting connections is not proven. A likely cause is earlier terminal sessions that were dropped without the normal `disconnect` when their deadline ran out.

Root causes in the app:

1. `synchronize()` read `AS` first and stopped at its failure, so the processing and the preset list were never read, even though the on-air document is available over the PC Remote session (opcode 239).
2. Without a processing document, the UI kept every built-in page and the statically derived pages without a `when`, which include the 2-band pages. A 5-band unit showed 2-band controls.
3. Every terminal timeout said "a write may have been applied", also for read-only `AS`/`LP`/`AP`, and did not say which stage timed out.
4. A terminal exchange that ran out of time was dropped without `disconnect`, which resets the unit's terminal connection.

## Fix

- `terminal.rs`: each exchange reports its stage: the port did not accept the connection / could not be reached (nothing sent), no banner (nothing sent), or the reply was not finished (bytes received; read-only: "nothing was changed"; RP, SP and DP: "may have been applied"). Every terminal session now ends with `disconnect` and a normal close, also after an error or a timeout.
- `synchronize()`: the system read and the processing/preset read are independent. A port that did not take the connection is not tried a second time. When the status port cannot give the processing, the on-air archive is read over the PC Remote session. The message says what is missing and why. Changes wait until a complete sync, because their confirmation is read on the status port.
- UI: no processing page is shown without a live on-air document. Then the 2-band pages appear only with `2B DRIVE`, the 5-band pages only with `B1 ATTACK`.
- Native Connections request timeout 25 → 30 s (the connection budget now includes the archive read).

## Tests

- `crates/orban-protocol/tests/terminal.rs`: late reply (stage, byte count, no write claim, normal `disconnect`), no banner, refused connection, closed at once, late recall reply keeps "may have been applied"; unit test for a connection that is never accepted.
- `crates/orban-web`: unreachable port → processing from the archive, preset list and system unavailable, no write claim; late `AS` does not skip `LP/AP`; archive fallback carries 5-band (`B1 ATTACK`) and 2-band (`2B DRIVE`) documents; neither read → no processing; closed and silent ports (updated expectations).
- `packages/ui/tests/model-profiles.test.ts`: no pages without a document; 5-band and 2-band documents show only their own structure's pages.

Not done: the fix has not been run against the physical unit (its TCP 23 still does not accept connections, and a soak session holds its PC Remote connection).
