---
name: optimod-5500-control
description: Use when implementing, diagnosing, or verifying control of an Orban OPTIMOD 5500, including its PC Remote session, processing and system parameters, presets, FM/stereo-encoder UI, writes, and live meters.
---

# OPTIMOD 5500 Control

Use this model skill for the OPTIMOD 5500 only. Use `optimod-5700i-control` for the shared PC Remote framing and safety rules, then apply the narrower facts here. Never show or send 5700i HD controls to a 5500.

## Evidence target

- Official PC Remote package: `1.2.8.24`; executable banner prefix: `5500 V `.
- The package-aligned `5500 V 1.2.8.24` target is a candidate until captured from hardware; do not turn that package association into a write claim.
- Official worksheet: `1.2.7`; processing document family: `8300.10`.
- Default PC Remote port: `6201`; terminal/status port: `23`.
- Read [evidence.md](references/evidence.md) before changing compatibility status.

## Parameter references

- Read [processing-observed-1.2.8.24.json](references/processing-observed-1.2.8.24.json) for the 108 processing wire names and the values/indexes observed across 294 official factory presets.
- Read [worksheet-1.2.7.json](references/worksheet-1.2.7.json) for 322 worksheet labels, ranges, units, sections, and setup functions.

The preset reference proves only observed processing values. It is not a complete enumeration of every value the processor accepts. Worksheet labels are presentation labels and must not be treated as wire field names until matched to a live AP/AS document or an independent protocol fixture.

## Model shape

The 5500 is an FM processor. Its interface needs FM processing, two-band and five-band structures, stereo enhancement, AGC, equalizer, multiband/compressors, band mix, distortion/final clipping, stereo encoder, composite output, I/O calibration, test, network/remote, silence/fallback, tally/GPI and diversity-delay setup where the connected hardware exposes it. It has no independent 5700i-style HD processing chain or FM-to-HD coupling control.

Only show a control when all of these match the active session: detected 5500 model, supported firmware profile, document scope, exact field name, field type, current `D` index, and an allowed value/index mapping. Build pages from the 5500 profile and the fields present in the connected processor.

## Current implementation boundary

Open Optimod Remote can identify the 5500, validate `8300.10` AP/AS documents, list LP presets, select its own skin, and reject a model mismatch. Parameter writes, recall, and live-meter decoding remain disabled until an exact hardware fixture proves the write acknowledgement, complete value ranges, meter records, and reconnect behavior.

Do not enable writes from factory-preset observations alone. Before enabling them for one exact firmware, capture a live AP and AS document, verify one reversible system write, verify one processing write, recall and restore a preset, map every meter lane, and confirm that control/meter sessions remain alive.
