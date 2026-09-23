# Meter continuity during interactive writes

> **Evidence status:** Current point-in-time hardware evidence for one reversible B2 processing write and restoration. It does not establish parity for other processing fields.

Target: installed native macOS app and an OPTIMOD 5700i running firmware `5700i V 3.0.1.20`.

The original interactive change path read a complete terminal processing document before the write and another complete document afterwards. A live SSE trace showed normal packets roughly every 40–55 ms, but each document request stopped meter delivery for about 568–600 ms. One user action therefore produced two visible meter freezes even though the PC Remote socket and canvas animation remained active.

The successful path now validates the host, application session, processing identity and exact prior field against the session-owned document, sends the typed opcode-227 write once, and requires an ordered opcode-250/251 boundary. It updates that one cached field after the boundary. An uncertain boundary still triggers exact terminal readback; manual refresh, export and preset workflows retain complete-document verification.

A reversible hardware check moved `B2 OUTPUT MIX` from index 57 (`-0.30 dB`) to index 58 (`-0.20 dB`) and restored index 57. The requests completed in 70 ms and 101 ms. The largest meter intervals intersecting those writes were 189 ms and 103 ms, down from the earlier paired 568–600 ms pauses. The final state was connected, reported no error, and contained the original field value with no modified-field marker.

No routing, coupling, text, preset Recall, firmware or other processing value was changed by this check.
