# Verification and hardware boundaries

## Evidence levels

Label claims as one of:

- **Wire/unit verified:** synthetic fixtures exercise framing, parsing, bounds, or conversion.
- **Static-reference derived:** pure data or conversion behavior recovered independently from a matching reference version.
- **Hardware read verified:** observed on firmware 3.0.1.20 without changing state.
- **Hardware write verified:** change was read back and the original state was restored.
- **Open:** not demonstrated; do not phrase as supported parity.

Current hardware evidence:

- 222 processing and 178 system fields read.
- 366 mapped and 34 typed-text current values matched.
- 75 presets listed read-only on the reference unit.
- live 112-byte meter frames and channel activity observed.
- four physical output-source values and FM→HD coupling read, not changed.
- display contrast has been changed by an automated hardware test, read back, restored, and compared across all 400 fields.
- `B2 Output Mix` has been moved by 0.1 dB and restored with exact readback while the connection and real meter stream remained live.

No routing, coupling, preset Recall, text, RDS, automation, network, security, license, or firmware write is currently hardware-verified. The B2 check is narrow evidence for the guarded processing-write path, not exhaustive processing parity.

## Test approach

For ordinary diagnostics, prefer the repository's read-only command:

```sh
cargo run --release --bin optimod-diagnose -- DEVICE_IP:6201 30
```

It prompts for the access code. Never put the code on a command line, in a fixture, screenshot, log, repository, or bug report.

The contrast checker is intentionally guarded:

```sh
cargo run --release --bin optimod-contrast-check -- DEVICE_IP:6201 --temporarily-change-display-contrast
```

Run write tests only when the user's authorization covers the concrete change and the device is in an appropriate test or maintenance state. Capture the complete baseline first, select a reversible non-audio field when possible, write one change, read it back, restore it, read it back again, and compare all fields. A TCP success alone is not proof.

For audio-affecting features, use synthetic/mock tests until a safe hardware window or test processor is available. Opening a confirmation dialog or switching a local view must send zero device mutations.

## Repository validation

In Open Optimod Remote, use:

```sh
npm ci --prefix packages/ui
node --experimental-strip-types --test packages/ui/tests/*.test.ts
npm run build --prefix packages/ui
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

On the reference Mac, `DEVELOPER_DIR=/Library/Developer/CommandLineTools` avoids depending on an unaccepted full-Xcode license.

## Remaining compatibility gates

- exact screen and interaction comparison with the matching Windows release;
- representative safe writes for processing, coupling, routing, Recall, and text;
- alternate Two-Band/Five-Band structures and context-sensitive field visibility;
- current/peak semantics, absolute HD/loudness axes, peak/gate/overload/lock indicators;
- restricted access levels and external/front-panel changes;
- complete preset management, backup/restore, automation, maintenance, and remaining RDS workflows;
- reconnect/fault injection, sleep/wake, an eight-hour soak, and release packaging.

Do not call the implementation a complete Windows replacement until these gates have evidence.
