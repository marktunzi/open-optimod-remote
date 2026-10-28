# OPTIMOD 5700i 3.0.1.20 — function matrix against the original PC Remote

> **Evidence status:** living matrix, last updated 28 October 2026 at the commit that changed it; each row links the dated evidence it rests on.

Original client: `5700iPC.exe` (SHA-256 `63def9bb…`), menus from its own resources (menu 128, 208–210), Setup tabs read from the running client. Firmware: virtual 5700i with the original 3.0.1.20 image (scratch .177) unless noted. Status: PASS / FAIL / UNTESTED / BLOCKED / N/A as defined in issue #17 §3. Updated with every feature group.

## Session and connection (Connect menu, connection list)

| Windows function | Mac app | Status | Evidence |
|---|---|---|---|
| New 5700i (connection entry) | Connections window, add | PASS | [Phase 1](2026-10-20-phase1-session.md) |
| Connect / password / busy / in use | Connect, access code | PASS | [Phase 1](2026-10-20-phase1-session.md) |
| Disconnect | Disconnect (drain to EOF) | PASS | [Phase 1](2026-10-20-phase1-session.md) |
| Rename / Delete / Properties of an entry | Connections window | UNTESTED | |
| Groups (New/Rename/Delete Group) | — | UNTESTED | |
| Reconnect after loss | automatic, 120 s backoff | UNTESTED on firmware (fake unit only) | |
| Access levels other than 0 | writes refused | UNTESTED | |

## Meters

| Windows function | Mac app | Status | Evidence |
|---|---|---|---|
| 5-band meter bank | Meters window | PASS | [Phase 3](2026-10-28-ui-phase3-meters.md) |
| 2-band FM/HD gain reduction + HF limiting | Meters window, 2-band only | PASS | [Phase 3–4 close-out](2026-10-28-ui-phase34-closeout.md) |
| Active input (not in the client; app status strip) | follows source/fallback changes | PASS | [M1A](2026-10-28-m1a-active-input.md) |

## Processing (Edit › Processing Parameters, FM→HD Couple/Uncouple)

| Windows function | Mac app | Status | Evidence |
|---|---|---|---|
| AGC page order and write with readback | Processing window | PASS | [Phase 4](2026-10-28-ui-phase4-processing.md) |
| Every other page, control, range and step (5-band, 2-band, HD, MX) | Processing window | UNTESTED | |
| FM->HD Couple / Uncouple | — | UNTESTED | |

## Setup (Edit/Tools › Setup, 11 tabs)

| Tab | Mac app | Status |
|---|---|---|
| Input | System Settings | UNTESTED (preferred input write PASS in M1A) |
| Output 1, Output 2, Stereo Encoder, Test, Utility | System Settings | UNTESTED |
| Network | read only | UNTESTED |
| Remote Interface / Silence | System Settings | UNTESTED (fallback write PASS in M1A) |
| HD Digital Radio, RDS, RDS Alternate Frequencies | System Settings | UNTESTED |

## Presets and files (File menu)

| Windows function | Mac app | Status | Evidence |
|---|---|---|---|
| Recall Preset (factory and user) | Recall | PASS | [Recall keeps the session](2026-10-20-recall-session.md) |
| Save Preset | — | UNTESTED (not implemented) | |
| Save Preset As | — | UNTESTED (not implemented) | |
| Delete Preset | — | UNTESTED (not implemented) | |
| Backup to PC | app backup (AS + AP + names, not the client's format) | UNTESTED against the client | |
| Restore from PC | setup restore | UNTESTED against the client | |

## Tools and Help

| Windows function | Mac app | Status |
|---|---|---|
| Automation | — | UNTESTED (not implemented) |
| Security | — | UNTESTED (not implemented) |
| 5700i Diagnostics | — | UNTESTED (not implemented) |
| Operating Manual, About | — | UNTESTED |

**Model status: NOT COMPLETE.**
