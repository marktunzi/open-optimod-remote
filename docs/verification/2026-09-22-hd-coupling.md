# FM→HD coupling and unique HD limiting controls — 22 September 2026

> **Evidence status:** Current point-in-time read-only evidence for coupled HD control visibility. It is not a hardware write verification of coupling.

The official *OPTIMOD 5700i Version 3.0 Operating Manual* was checked against the processing dialogs extracted from 5700i PC Remote 3.0.1.20 and the live field document from the reference processor.

The manual states on pages 3-70 through 3-72 that FM→HD coupling makes the HD equalizer and multiband compressor/limiter controls with FM counterparts follow those counterparts. It also states that only the controls in PC Remote's **HD Limiting** page remain separately adjustable while coupled. The extracted dialog contains these five fields:

- `IBOC EQ GAIN` — HD EQ Gain
- `IBOC EQ FREQ` — HD EQ Frequency
- `IBOC LIM DR` — HD Limiter Drive
- `HD DE ESS` — HD De-Esser
- `HD COUPLING` — FM→HD Control Coupling

The UI now keeps **HD Limiting** in the processing tabs while coupling is `FM->HD`; the FM/HD path selector remains reserved for independent processing. Frontend and backend use the same field classification. A coupled preset import omits only counterpart HD fields and continues to write and verify these five unique controls.

Read-only hardware verification used an OPTIMOD 5700i running firmware `5700i V 3.0.1.20`. The processor reported `HD COUPLING = FM->HD` and returned all five HD Limiting fields in its active processing document. The installed macOS app remained connected with no reported error. No processor setting or preset was changed during this verification.

Automated verification passed 43 UI tests, 47 Rust protocol/backend tests, the production TypeScript/Vite build, Rust formatting and Clippy, and all eight native macOS test executables. The rebuilt bundle was installed at `/Applications/Open Optimod Remote.app`, its nested code signatures and property list were verified, and it reconnected to the reference processor.
