# Development handoff

This repository is the working source for **Open Optimod Remote**, an independent native/local-web controller with verified 5700i control and a safe multi-model adapter foundation. The current branch is `feature/native-remote`. It is a tested development milestone, not a declaration of complete Windows PC Remote parity.

## What works

- A native Rust process connects directly to the 5700i over Ethernet. Windows, Wine, CrossOver and proprietary Orban binaries are not runtime dependencies.
- The self-contained macOS bundle opens the local interface in an AppKit/WebKit window, starts its bundled Rust service when needed, and shuts down only the service instance it owns.
- The local React interface is English and reproduces the final supplied 2264×2108 instrument artwork proportionally. It uses the supplied 5700 vector identity, high-resolution Orban mark and a dotmatrix font scoped to the blue LCD. The graphite texture remains on the chassis and control surfaces, while the meter canvas stays clean and uses the reference light inner outline and white lower highlight. Connection, on-air preset and the single global FM/HD coupling action share this instrument header.
- The reference device exposes 222 processing fields and 178 system fields. All 400 current fields have an editor backed by 366 static value mappings and 34 typed text editors.
- Sixteen grouped Shared/FM/HD processing pages, simultaneous FM+HD meter groups, conditional FM/HD editing tabs, explicit FM → HD coupling, physical output routing and the reference device's 75-entry preset catalog are present.
- FM→HD coupling hides and locks only HD equalizer/multiband controls with FM counterparts. The original **HD Limiting** page stays available and writable for HD EQ Gain/Frequency, HD Limiter Drive, HD De-Esser and the coupling control, as required by the official 5700i 3.0 manual.
- System Setup uses ten task-oriented categories and named subsections. Friendly labels, current values, status fields, output-routing links and cross-category search replace the raw parameter table.
- The macOS shell also provides a separate native System Settings window. Its ten SF Symbol primary tabs contain 134 controls from the 5700i worksheet, including AES1/AES2 differences, GPI/tally lists, diversity trim, RDS and 24 AF pickers. Five compact categories show every grouped section on one page; larger categories use a second native selector. Every profile-backed item is asserted against the firmware profile in the native test. Fixed-width controls prevent text and IP inputs from stretching across the window. The window follows the current macOS appearance.
- A compact Finder-style native macOS Connections browser owns add/edit/search/connect/switch/remove workflows for multiple named processors. It uses `NSSplitViewController` with a real `.sidebar` item, a 200–260 point system-resizable sidebar, unified toolbar, 52-point rows, native key/inactive selection colors and no duplicate bottom inspector. Connections support Auto Detect or an explicit selection among the eleven current PC Remote models. Network scans the local IPv4 subnet for their terminal-banner families without sending credentials. Access codes are stored locally outside connection metadata and shown only as bullets; Keychain is not used.
- Presets open in a compact native browser built on the same `.sidebar` split-item architecture, with title-cased display names, 40-point rows, a trailing on-air state, one written Recall button and a compact import/export action menu. Selecting or double-clicking a row never changes processing. Recall starts directly from the explicit Recall action without another confirmation sheet, while retaining the session-owned catalog/current-document guards, one combined RP/AP terminal transaction, paused PC Remote polling, one meter resubscription and the 50 ms resumed cadence. The current processing can be saved to a local `.orb57user` file or restored from a compatible file through validated per-field writes and complete readback. The first confirmed processing document is retained as the current-session comparison baseline, including when the processor was already in a `modif …` state at connection time. Confirmed edits remain cyan until restored or a new Recall/file establishes a new baseline.
- Processing controls use the dimensions and materials extracted from the supplied full-interface SVG: 151×8 px tracks, 28×18 px textured thumbs and 70×26 px value fields.
- Live meter packets use channel-specific display curves and browser-frame interpolation. Every 8 px lane has an independent 900 ms peak hold and controlled release. Stale or malformed data clears both the live value and peak state.
- Mutations require the expected device host and connection-session token. Processing changes also check the cached on-air preset and prior field. A normal interactive write crosses the ordered PC Remote 250/251 boundary and updates the guarded cache without opening a terminal AP/AS read, because the reference 5700i suspends meter delivery while producing those full documents. Boundary failures use exact terminal readback; manual refresh, export and preset workflows also retain full-document verification. The single owner continues heartbeats and meter polling during terminal work, never replays an uncertain write, and leaves actual transport-loss detection to the heartbeat. The native Connections list refreshes live connection state while visible.
- Factory-preset Recall enables Less More. Changing another processing control disables Less More for that loaded preset, while moving Less More itself can update the related fields without disabling or disconnecting the session.
- A control moves optimistically as soon as it is released; background polling pauses for that mutation and the device-confirmed snapshot replaces it after verification. This removes the visible delay without weakening write safety.
- Value boxes participate in native Tab order and accept arrows, Page Up/Down, Home/End and +/−. Routine confirmation banners are absent; real failures are redacted and stored in the native Error Log.

