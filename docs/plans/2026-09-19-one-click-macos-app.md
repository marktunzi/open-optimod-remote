# One-click macOS App Implementation Plan

> **Status:** Completed. The current bundle is built by `scripts/build-macos.sh`, installed by `scripts/install-macos.sh`, and documented in [HANDOFF](../HANDOFF.md).

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Turn the existing development launcher into one self-contained macOS application that opens the complete local Optimod interface in its own window with one click.

**Architecture:** Keep `orban-web` as the sole protocol and device-session owner. Add a small native AppKit/WebKit launcher as the bundle executable, place the Rust server in `Contents/Resources/backend`, and serve the existing compiled React assets from `Contents/Resources/ui`. The launcher verifies an existing local service before starting its bundled helper, displays a dark loading/error state, restricts the embedded browser to the loopback origin, requests a clean device disconnect on Quit, and terminates only a helper it started.

**Tech Stack:** Swift 6/AppKit/WebKit, Rust/Axum/Tokio, React/Vite, shell bundle tooling, ad-hoc macOS code signing.

---

### Task 1: Test the launcher core contract

**Files:**
- Create: `native/macos/LauncherCoreTests.swift`
- Create: `native/macos/LauncherCore.swift`

**Steps:**

1. Write a standalone Swift test executable for exact local health-response recognition and the fixed loopback endpoint.
2. Compile the test without `LauncherCore.swift` and confirm it fails because `BackendHealth` is missing.
3. Implement the minimal pure `BackendHealth` functions.
4. Compile and run the test executable; require a zero exit and its success marker.

### Task 2: Build the native application window

**Files:**
- Create: `native/macos/OpenOptimodApp.swift`

**Steps:**

1. Create an AppKit application with a resizable `WKWebView`, normal Dock presence, standard application menu, and a minimum window size matching the interface.
2. Probe `http://127.0.0.1:5701/api/health`. Reuse only a response beginning with the expected Open Optimod identifier.
3. If unavailable, start `Contents/Resources/backend/orban-web` with browser auto-opening disabled and poll the bounded health check.
4. Restrict in-window navigation to the loopback origin and send external URLs to the system browser.
5. On Quit, POST `/api/disconnect` with the required Origin header, then terminate only the bundled helper owned by this application.
6. Compile the native launcher against AppKit and WebKit.

### Task 3: Make bundled backend behavior explicit

**Files:**
- Modify: `crates/orban-web/src/main.rs`
- Modify: `scripts/stop-macos.sh`
- Test: `crates/orban-web/src/main.rs`

**Steps:**

1. Add a failing unit test for the environment-controlled browser-launch decision.
2. Add a pure helper that disables default-browser launch when `OPEN_OPTIMOD_NO_BROWSER=1`.
3. Use the helper in bundled startup without changing server, session, or protocol behavior.
4. Accept both the legacy and embedded-helper bundle paths in the stop script.
5. Run the focused Rust tests.

### Task 4: Produce the self-contained bundle

**Files:**
- Modify: `scripts/build-macos.sh`
- Create: `scripts/install-macos.sh`

**Steps:**

1. Build UI assets and the release Rust server.
2. Compile the Swift launcher for Apple Silicon with a macOS 13 deployment target.
3. Assemble the launcher, backend, UI, stop command, and Info.plist in `dist/Open Optimod Remote.app`.
4. Add local-network and local-HTTP declarations, normal Dock application metadata, and bundle build number 2.
5. Sign the nested backend and whole application ad hoc, then verify the signature and plist.
6. Add an installer that copies the built app to `/Applications`, signs the installed copy, and verifies it.

### Task 5: Verify one-click operation and update documentation

**Files:**
- Modify: `README.md`
- Modify: `docs/HANDOFF.md`
- Modify: `CHANGELOG.md`
- Create: `docs/verification/2026-09-19-macos-app.md`

**Steps:**

1. Run UI tests/build, all Rust tests, Clippy, formatting, Swift core tests, and the macOS bundle build.
2. Launch the app from `/Applications` with the normal macOS open operation.
3. Confirm the native window displays the local UI, saved connections are visible, no browser tab is opened by the helper, and no device mutation occurs.
4. Quit the app and verify the helper process and local health endpoint stop.
5. Document installation, launch, signing limitations, architecture, evidence, and remaining release gates.
6. Commit the verified one-click macOS milestone locally without publishing it.
