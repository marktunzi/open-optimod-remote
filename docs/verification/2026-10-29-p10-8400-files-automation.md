# Phase 10: 8400 Backup, Restore and Automation; 6300 and 8700i file functions on firmware

> **Evidence status:** 29 October 2026. Original `8400_pc.exe` (PC Remote 3.0, unmodified, CrossOver bottle "Optimod 8400", driven only through its own key messages; screens read with `screencapture -l`) against the virtual 8400 running original firmware 3.0.5.7 (192.168.1.176). The app's checks ran in isolated service instances (ports 15702–15706, own data folders) against the virtual 8400, 6300 4.1.1.19 (.173) and 8700i 1.5.1.11 (.175). Each unit was copied before and compared afterwards; all ended equal to their baselines. The owner's app and settings were not used.

## 8400: what the original client does

| Function | Client (capture) | Unit messages |
|---|---|---|
| Backup (*Save/Save As › Memory › Copy All*) | 18 files `<Preset Name>.orb`, Windows line ends, in its `user` folder | **none**: written from the presets it read at login |
| Restore (*Memory › Copy* from the PC list) | `OOR REST.orb` from the PC list | `chdir user`, write calls 24, 28, 8, 30, 27 (the text with Unix line ends), 29, 26 to `OOR REST.orb`; then `16391;18;OOR REST;` |
| Automation Add | event *every day, 00:00:00, CLASSICAL-2 BAND* | `AUTO_0.aut` through the write calls, then `16389;784;AUTO_0.aut;`; during the edit `AUTOMATION EVENT;1;UserString:<CLASSICAL-2 BAND>;` |
| Automation Delete | the event | `remove AUTO_0.aut`, then `16389;784;AUTO_0.aut;` |
| Automation Enabled/Disabled | the list's left button | `AUTOMATION ENABLED;1;String:<Enabled>;` / `;0;String:<Disabled>;` |

The client's copies equal the unit's files for presets in the current format; for 16 older ones it adds controls the file lacks with its own defaults and renumbers one stale index (details in [docs/protocol/8400.md](../protocol/8400.md)).

## 8400: the app on firmware

| Check | Result |
|---|---|
| Backup | **PASS**: 19 files, exactly the client's file names; 3 byte-identical to the client's copies, the other 16 differ only by the client's added defaults (the app keeps the unit's contents) |
| Restore | **PASS**: `OOR REST` deleted, then restored from the backup (5 s): listed again, and the unit's file is **byte-identical** to the one the client's Copy produced |
| Automation write format | **PASS**: the app renders the client's `AUTO_0.aut` byte for byte (`renders_the_8400_clients_new_event`); events written, read back and cleared through the service; the unit opened the new file after the automation event |
| Automation Enabled | **PASS**: set and cleared through the service, read back from `system.orb` |
| Automation execution | **BLOCKED**: with automation enabled, neither an event written by the app (TEST 02) nor one in the client's exact form (CLASSICAL-2 BAND, index 1) went on air at its time on the virtual unit. Whether the unit runs an event by the action's name or by its index in the client's list could therefore not be seen; the app writes both as the client does |
| Session drops of the original client | seen twice during the capture: the unit stopped acknowledging the client's keepalives and closed the session 30 s later (*The 8400 Has Disconnected*), as described in [8400 session stability](2026-10-29-8400-session-stability.md) |

The native Presets window now offers *Back Up…* and *Restore from Backup…* as files for the 8400 and the 2300 (the 2300's service side was done before but not reachable from the window).

## 6300, 8700i and 8700HD file functions on firmware

| Function | 6300 4.1.1.19 | 8700i 1.5.1.11 | 8700HD 1.0.2.161 (.171) |
|---|---|---|---|
| Save As | PASS (listed, on air, 2 s) | PASS | PASS |
| Backup | PASS: 14 files, each equal to the unit's own | PASS: 3 files, each equal to the unit's own | PASS: 13 files, each equal to the unit's own |
| Delete | PASS (gone from list and unit) | PASS | PASS |
| Restore | **FAIL → fixed**: the file was written but the app waited for a boundary answer the 6300 never sends after a 231 upload. With `restore_answered` false: PASS (1 s, listed) | PASS; its packet log shows no extra answer after the upload either | PASS (1.5 s); no extra answer after the upload |
| Security | PASS: passcode added, read back, removed; file equal to the baseline | PASS | PASS |
| Diagnostics | PASS (205 text) | PASS | PASS |

Physical 8400, 6300, 8700i and 8700HD hardware: **BLOCKED** (none available).