## Multi-model research

Official manuals, product pages and PC Remote packages for the 8200, 8400, 8500, 5500/5500i, 5700 FM/HD, 6300, 8600, 8700i, 9300, 9400 and the current HTML5 generation have been inventoried. The evidence, package hashes, protocol-family matrix, proposed adapter architecture and hardware verification gates are in [`docs/research/2026-09-22-multi-model-optimod-support.md`](research/2026-09-22-multi-model-optimod-support.md). The implementation sequence is in [`docs/plans/2026-09-22-multi-model-adapter-plan.md`](plans/2026-09-22-multi-model-adapter-plan.md).

The adapter register now identifies 5700i, 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 and 9400 from their official PC Remote banners. The ten additional models can read model-specific AP/AS documents and LP preset catalogs using the `8300`, `5700`, `6300`, `8500`, `8600`, `8700`, `9300` and `9400` document families. Each model has a distinct skin ID, product mark, material palette, LCD treatment, path label and meter grouping; an unknown identity uses a neutral skin. Only 5700i 3.0.1.20 can write, recall or decode the verified live-meter format. Continue with hardware fixtures model by model. Keep the 8400 TCP/UDP and 8200 serial transports separate.

## Repository layout

- `crates/orban-protocol`: login, wire framing, archives, terminal reads, profiles, presets and diagnostics.
- `crates/orban-web`: single-owner device session, local HTTP/SSE API, connection book and local per-user credential file.
- `packages/ui`: React/Vite interface, final-reference canvas meters, bundled UI identity assets, processing layouts, presets, routing, Setup groups and connection management.
- `native/macos`: native AppKit/WebKit shell, native Connections, Presets and System Settings windows, the supplied app icon and standalone tests.
- `profiles`: independently recorded value mappings used by the protocol layer.
- `skills/optimod-5700i-control`: portable Codex skill containing the protocol workflow, safety invariants, complete 3.0.1.20 parameter profile and meter curves.
- `docs`: protocol evidence, compatibility matrix, multi-model research, meter facts, verification reports and plans.
- `scripts/build-macos.sh`: builds the local web assets, release Rust binary and ad-hoc signed macOS app bundle.
- `scripts/install-macos.sh`: rebuilds, installs and verifies `/Applications/Open Optimod Remote.app`.

## Build and verify

The tested host uses Apple Command Line Tools. If global Xcode is installed but its license is not accepted, set `DEVELOPER_DIR` as shown:

```sh
export DEVELOPER_DIR=/Library/Developer/CommandLineTools
python3 scripts/check-docs.py
npm ci --prefix packages/ui
node --experimental-strip-types --test packages/ui/tests/*.test.ts
npm run build --prefix packages/ui
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
swiftc -parse-as-library native/macos/LauncherCore.swift native/macos/LauncherCoreTests.swift -o /tmp/optimod-launcher-core-tests
/tmp/optimod-launcher-core-tests
swiftc -swift-version 5 native/macos/LauncherCore.swift native/macos/SystemSettingsSpec.swift native/macos/SystemSettingsAPI.swift native/macos/SystemSettingsSpecTests.swift -o /tmp/optimod-settings-spec-tests
/tmp/optimod-settings-spec-tests
swiftc -swift-version 5 native/macos/LauncherCore.swift native/macos/ConnectionsAPI.swift native/macos/ConnectionsAPITests.swift -o /tmp/optimod-connections-api-tests
/tmp/optimod-connections-api-tests
swiftc -swift-version 5 -framework AppKit -framework Network native/macos/ErrorLog.swift native/macos/LauncherCore.swift native/macos/ConnectionsAPI.swift native/macos/NetworkDiscovery.swift native/macos/ConnectionsWindow.swift native/macos/ConnectionsWindowTests.swift -o /tmp/optimod-connections-window-tests
/tmp/optimod-connections-window-tests
swiftc -swift-version 5 native/macos/LauncherCore.swift native/macos/SystemSettingsAPI.swift native/macos/PresetsAPI.swift native/macos/PresetsAPITests.swift -o /tmp/optimod-presets-api-tests
/tmp/optimod-presets-api-tests
swiftc -swift-version 5 -framework Network native/macos/NetworkDiscovery.swift native/macos/NetworkDiscoveryTests.swift -o /tmp/optimod-network-tests
/tmp/optimod-network-tests
swiftc -swift-version 5 -framework AppKit native/macos/ErrorLog.swift native/macos/ErrorLogTests.swift -o /tmp/optimod-error-log-tests
/tmp/optimod-error-log-tests
swiftc -swift-version 5 -framework AppKit -framework UniformTypeIdentifiers native/macos/ErrorLog.swift native/macos/LauncherCore.swift native/macos/SystemSettingsAPI.swift native/macos/PresetsAPI.swift native/macos/PresetsWindow.swift native/macos/PresetsWindowTests.swift -o /tmp/optimod-presets-window-tests
/tmp/optimod-presets-window-tests
sh -n scripts/build-macos.sh scripts/install-macos.sh scripts/stop-macos.sh
sh scripts/build-macos.sh
```

