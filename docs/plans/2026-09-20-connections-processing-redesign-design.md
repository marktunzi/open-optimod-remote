# Connections and Processing Redesign

> **Status:** Implemented, then refined by the native Connections and Presets work of 22 September. Use [HANDOFF](../HANDOFF.md) for the current architecture.

## Approved direction

Open Optimod Remote remains a single local macOS application with one Rust owner for the active 5700i session. The supplied connection-browser, processing, control and instrument screenshots are the visual specification. All visible copy remains English.

The interface is an operator instrument used for long sessions on a studio monitor. It therefore keeps the dark charcoal surface and high-contrast meters, with restrained blue selection, cyan value emphasis and green connected/on states.

## Connections

Connections become a dedicated application view rather than a narrow processing sidebar. It uses a macOS-style source rail and a large list of saved Optimods with search, list/grid controls, add, edit/info and remove actions. Selecting a row exposes its details; activating it connects. Returning to the instrument does not discard the saved selection.

Name, address, PC Remote port, status port and access code are entered once. Keychain is removed completely to prevent repeated authorization prompts from ad-hoc development signatures. Codes live in a separate app-private credential file under Application Support, never in the connection-list JSON or any API response. The credential directory is owner-only and the credential file is mode `0600`. The UI always renders a saved code as bullets and has no Keychain checkbox or Keychain wording.

Removing an inactive connection deletes its record and saved code after confirmation. Removing the active connection first disconnects through the existing owner command, then deletes it. A failed disconnect or failed credential deletion leaves the record intact and shows the error.

## Instrument navigation

The upper instrument follows the supplied final reference. The supplied purple Orban logo replaces the current mark. Setup and Presets become top controls next to coupling and connection state. Setup opens the native System Settings window; Presets opens the preset library in the main workspace. The connection status control opens the connection browser.

The permanent FM/HD and meter-view segmented controls are removed. The meter strip continues to show both FM and HD. Processing follows FM while the processor reports FM→HD coupling. A compact FM/HD selector appears only when the device reports independent processing.

The processing navigation order is Less More, Stereo Enhancer, AGC, EQ, Multiband, Compressors, Band Mix, Distortion, Final Clipping and Speech mode. Less More is the verified processor field `LESS MORE`, exposed as its own processing page and guarded by the same host, session, preset and readback checks as every other change.

## Processing controls

The absolute Windows-dialog layout is replaced with the approved grouped reference: full-width dark navigation, blue raised active tab, responsive group columns, inset charcoal wells, rounded metallic slider thumbs, black value readouts and green native-style toggles. Group order and field order remain driven by the verified layout inventory.

Each existing layout group becomes a visual group. Controls are assigned to the group whose reference rectangle contains their center point. Empty helper rectangles are ignored. Pages without useful named groups receive a single page group. Band Mix pairs each band mix and on/off control in one group. All controls retain keyboard, wheel, pending, unavailable and exact device-step behavior.

Radio fields with two opposite choices render as toggles. Multi-choice fields retain segmented/radio selection. Sliders retain an explicit value readout. Controls disabled by disconnection, read-only access, missing profile data or FM→HD coupling remain visibly unavailable.

## Error handling and verification

The local API never returns saved codes. Connection mutations stay bounded to the selected device id. Delete, save, connect and disconnect errors remain inline and do not optimistically alter active-device state.

Verification covers local credential permissions and persistence, multiple device add/edit/delete, active-device deletion, the Less More field, conditional FM/HD selection, top-level Setup/Presets/Connections navigation, all existing protocol tests, production UI build and visual inspection at 1440×900 and 1180×760.
