# Phase 0 baseline: diagnostics and reproducible failures

> **Evidence status:** Source analysis plus deterministic synthetic fault injection in Rust, run on Linux. No physical processor, no emulator, no original Windows client and no macOS build were used. No write or recall was sent to any hardware. Plan: [issue #17](https://github.com/marktunzi/open-optimod-remote/issues/17), phase 0; scope: [issue #9](https://github.com/marktunzi/open-optimod-remote/issues/9); findings: [issue #8](https://github.com/marktunzi/open-optimod-remote/issues/8).

Baseline: Remote `1b3286e`, emulator `66e7b73`. Before this change `cargo test --workspace` passed 76 tests; after it, 83, five consecutive runs without a failure. `cargo clippy --workspace --all-targets` reports no warnings before or after.

## What changed

- **Connection stages.** A failed connect now names its stage and port: `PC Remote login (port N): …`, `Terminal (port N): System document: …`, `Terminal (port N): Processing document: …`, `Terminal (port N): Preset catalog: …` or `Meter subscription: …`. Failures after a valid login end with `(login succeeded; the session was closed)`, so they can no longer be mistaken for a rejected access code. Connect behaviour is otherwise unchanged.
- **Document rejections.** A document of the wrong family reports the observed and expected version, for example `Unsupported Optimod document version 5500.10 (expected 8300.…)`. Only digits and dots are echoed. A truncated document reports `Incomplete Optimod document`.
- **Diagnostic log.** `GET /api/diagnostics` returns the last 256 events: connect parameters (ports and saved model, never the address), login result with firmware, access level and adapter, each completed or failed stage with elapsed time, every failed meter poll with its count, recovery, session loss with its reason, requested disconnects and a failed meter re-subscription after recall (until now discarded). Bracketed text (terminal commands carry the code as `[CODE]`) and IPv4 addresses are removed on entry; firmware versions after `V ` are kept.
- **Build identity.** `GET /api/version` returns `{"name","version","api"}`. `/api/health` is unchanged in content but now derives its version from the crate, so it cannot drift.

## Findings

| Finding | Status | Evidence |
|---|---|---|
| Issue #1, 5500 connect fails silently | **Reproduced as one confirmed failure path; cause for the reporter unconfirmed** | The 5500/5500i factory set has 2 `5500.10` documents (`ST ENC NO LIM`, 50 and 75 µs) next to 292 `8300.10` documents (emulator `research/5500i/firmware/extracted/app/presets`). The parser accepts only `8300.` for both models. With such a preset on air the processing read fails after a valid login and the session is closed. Test: `issue_1_a_5500_with_a_5500_10_preset_on_air_names_the_failing_stage`. Not known: which preset the reporter's unit ran, and the 5500 system (AS) document version, which the test assumes to be `8300.10`. The reporter used the published 0.1.0, where the 5500 was read-only; the path is the same. |
| CR-01, valid login reported as failure | **Confirmed** | `known_defect_cr01_a_stalled_terminal_discards_a_valid_login`: a terminal that accepts but never answers ends the valid PC Remote session after 4 s. Worst-case successful budget is 4 (login) + 4 (AS) + 4 (AP) + 6 (LP/AP) = 18 s. The native client sets `timeoutIntervalForRequest = 8` and `timeoutIntervalForResource = 12` (`native/macos/ConnectionsAPI.swift:132`); the backend sends nothing until the connect finishes, so the native side likely gives up at 8 s while the backend may still connect. The 8 s figure is inferred from URLSession semantics, not measured. |
| CR-02, meter errors end the session | **Confirmed** | `known_defect_cr02_three_bad_meter_replies_end_a_live_session`: three unusable meter replies end a session whose socket the processor never closed. Each poll failure and the session loss are now logged. |
| CR-03, recall discontinuity on the 5700i | **Blocked: hardware or emulator trace** | Existing synthetic test `recall_uses_cached_catalog_and_pauses_pc_remote_polling` still passes. The real symptom needs a trace of the original client and this app against the 5700i; the emulator has no 5700i model. The discarded re-subscription result is now logged as `meter_resubscribe`. |
| CR-04, edited endpoint forgets the access code | **Confirmed, already fixed in a test as intended behaviour** | `devices::tests::changing_endpoint_forgets_code_but_renaming_keeps_it` asserts the loss. Phase 1 must reverse a design decision, not just fix a bug. The stale `hasSavedCode` in the native editor needs a macOS build to test. |
| CR-05, stale backend accepted | **Partly addressed; launcher blocked** | `/api/version` exists. The launcher still accepts any `open-optimod-remote/` health body; changing `LauncherCore.swift` needs a macOS toolchain to build and test. Service output still goes to `/dev/null`. |
| CR-06, generic beep instead of a cause | **Partly addressed; native window blocked** | The backend message that reaches the native Error Log now names the stage and cause. The Connections window subtitle still shows `Operation failed · See Error Log`; changing it needs a macOS build. |
| CR-15, too little diagnostic evidence | **Addressed in the backend** | `/api/diagnostics`; unit tests cover redaction and the bound. Every connect test asserts that neither the access code nor the address appears in any event. |
| CR-16, no CI | **Not started** | Out of the narrow scope chosen here; the checks a workflow would run are listed above and pass. |

## Blocked in this environment

- **Original-client traces.** The emulator runs only on macOS (CoreAudio, vmnet, CrossOver for the original Windows clients). Side-by-side original-client and app traces need that Mac.
- **Native code.** No Swift toolchain on Linux. Launcher handshake, visible native errors and the connection editor stay unchanged.
- **5500 AS document version.** The 5500i firmware image in the emulator is packed; its version strings are not visible to a static search.

## Not changed

No protocol, timeout, reconnect or credential behaviour. The `known_defect_*` tests pin current behaviour; phase 1 must invert them.