The app bundle is generated at `dist/Open Optimod Remote.app`. Run `scripts/install-macos.sh` to place it in `/Applications`; after that it starts with a normal Finder double-click and does not open a separate browser. It is ad-hoc signed for local development and is not notarized. macOS may ask for local-network access after a rebuild because the signature changes. The browser interface remains available for development by running `cargo run --release -p orban-web` and opening `http://127.0.0.1:5701`.

## Local data

Connection metadata is outside the repository at:

`~/Library/Application Support/OpenOptimodRemote/connections.json`

Saved access codes are kept separately at:

`~/Library/Application Support/OpenOptimodRemote/credentials.json`

The application directory is created with mode `0700` and the credential file with mode `0600`. The native UI displays only bullets for a saved code and never asks for Keychain access. Do not copy credentials, station configurations, user presets or reference executables into the repository. The supplied UI assets live under `packages/ui/public/assets`; their status is recorded in that directory's README.

## Hardware-test boundary

The running reference processor has had its display contrast changed and restored, plus a B2 Output Mix step from 0.0 dB to 0.1 dB and back to 0.0 dB with exact readback. During the processing check the connection remained active and the live meter stream had no false event after becoming live. Coupling, routing, text and Recall checks remain synthetic or read-only. Do not add broader live audio-changing tests without an appropriate maintenance window or test processor.

## Known gaps before parity

1. Complete final visual acceptance against the supplied instrument and native-window references, then compare every remaining page, menu, interaction, unit and context-sensitive visibility rule against the matching Windows release. The native sidebar structure and layout bounds are automated; pixel-level user acceptance remains separate.
2. Verify representative processing writes, FM → HD coupling, every physical route and preset Recall on a safe test setup.
3. Verify the device protocol for named on-device preset Save/Save As/rename/delete/previous before implementing those writes. Local current-document export and validated local-file application are implemented; full device backup and restore remain open. The official 3.0.1.20 manual and PC Remote package have been inspected, but only Recall currently has a verified named-preset command exchange.
4. Complete alternate Two-Band/Five-Band structures, external-change events, restricted-rights behavior, automation, maintenance and remaining RDS workflows.
5. Verify the display-derived peak hold against PC Remote, complete absolute HD/loudness axes, add device gate/overload/lock indicators and run an eight-hour meter/session soak.
6. Add Developer ID signing, notarization, release automation and a clean-machine test before public binaries are offered.

The native Settings evidence is in `docs/verification/2026-09-20-native-system-settings.md`; latest instrument evidence is in `docs/verification/2026-09-20-instrument-ui.md`; protocol evidence is in `docs/verification/2026-09-19.md`; native packaging evidence is in `docs/verification/2026-09-19-macos-app.md`. The compatibility matrix remains the release checklist.

## Codex skill installation

The development machine has the skill installed at `~/.codex/skills/optimod-5700i-control`. To install the repository copy elsewhere:

```sh
cp -R skills/optimod-5700i-control ~/.codex/skills/
```

The skill is intentionally firmware-specific. Add separate profiles and evidence before using it to write to another model or firmware version.
