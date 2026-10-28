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
| Meter speed and peak markers (bar = bank 1 per record, marker = firmware peak bank 2) | Meters window | PASS | [M1B](2026-10-28-m1b-meter-ballistics-gates.md) |
| AGC Gate (62), MPX Pwr Gate (111) lamps | red LEDs | PASS | [M1B](2026-10-28-m1b-meter-ballistics-gates.md) |
| FM Multi Gate (63), HD Multi Gate (38) lamps | red LEDs | PASS dark; lit state UNTESTED (did not gate) | [M1B](2026-10-28-m1b-meter-ballistics-gates.md) |
| Loudness colours (amber top 33/162, green below) | Meters window | PASS (from client bitmap 397) | [M1B](2026-10-28-m1b-meter-ballistics-gates.md) |

## Processing (Edit › Processing Parameters, FM→HD Couple/Uncouple)

| Windows function | Mac app | Status | Evidence |
|---|---|---|---|
| AGC page order and write with readback | Processing window | PASS | [Phase 4](2026-10-28-ui-phase4-processing.md) |
| Every on-air field the client binds is on its page (B3 Clip Threshold, MPX Power Offset added) | Processing window | PASS (binding comparison; the two added sliders written with readback) | [M1H](2026-10-28-m1h-couple-processing.md) |
| Preset RDS page (text boxes, Use System RDS Values, modulator) | Processing › RDS | PASS (PTYN set and cleared, wire identical to the client) | [M1H](2026-10-28-m1h-couple-processing.md) |
| Every other control's range and step on hardware | Processing window | UNTESTED (ranges from the verified profile) | |
| FM->HD Couple / Uncouple | FM HD button (Meters/Processing header) | PASS (same single write as the client) | [M1H](2026-10-28-m1h-couple-processing.md) |

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
| Recall Preset (factory and user) | native Presets window › Recall | PASS | [Recall keeps the session](2026-10-20-recall-session.md), user presets told apart in [M1C](2026-10-28-m1c-preset-save-delete.md) |
| Save Preset | native Presets window › Save (replaces the on-air user preset) | PASS | [M1C](2026-10-28-m1c-preset-save-delete.md), native [M1G](2026-10-28-m1g-native-presets.md) |
| Save Preset As | native Presets window › Save As… | PASS | [M1C](2026-10-28-m1c-preset-save-delete.md), native [M1G](2026-10-28-m1g-native-presets.md) |
| Delete Preset | native Presets window › ⋯ › Delete… (or Delete key) | PASS | [M1C](2026-10-28-m1c-preset-save-delete.md), native [M1G](2026-10-28-m1g-native-presets.md) |
| Backup to PC | native Presets window › ⋯ › Back Up… (the client's plain-text files) | PASS | [M1D](2026-10-28-m1d-backup-restore.md), native [M1G](2026-10-28-m1g-native-presets.md) |
| Restore from PC | native Presets window › ⋯ › Restore from Backup… (backup folder or files) | PASS; encrypted client backups not read | [M1D](2026-10-28-m1d-backup-restore.md), native [M1G](2026-10-28-m1g-native-presets.md) |

## Tools and Help

| Windows function | Mac app | Status |
|---|---|---|
| Automation | Open Optimod Remote › Automation… (native) | PASS ([M1F](2026-10-28-m1f-automation-security.md)) |
| Security | Open Optimod Remote › Security… (native) | PASS ([M1F](2026-10-28-m1f-automation-security.md)) |
| 5700i Diagnostics | Open Optimod Remote › Diagnostics… | PASS ([M1E](2026-10-28-m1e-diagnostics.md)) |
| Operating Manual, About | — | UNTESTED |

**Model status: NOT COMPLETE.**
