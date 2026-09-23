# Connections and Processing Redesign Implementation Plan

> **Status:** Completed. The current implementation includes later Recall-continuity and real AppKit-sidebar corrections documented in [HANDOFF](../HANDOFF.md).

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task by task.

**Goal:** Deliver a one-click macOS OPTIMOD 5700i controller with quiet local credential storage, reliable multi-device management, the approved instrument header, and complete grouped processing pages including Less More.

**Architecture:** Keep Rust as the only owner of the active TCP session and React as a local UI over the existing bounded HTTP API. Replace macOS Keychain storage with a separate owner-only JSON credential vault under Application Support. Make Connections, Presets, and Processing first-class workspace views while retaining the verified profile and layout inventories as the source of truth for every device control.

**Tech Stack:** Rust/Axum/Tokio/Serde, React/TypeScript/Vite, native Swift WebKit launcher, Node test runner, Cargo tests.

---

## Task 1: Replace Keychain with an app-private credential vault

**Files:**
- Modify: `crates/orban-web/src/devices.rs`
- Modify: `crates/orban-web/src/main.rs`
- Modify: `crates/orban-web/Cargo.toml`
- Modify: `Cargo.lock`

1. Add failing Rust tests proving credentials survive a fresh vault instance, are deleted with their device, never appear in the connection-list JSON, and use owner-only permissions on Unix.
2. Run the focused device tests and confirm the missing file vault API is the expected failure.
3. Implement an atomic `FileVault` backed by `credentials.json`, with a `0700` parent directory and `0600` file permissions, bounded reads, zeroized returned secrets, and no secret logging.
4. Instantiate `FileVault` in the server, remove the Security Framework dependency, and preserve the existing `Vault` abstraction for deterministic tests.
5. Run focused and full Rust tests, then commit.

## Task 2: Make multi-device connection management reliable

**Files:**
- Modify: `packages/ui/src/Devices.tsx`
- Modify: `packages/ui/src/api.ts`
- Modify: `packages/ui/src/main.tsx`
- Add: `packages/ui/src/connection-behavior.ts`
- Add: `packages/ui/tests/connection-behavior.test.ts`
- Modify: `packages/ui/src/style.css`

1. Add failing UI tests for active-device deletion requiring disconnect first, saved-code display state, search filtering, and selection fallback after deletion.
2. Implement pure connection behavior helpers and rerun the focused tests to green.
3. Replace the sidebar with a dedicated All Connections workspace: source rail, search, list/grid toggle, add, info/edit, connect, disconnect, and remove.
4. Store a newly entered code once through the existing credential endpoint, leave a blank edit field unchanged, and show saved state as bullets without Keychain copy or permission prompts.
5. On active removal, wait for a successful disconnect before deleting. Keep the row and show an inline error when either operation fails.
6. Run UI tests and commit.

## Task 3: Rebuild the top instrument navigation

**Files:**
- Modify: `packages/ui/src/InstrumentHeader.tsx`
- Modify: `packages/ui/src/main.tsx`
- Modify: `packages/ui/src/MeterStrip.tsx`
- Modify: `packages/ui/src/style.css`
- Add: `packages/ui/public/assets/orban-purple.png`
- Add: `packages/ui/tests/navigation-model.test.ts`

1. Add failing tests for top-level Processing/Presets/Connections routing, Setup dispatch, permanent Both metering, and the FM/HD selector appearing only while processing is decoupled.
2. Add the supplied purple Orban asset and rebuild the header controls to match the approved reference.
3. Make Connected open Connections, Presets open the preset library, and Setup open the native settings panel.
4. Remove the old processing-area toolbar, permanent FM/HD selector, meter-view selector, and device sidebar.
5. Keep both FM and HD meters visible; show a compact processing-path choice only when the host reports independent paths.
6. Run UI tests and commit.

## Task 4: Add Less More and responsive processing groups

**Files:**
- Modify: `packages/ui/src/layouts.json`
- Modify: `packages/ui/src/main.tsx`
- Add: `packages/ui/src/processing-layout.ts`
- Add: `packages/ui/tests/processing-layout.test.ts`
- Modify: `packages/ui/src/style.css`

1. Add failing tests for Less More being first, `LESS MORE` binding to reference id 273, geometric group assignment, unnamed-page fallback, and Band Mix pairing.
2. Add the verified Less More page and implement deterministic group construction from the existing layout rectangles.
3. Render named group cards in responsive columns while preserving the profile-backed control order and exact write/readback path.
4. Render two-choice On/Off fields as accessible toggles; retain choice controls for true multi-option fields and value readouts for sliders.
5. Run UI tests and commit.

## Task 5: Apply the approved control and surface styling

**Files:**
- Modify: `packages/ui/src/style.css`
- Modify: `packages/ui/src/Devices.tsx`
- Modify: `packages/ui/src/InstrumentHeader.tsx`
- Modify: `packages/ui/src/main.tsx`

1. Consolidate interface tokens for charcoal surfaces, blue selected states, cyan values, green on/connected states, shadows, focus, disabled, and error states.
2. Match the supplied tabs, inset group wells, metallic slider thumbs, black value fields, toggles, top buttons, spacing, and typography.
3. Make the connections workspace and processing grid usable at 1440×900 and 1180×760 without overlap, clipped labels, or hidden actions.
4. Build the production UI and visually inspect both target sizes. Fix every observed overflow or alignment defect, then commit.

## Task 6: Update docs, package, verify, and install

**Files:**
- Modify: `README.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/protocol.md` if storage or behavior notes require it
- Modify: `scripts/install-macos.sh` only if packaging changes require it

1. Document multiple saved Optimods, local credential storage, connection removal behavior, Less More, conditional FM/HD selection, Setup, and Presets.
2. Verify credentials never occur in public API responses, logs, connection JSON, source control, or built web assets.
3. Run all UI tests, all Rust tests, the production build, and the native packaging/install flow.
4. Launch the installed app, confirm it reaches the connection browser without Keychain prompts, connect to the saved 5700i, and inspect the live instrument without issuing processing writes.
5. Capture final status and commit the documentation/package result.
