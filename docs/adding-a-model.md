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

Use the existing 5500 and 8700HD entries as templates. Never commit anything from the installer; only the generated JSON files.

## 2. Review the report

Read `report.json`:

- Every excluded field must be understood.
- `transform` fields need a plausible reason.
- `preset_families` must match the model's own document family. Presets from another model, such as the 8600 presets in the 8700HD package, make the cross-check weaker; say so in the documentation.

## 3. Register the model

- `crates/orban-protocol/src/adapter.rs`: a `DeviceModel` and `SkinId`, a read-only family adapter for the banner prefix, an `EXACT` entry for the exact banner with `Evidence::Static`, and the terminal banner in `identify_terminal_banner`.
- `crates/orban-protocol/src/profile.rs`: one `EMBEDDED` entry pointing to the three JSON files.
- `crates/orban-protocol/src/document.rs`: the document family if it is new.
- `packages/ui/src/skin-registry.ts`, `main-interface.css`, `public/assets/optimod-<model>.svg` and `Devices.tsx`: skin and model choice.
- `native/macos/ConnectionsAPI.swift` and `NetworkDiscovery.swift`: model choice and discovery.

The tests in `crates/orban-protocol/tests/adapters.rs` fail if a writable adapter has no embedded profile, pages or meters.

## 4. Verify

Run the validation commands from the README. A simulated session against the new banner checks that pages, meters, one write with readback and recall work end to end. Hardware verification then follows the gate in the [multi-model research](research/2026-09-22-multi-model-optimod-support.md#verificatiepoort-per-model). Only after that may `Evidence::Static` become `Evidence::Hardware`.
