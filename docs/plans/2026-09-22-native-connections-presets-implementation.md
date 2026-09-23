# Native Connections and Presets Implementation Plan

> **Status:** Completed. A follow-up replaced the interim plain split views with native `.sidebar` items and versioned the corrected window geometry. See [2026-09-22-native-browser-redesign.md](../verification/2026-09-22-native-browser-redesign.md).

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the two native browser windows with compact macOS composition and make Recall an immediate explicit action without a second confirmation sheet.

**Architecture:** Keep the existing ConnectionsAPI, PresetsAPI, discovery, backend guards, and processor protocol unchanged. Refactor only the AppKit presentation layer, extract small presentation and Recall-policy values that native test executables can assert, and preserve asynchronous API callbacks and Error Log reporting.

**Tech Stack:** Swift 5, AppKit, UniformTypeIdentifiers, Rust backend, shell build/install scripts.

---

### Task 1: Lock the desired native presentation in failing tests

**Files:**
- Modify: `native/macos/ConnectionsWindowTests.swift` or create it if absent
- Modify: `native/macos/PresetsWindowTests.swift`

**Step 1: Write failing assertions**

Assert compact default sizes, 220-point Connections sidebar, 52-point connection rows, 40-point preset rows, no duplicated content heading/footer, native source-list selection policy, no preset double-action, and direct Recall policy.

**Step 2: Run the focused test executables**

Run the relevant `swiftc` commands from `README.md` and execute both binaries.

Expected: FAIL because the existing constants, hierarchy, and Recall policy still describe the old windows.

**Step 3: Commit the tests after the red result is recorded with the implementation**

### Task 2: Rebuild Connections window composition

**Files:**
- Modify: `native/macos/ConnectionsWindow.swift`
- Test: `native/macos/ConnectionsWindowTests.swift`

**Step 1: Introduce presentation constants and native source-list rows**

Add a `ConnectionsPresentation` namespace containing default/minimum sizes, sidebar width, row height, and content margins. Replace hand-painted sidebar selection with a source-list table or system-backed source-list row presentation.

**Step 2: Rebuild the unified toolbar**

Add the sidebar toggle/tracking separator, a main-area title, view mode, standard actions menu, Add, and Search. Remove `window.subtitle` and update operation state in a compact status control rather than beside the traffic lights.

**Step 3: Compact the connection list**

Use 52-point rows, smaller typography and symbols, native selection, trailing state, and `info.circle`. Remove the bottom inspector and expose Connect/Edit/Remove/New through the row, keyboard, and actions menu.

**Step 4: Preserve discovery and mutation behavior**

Keep automatic Network discovery, connect/disconnect, edit, delete confirmation, access-code handling, and Error Log unchanged.

**Step 5: Run the focused Connections test**

Expected: PASS.

### Task 3: Rebuild Presets window composition and Recall behavior

**Files:**
- Modify: `native/macos/PresetsWindow.swift`
- Test: `native/macos/PresetsWindowTests.swift`

**Step 1: Add direct Recall policy and compact presentation constants**

Expose a testable policy that Recall is explicit, immediate, and not triggered by table double-click. Set 900 × 560 default, 760 × 460 minimum, 188-point sidebar, and 40-point rows.

**Step 2: Rebuild toolbar and sidebar**

Use system toolbar items, retain one labeled Recall action, place Apply File and Save to Mac in an actions menu, keep Refresh compact, retain Search, and use native source-list rows.

**Step 3: Rebuild the preset list**

Remove the duplicated title and permanent footer. Use compact native rows with name, kind, icon, and On Air status. Let AppKit handle active and inactive selection colors.

**Step 4: Remove the Recall confirmation sheet**

Construct and send the existing `PresetRecallRequest` directly from the Recall action with `confirmed: true`. Remove table double-click Recall while retaining all backend safety guards, progress state, meter continuity, reload, and Error Log behavior.

**Step 5: Run the focused Presets test**

Expected: PASS.

### Task 4: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/HANDOFF.md`
- Create: `docs/verification/2026-09-22-native-browser-redesign.md`

**Step 1: Describe the final native behavior**

Document compact browsers, automatic discovery, direct Recall, keyboard behavior, and the unchanged hardware safeguards.

**Step 2: Record exact verification evidence**

List focused tests, full tests, build, install, signature verification, and read-only hardware observations.

### Task 5: Full verification and installation

**Files:**
- Verify: entire repository

**Step 1: Run native focused tests**

Compile and run Connections and Presets AppKit test executables.

**Step 2: Run repository tests and static checks**

Run UI tests/build, Rust tests, formatting, Clippy, Swift test executables, and shell syntax checks documented in `README.md`.

**Step 3: Build and install the application**

Run `scripts/install-macos.sh` and verify the installed bundle, property list, executable, and deep code signature.

**Step 4: Perform read-only runtime verification**

Open the installed app, inspect both native windows, reconnect if needed, and confirm meters continue. Do not Recall a real preset or submit another processor write.

**Step 5: Commit the completed implementation**

Commit code, tests, and updated documentation with a focused message.
