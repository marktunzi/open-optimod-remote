# Changelog

## 0.2.0 — 29 September 2026

On `main`; the signed release has not yet been published. The latest published release is 0.1.0.

- Added on-device preset management for the OPTIMOD 5500: save the on-air processing as a new user preset (`SP`), rename the unmodified on-air user preset (save under the new name, then delete the old one) and delete a user preset that is not on air (`DP`). The firmware documents both commands. Every change is confirmed with a fresh preset list and never retried; factory names, existing names, names over 18 characters and the `modif ` prefix are refused before anything is sent. The 5700i and 8700HD firmware have no such command, so these actions are not offered there.
- Added a backup for every writable model: the system document, the on-air preset and the names of all presets in one JSON file. A restore first lists which system settings it will change and which it leaves alone (network settings, the running clock and status values are never restored), then writes each change once and reads the system document back. User presets that are not on air cannot be backed up, because their contents can only be read by recalling them.
- The Presets workspace now also serves the macOS app (Presets button and Shift-Command-P) and adds preset files for every writable model, with the model's own extension (`.orb55user`, `.orb57user`, `.orb86user`). The native Presets window remains as a fallback when the interface is not loaded.
- System Settings… and Command-, open the native 5700i Settings window only while a 5700i (or no processor) is connected; for the 5500 and 8700HD they open the Setup workspace. The native Settings window reloads its profile when the connected model changes.
- Fixed the native preset file panels: they named the 5700i and its extension for every model, and three status texts showed `(url.lastPathComponent)` literally instead of the file name.
- Setup now places every system field of the 5500, 5700i and 8700HD in one of the same ten categories. That includes test and bypass, the manual and automatic clock, the EI1/EI2 inputs and EO1/EO2 outputs, the processing-structure switches and, on the 8700HD and 5700i, the passcode access level. Passcodes themselves cannot be created, changed or deleted. The 5700i fields keep their exact placement.
- Validated the extraction against the hardware-verified 5700i: 363 of 366 mappings identical and none wrong. This found and fixed four extractor faults: overlapping registrations were missed, lower-case names were rejected, the sample-rate snap was imprecise, and preset offset transforms were unreliable. Offset transforms are now rejected.
- Added 199 statically derived 5700i field names (364 scope entries) next to the verified profile, including the two-band structure and the MX structure of Orban's paid MX upgrade (7 pages: 3 for MX (MX Distortion Control, MX 2 Band Distortion, MX Speech Mode), 3 for the two-band structure (2 Band for FM and HD, 2 Band Distortion) and Compressors (B4/5 Linked)). A statically derived page appears only when the processor reports its fields, so a 5700i without the MX upgrade shows no MX pages. Changes to any statically derived field are read back; hardware-verified fields keep the fast path.
- The 5500 and 8700HD profiles now also cover the terminal port, RDS alternate frequencies 1–24 and timeouts (8700HD), output levels and more. `DWNWRD EXP` is read-only.
- Known limitations: the native Settings window supports only the 5700i (the 5500 and 8700HD use the Setup workspace); passcode management, the automation schedule and maintenance are not available on any model.
- OPTIMOD 5700i: unchanged and still the only hardware-verified model. Its hardware-verified 3.0.1.20 profile, built-in pages and meters are unchanged from 0.1.0 and are now served through the per-model profile registry; the statically derived supplement above is added next to them. Its writes keep the verified fast path without an extra readback.
- Added full control for the OPTIMOD 5500 (firmware 1.2.8.24) and the original OPTIMOD-FM 8700HD (firmware 1.0.2.161): parameter writes, preset recall, live meters and their own processing pages and meter layouts. Both profiles are statically derived from the official PC Remote and firmware and are not yet hardware-verified. The interface says so, and every write is confirmed by a full AP/AS readback that shows the processor's actual value on a mismatch.
- Added the 8700HD as a separate model with its own adapter, skin, discovery banner and connection choice. It is not the 8700i.
- Added `scripts/extract_pc_remote.py`, which derives parameters (emulated conversion routines cross-checked against factory presets), pages (dialog resources and control bindings) and meters (bar channels, curves and orientation) from an official installer. `docs/adding-a-model.md` describes how to add the next model.
- Made profiles, pages and meters per adapter: `Profile::for_adapter`, an exact-banner registry with `Evidence::{Hardware, Static}`, and variable-length meter records for non-5700i models.
- Fixed preset recall and preset-file application, which were hard-coded to the 5700i terminal and document family.

## 0.1.0 — 23 September 2026

