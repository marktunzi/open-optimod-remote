# Native connections, presets and responsive writes — 21 September 2026

> **Superseded current-behavior note:** This file preserves the evidence gathered on 21 September. The 44-point preset rows and continued polling during Recall described below were replaced on 22 September by 40-point rows, native `.sidebar` split items and a single RP/AP Recall exchange with polling paused. Use [native browser](2026-09-22-native-browser-redesign.md), [Recall continuity](2026-09-22-recall-continuity.md) and [meter/write continuity](2026-09-22-meter-write-continuity.md) for the behavior as of 22 September. Since 29 September the Presets workflow is a workspace of the interface, also in the macOS app, and the native Presets window is only a fallback; see [preset management and backup](2026-09-29-preset-management-backup.md).

## Reference behavior reviewed

The OPTIMOD 5700i 3.0 operating manual and official PC Remote 3.0.1.20 package were inspected. The reference application opens Recall in a separate preset dialog, synchronizes its temporary preset folder from the processor when connecting, treats the processor as master, keeps factory presets immutable and permits a modified factory or user preset to be saved as a user preset. Repeated Recall toggles current and previous presets. Saving writes to the processor; backup is a distinct PC archive operation.

The extracted official factory preset files are plaintext and contain the preset name, parent factory preset and processing fields. They were used only to understand the comparison model and are not included in this open-source repository. The 5700i manual states that fetched user preset files remain encrypted and temporary during a PC Remote session.

## Implemented and verified

- Connections is a standalone AppKit window with native sidebar selection, rounded list rows, search, add/remove/actions, a grouped native connection sheet and masked locally saved access codes. The Network source scans the active local IPv4 subnet, probes TCP 23 and accepts only an OPTIMOD 5700i banner; it never sends an access code.
- Presets is a standalone AppKit window with All/Factory/User/Modified filters, search, On Air status, explicit guarded Recall, local `.orb57user` export and local file application.
- The preset window uses 44-point rows, a 188-point sidebar, blue accent selection, title-cased display labels, labeled icon actions and a standard titlebar content inset so neither the sidebar nor its title can overlap the macOS traffic lights.
- Recall uses the complete AP document returned by the authenticated terminal exchange as its confirmation and baseline. The PC Remote owner continues its 50 ms heartbeat and meter cycle throughout Recall and every parameter-readback snapshot, preventing a slow terminal exchange from expiring the live session. Firmware preset events with empty filename metadata are accepted as bounded device metadata.
- Temporary receive timeouts and ambiguous readback failures remain visible in Error Log but do not by themselves close the PC Remote session. The native Connections list checks state twice per second and changes its saved-device status to Connected or Disconnected without reopening the window.
- Less More remains available after its own compound adjustment. It becomes unavailable after another processing control is changed and is enabled again by recalling a factory preset.
- The first confirmed on-air processing document is the active session comparison baseline, including an already modified document present at connection time. Exact field differences are published by the Rust service; controls use cyan for those differences immediately and retain cyan after device confirmation. Returning a field to its baseline clears the marker; Recall or a successfully applied file establishes a new baseline.
- Slider and toggle values update immediately in the interface. The service still reads fresh state, checks host/session/preset/current value, sends one write, crosses the ordered response boundary and requires exact readback. The one-second state poll is paused for the active mutation.
- Setup labels use a 160-point column; sliders use 220 points, pickers 240 and text fields 260. Remaining width is spacer space.
- The revised top status pills use the supplied 56×22 geometry, graphite texture, radial and diagonal face gradients, restrained border and inner highlight, drop shadow and radial green LED.
- The supplied 1254×1254 icon is built into a multi-resolution `.icns` resource.

## Hardware result

The installed app connected to `5700i V 3.0.1.20`, loaded 222 processing and 178 system fields, and loaded 75 presets from the processor. A live B2 Output Mix check changed the value from 0.0 dB to 0.1 dB and restored it to 0.0 dB with exact readback. Across both writes the session stayed connected, the SSE stream produced 42 live samples after its first live packet, no `live:false` event occurred, and the longest observed meter gap was 572 ms. No preset Recall was sent during this hardware verification.

Named on-device preset Save, Save As, rename, delete, previous-preset and bulk backup commands remain unverified at the protocol level. They are intentionally not emitted based on a guessed opcode or terminal command. Local export reads the current processing document. Local file application validates every field against the connected firmware profile, performs ordered writes, then requires the complete imported document to match before it becomes the baseline.

## Automated checks

- Rust workspace: 44 tests passed, including a delayed terminal-operation regression that requires at least four PC Remote heartbeats during the operation.
- UI/model: 41 tests passed.
- Native Settings specification: 134 controls across 10 tabs passed, including complete profile-mapping coverage.
- Native Connections API tests passed.
- Native network-discovery, Error Log redaction, Presets API and preset-window construction tests passed.
- Vite production build passed.
- The application bundle built, installed at `/Applications/Open Optimod Remote.app`, passed strict deep code-signature verification and opened all three native auxiliary windows.
