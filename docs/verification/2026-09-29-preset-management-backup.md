# Preset management, backup and model-aware menus — 29 September 2026

> **Evidence status:** Implementation and local verification record without hardware. Every check below ran against automated tests, a local protocol simulator or headless Chromium, not against a processor.

## Implemented boundary

- **5500 (`5500 V 1.2.8.24`):** save the on-air processing as a user preset (`SP NAME[code]`), rename the unmodified on-air user preset (save, then delete) and delete a user preset that is not on air (`DP NAME[code]`). The capabilities `preset_store` and `preset_delete` are set only for this adapter. The command is followed by AP in one terminal transaction; a refusal in the reply is an error, and a fresh LP list decides success. Nothing is retried.
- **Refused before sending:** factory or existing names, names over 18 characters (the 5500 PC Remote limit), leading or trailing spaces, the `modif ` prefix, brackets, deleting a factory or on-air preset, and renaming a preset that is not on air unmodified.
- **5700i and 8700HD:** no on-device save, rename or delete. Their firmware has no terminal command; see the [static analysis](../research/2026-09-29-5500-8700hd-static-analysis.md#presets-op-het-apparaat-opslaan-en-verwijderen).
- **Backup (every writable model):** `GET /api/backup` refreshes the AP and AS documents and the preset list and returns them as one JSON file.
- **Restore:** `POST /api/backup/setup-plan` lists the system settings a restore would change and the ones it skips, without contacting the processor. `POST /api/backup/restore-setup` writes each planned field once with opcode 227 and reads AS back up to four times. Network settings, the running clock and `ACTUAL` status values are never restored.
- **Presets workspace:** now also used by the macOS app (Presets button and Shift-Command-P). It adds on-device actions where available, preset files with the model's extension and backup/restore.
- **Command-,:** asks the interface first, so the 5500 and 8700HD open the Setup workspace and the 5700i opens its native window. The native Settings window reloads its profile when the adapter changes.
- **Native preset file panels:** use the connected model's name and extension, and show the file name instead of the literal `(url.lastPathComponent)`.

## Verification run

- `cargo test --workspace` (76 tests), `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --all -- --check`: passed. New tests cover:
  - the 5500 save and delete confirmed by the list, and rename as save then delete;
  - seven refusals that send no command;
  - an ignored delete that is reported from the list and sent only once;
  - the 5700i and 8700HD refusing preset changes;
  - capability flags per adapter, and the `SP`/`DP` command and name rules;
  - the restore plan (network port, clock and unknown fields skipped; out-of-profile index refused);
  - a restore that writes once and succeeds or reports a mismatch from the readback;
  - applying a preset file (one write for one difference, the stale-preset refusal). Until now no test covered that.
- `node --experimental-strip-types --test packages/ui/tests/*.test.ts`: 53 passed, including which actions are offered, name validation, file extensions and backup validation.
- `npm run build --prefix packages/ui` (includes `tsc`) and `python3 scripts/check-docs.py`: passed.
- **Simulator run:** a local simulator speaking the PC Remote framing and the 5500 terminal, including `SP`/`DP`, was driven by headless Chromium through the Presets workspace:
  - saved EVENING, renamed it to NIGHT and deleted MY SOUND;
  - found delete disabled for the on-air preset;
  - saved a backup, changed `CONTRAST` and restored it from the backup file, which the restore reported and the state confirmed.
- **Not run in this Linux environment:** the Swift test programs and the macOS app bundle. The Swift changes are the Command-, and Shift-Command-P routing, the profile cache key, the file panels and one added `PresetFileFormat` test.

## Not verified

- Whether a real 5500 accepts `SP` and `DP` in this form and what it answers on success.
- Restore on real hardware.
- The Swift changes (not compiled here).
