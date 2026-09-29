---
name: optimod-5500-control
description: Use when implementing, diagnosing, or verifying control of an Orban OPTIMOD 5500, including its PC Remote session, processing and system parameters, presets, FM/stereo-encoder UI, writes, and live meters.
---

# OPTIMOD 5500 Control

Use this model skill for the OPTIMOD 5500 only. Use `optimod-5700i-control` for the shared PC Remote framing and safety rules, then apply the narrower facts here. Never show or send 5700i HD controls to a 5500.

## Target

- Exact login banner `5500 V 1.2.8.24`: firmware 1.2.8.24 with PC Remote 1.2.8.24 from the same installer. Any other 5500 firmware is read-only.
- Terminal banner `Orban Optimod 5500`; document family `8300.10`; PC Remote TCP 6201; terminal TCP 23.
- The profile is **statically derived, not hardware-verified**. Read [evidence.md](references/evidence.md) before changing compatibility status.

## References

- [parameters-1.2.8.24.json](references/parameters-1.2.8.24.json): 256 wire names with scope, index-to-value table and per-field `evidence` (`confirmed`, `transform …` or `unobserved`). It is identical to `profiles/5500/1.2.8.24/parameters.json`.
- [meters-1.2.8.24.json](references/meters-1.2.8.24.json): meter groups, channel numbers, orientation and raw-to-percent curves.
- [processing-observed-1.2.8.24.json](references/processing-observed-1.2.8.24.json): the factory-preset observations used for the cross-check.
- [worksheet-1.2.7.json](references/worksheet-1.2.7.json): worksheet labels, ranges and units. They are presentation labels, not wire names.

Regenerate the profile with `scripts/extract_pc_remote.py`; never edit values by hand.

## Wire facts that differ from what PC Remote displays

The processor documents several values in a different unit than PC Remote shows. Always send the firmware format:

- `B1`–`B4 ATTACK`, `B1`–`B4 LIMIT ATTACK` and `SE RATIO WIDTH`: PC Remote shows `Int n`; the factory presets store `Cent n×100`, and the profile uses the preset format. The runtime check refuses a write if the processor reports the other format.
- Not writable, because PC Remote and the factory presets disagree: `AGC DIFF GR`, `AGC RATIO`, `DWNWRD EXP`, `B3 CLIP THRSH`, `PEQ LOW/MID/HIGH WIDTH`, `B12 CROSSOVER` and `INPUT EMPH STATUS`. Constant offsets between the two are never trusted: on the hardware-verified 5700i such an offset was wrong.

## Model shape

An FM processor with two-band and five-band structures, stereo enhancer, AGC, equalizer, multiband compressors, band mix, distortion control, final clipping, clipper options, speech mode, stereo encoder, composite output and diversity delay. There is no independent HD chain and no FM→HD coupling.

Meters: input 1/2, AGC B/M 3/4, HF enhance 25, five-band gain reduction 5–9, MPX limiter G/R 26, MPX power 77, output 27/28, composite 29, two-band gain reduction 38/39, HF limiting 21/22, overshoot limit 23/24. Records may carry up to 79 values.

## Safety rules for this model

1. Write only after the live AP/AS document contains the field and its current value matches the profile at the current index.
2. After every write, read the full AP or AS document back. If the value differs, show the processor's value and never retry.
3. Recall uses `RP NAME[CODE]` plus AP and requires `ON AIR: NAME`.
4. Only a hardware check on one exact firmware can turn this into a verified profile. Start with one reversible system setting, then one processing setting, then recall.
