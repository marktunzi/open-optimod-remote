# Adding an OPTIMOD model

This checklist turns one official PC Remote installer into a writable model profile. The 5500 and 8700HD were added this way; see the [static analysis](research/2026-09-29-5500-8700hd-static-analysis.md). A model that is added from software alone stays marked as *statically derived* until someone verifies it on hardware.

## 1. Extract

Install `innoextract`, and `pip install pefile capstone unicorn`, then run:

```sh
python3 scripts/extract_pc_remote.py Setup<model>_<version>_PC_Remote.exe profiles/<model>/<firmware>
```

An unknown PC Remote build stops with its SHA-256. Add a `CONFIGS` entry for that build in `scripts/extract_pc_remote.py`:

- `ctor`: the call target that follows `push "<NAME>"; push <id>` hundreds of times.
- `assign`, `sprintf` and `itoa`: the string assignment and formatting helpers called by the conversion routines.
- `binders`: the slider (5 arguments), radio (7) and combo (3) binders. These are the most frequent call targets with immediate control IDs.
- `processing_dialogs`: the dialog resource IDs of the processing pages.
- `meter`: the subclass, init, channel and curve setters of the meter bars, their dialogs, and the channel limit from the meter store (`cmp [ebp+8], N`).
- `overrides`: constructor calls whose arguments are pushed from registers.

Use the existing 5500, 5700i and 8700HD entries as templates. For a model that already has a hardware-verified profile, set `supplement=True`: the extractor then writes `static-*.json` next to it, and the app adds only the fields and pages the verified profile lacks. Never commit anything from the installer; only the generated JSON files.

The extractor rejects a constant offset between PC Remote and the factory presets. On the hardware-verified 5700i such an offset was wrong, because the presets came from older firmware.

## 2. Review the report

Read `report.json`:

- Every excluded field must be understood.
- `transform x100` fields need a plausible reason. The runtime check refuses a write if the processor reports the other unit.
- `preset_families` must match the model's own document family. Presets from another model, such as the 8600 presets in the 8700HD package, make the cross-check weaker; say so in the documentation.

## 3. Register the model

- `crates/orban-protocol/src/adapter.rs`: a `DeviceModel` and `SkinId`, a read-only family adapter for the banner prefix, an `EXACT` entry for the exact banner with `Evidence::Static`, and the terminal banner in `identify_terminal_banner`. Set `preset_store` and `preset_delete` only when the firmware documents a save and delete command, as the 5500 does with `SP` and `DP`; add the model's preset name limit to `presets::max_store_name_length`.
- `crates/orban-protocol/src/profile.rs`: one `EMBEDDED` entry pointing to the three JSON files.
- `crates/orban-protocol/src/document.rs`: the document family if it is new.
- `packages/ui/src/skin-registry.ts`, `main-interface.css`, `public/assets/optimod-<model>.svg` and `Devices.tsx`: skin and model choice.
- `native/macos/ConnectionsAPI.swift` and `NetworkDiscovery.swift`: model choice and discovery.

The tests in `crates/orban-protocol/tests/adapters.rs` fail if a writable adapter has no embedded profile, pages or meters.

## 4. Validate the method when you can

If the model already has a hardware-verified profile, compare the extraction with it field by field, as was done for the 5700i (363 of 366 identical, none wrong). Any difference points to an extractor fault that also affects the other models.

## 5. Verify

Run the validation commands from the README. A simulated session against the new banner checks that pages, meters, one write with readback and recall work end to end. Hardware verification then follows the gate in the [multi-model research](research/2026-09-22-multi-model-optimod-support.md#verificatiepoort-per-model). Only after that may `Evidence::Static` become `Evidence::Hardware`.