- Added safe read-only adapters for 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 and 9400 using their official login banners, terminal AP/AS/LP commands and model-specific document families. Writes, recall and live meters remain enabled only for the verified 5700i 3.0.1.20 profile.
- Migrated saved connections to schema version 2 with Auto Detect or explicit processor selection, added multi-model native network discovery, and reject any mismatch between the saved model, PC Remote banner and terminal identity.
- Added a distinct skin, product mark, chassis palette, LCD treatment, path label and meter grouping for each selectable processor plus a neutral unknown/offline skin.
- Researched official manuals and PC Remote packages for the 8200, 8400, 8500, 5500/5500i, 5700 FM/HD, 6300, 8600, 8700i, 9300, 9400 and HTML5-generation processors; documented four protocol families, package evidence, an adapter architecture and a staged hardware-verification plan.
- Replaced the simulated split layouts in Connections and Presets with `NSSplitViewController` and real `.sidebar` split items, including system-managed translucency, collapse behavior, constrained widths and toolbar-divider tracking.
- Removed the two full AP/AS snapshots from the successful single-control write path. The app now uses its host/session/preset/value guard plus the ordered PC Remote boundary, with exact terminal readback reserved for boundary failures and explicit refresh/verification. On the reference processor this reduced write-related meter pauses from roughly 568–600 ms to 103–189 ms while keeping the connection active and restoring the tested value exactly.
- Reworked preset Recall to use the loaded catalog and current document, perform one RP/AP transaction with PC Remote polling paused, re-enable meters once afterwards and delay the next poll by 50 ms. This removes the redundant LP/AP preflight and prevents heartbeat timeouts from piling up while the processor changes presets.
- Added the supplied 5700/Orban meter artwork as the generated macOS application icon.
- Rebuilt Connections as a Finder-style AppKit window with a native sidebar, view/action controls, saved-device rows, search, info buttons and compact edit sheets.
- Added a separate native Presets window with Factory/User/Modified filters, search, on-air status and guarded Recall.
- Added preset-relative change tracking: processing values turn cyan when they differ from the saved preset and clear when restored.
- Made processing changes respond immediately while preserving fresh-state checks, ordered writes and exact device readback; background polling no longer rolls a pending control back visually.
- Updated the FM/HD and connection pills from the supplied 58×24 gradients, borders, highlights, LEDs and shadows, and reduced processing typography to the regular design weight.
- Fixed Setup input, picker and slider widths so controls remain compact instead of spanning the window.
- Replaced Keychain access with a separate per-user local credential file so saved access codes require no recurring macOS password prompts; the UI displays only bullets.
- Added a second native segmented tab layer to System Settings and retained the selected subsection across refreshes.
- Matched processing sliders to the supplied SVG at 151×8 px tracks, 28×18 px textured metal thumbs and 70×26 px value fields with its exact shadow stack.
- Added a separate native macOS **Optimod 5700i Settings** window, available from the instrument interface and with Command-,.
- Added ten icon-backed native tabs covering Input, Output, Test, Utility, Network, Stereo Encoder, Remote I/O, HD / Digital Radio, RDS and 24 individual RDS AF selectors.
- Added 134 specification-backed controls using native switches, segmented controls, pickers, sliders with editable numeric fields, steppers and text/IP/port fields. Off-plus-value settings retain separate enabled and numeric state.
- Kept System Settings on the existing single-owner Rust session and guarded `/api/change` path. Unverified External AGC, GPI assignment and firmware-missing AES2 Ratings mappings are shown honestly rather than emitting guessed writes.
- Made the Settings window follow the active macOS appearance, with an adaptive light-grey canvas and white cards in Light appearance.
- Extended the light-grey canvas through the native title bar, removed card outlines, and made macOS updates stop the previous signed service before replacement.
- Removed the redundant per-row value copy and content header from System Settings, then reflowed setting cards into two balanced columns with the 24 RDS AF entries split evenly.
- Rebuilt the complete upper instrument area from the final 1132×371 reference, using its exact margins, group widths, separator positions and grey gradients.
- Added the supplied vector 5700 identity, high-resolution Orban mark and Liquid Crystal Display font; the display font is scoped only to the blue LCD.
- Updated meters to the final 8 px lane geometry with 23.5 px stereo wells, 13 px mono wells and independent peak-hold markers for every live lane.
- Reworked coupling and connection state as the two reference-sized status pills while preserving guarded FM → HD couple/decouple confirmation.
- Verified all 27 Processing and Setup states plus FM/HD/Both meter modes at 1440×900 and 1180×760 with no clipped control text or unintended page overflow.

## 0.1.0-alpha.1 — 19 September 2026

- Added a direct Rust 5700i protocol/session implementation and loopback-only web API.
- Added the dark English control interface with reference-derived FM/HD processing layouts.
- Added live grouped canvas meters with fixed-width wells, channel-specific curves and smooth animation-frame interpolation.
- Added multiple saved connections and local access-code storage.
- Added device preset catalog and confirmed Recall workflow.
- Added physical analog, digital and headphone routing controls.
- Added explicit confirmed FM → HD coupling and independent processing control.
- Added ten functional Setup groups with cross-group search.
- Reworked Setup into ten task-oriented categories with subsections, friendly labels, current values, status/routing actions and useful empty states.
- Consolidated connection, on-air preset and the confirmed FM/HD coupling action into one global top bar.
- Audited every Setup category and FM/HD processing page at two native window sizes; fixed text containment, form labels, focus handling and empty results.
- Added host, session, preset and current-value preconditions for mutations.
- Added a self-contained native macOS app with an AppKit/WebKit window, bundled Rust service, one-command installation and clean owned-service shutdown.
- Added the portable `optimod-5700i-control` Codex skill with protocol, control, metering and verification references.

This version remains a development milestone. See `docs/compatibility.md` for incomplete parity areas.
