# Multi-model adapter foundation — 23 September 2026

> **Evidence status:** Implementation and local verification record. Read-only support for the listed additional models is based on official PC Remote login banners and documented terminal commands. Writes, preset recall and live-meter decoding have not been enabled without an exact hardware and firmware fixture.

## Implemented boundary

- Saved connections persist `Auto Detect` or an explicit 5700i, 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 or 9400 model.
- Existing version 1 connection files migrate atomically to version 2 and retain their identifiers, addresses and locally stored access codes.
- PC Remote login identifies each model from its own firmware banner and rejects a mismatch with an explicitly saved model.
- Native Network discovery recognizes the eleven corresponding terminal banners without sending an access code.
- The ten additional adapters may read AP/AS processing and system documents and LP preset catalogs using their model-specific `8300`, `5700`, `6300`, `8500`, `8600`, `8700`, `9300` or `9400` document family.
- Each model selects its own skin ID, local product mark, chassis and LCD palette, signal-path label and meter grouping. Unknown identity uses the neutral skin.
- Inactive layouts for models without a verified meter map contain empty wells; the UI never presents generated values as device data.

## Safety boundary

Only the exact `5700i V 3.0.1.20` adapter enables parameter writes, preset recall and the verified 112-value live-meter mapping. All ten other known banners remain read-only. Unknown models, mismatched terminal identity and unsupported document versions fail closed.

## Verification run

- Rust workspace tests, including adapter identity, model mismatch, document families and connection-file migration.
- TypeScript UI tests, including unique skin, asset, material and neutral-fallback assertions.
- Native Swift connection, window and network-discovery tests.
- Production UI build, documentation audit, Rust formatting and clippy with warnings denied.
- One-click macOS application build, installation, property-list validation and strict code-signature verification.
