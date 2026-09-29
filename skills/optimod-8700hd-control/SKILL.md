---
name: optimod-8700hd-control
description: Use when implementing, diagnosing, or verifying control of an Orban OPTIMOD-FM 8700HD (not the 8700i), including its PC Remote session, FM and HD processing parameters, FM→HD coupling, MX and speech structures, presets, writes, and live meters.
---

# OPTIMOD-FM 8700HD Control

Use this model skill for the OPTIMOD-FM 8700HD only. The 8700i is a different model with its own banner (`8700i V`). Use `optimod-5700i-control` for the shared PC Remote framing and safety rules, then apply the narrower facts here.

## Target

- Exact login banner `8700HD V 1.0.2.161`: firmware 1.0.2.161 with PC Remote 1.0.2.161 from the same installer. Any other 8700HD firmware is read-only.
- Terminal banner `Welcome to the Orban Optimod-FM 8700HD.`; document family `8700.51`; PC Remote TCP 6201; terminal TCP 23.
- The profile is **statically derived, not hardware-verified**. Read [evidence.md](references/evidence.md) first.

## References

- [parameters-1.0.2.161.json](references/parameters-1.0.2.161.json): 524 wire names with scope, value tables and per-field `evidence`. It is identical to `profiles/8700hd/1.0.2.161/parameters.json`.
- [meters-1.0.2.161.json](references/meters-1.0.2.161.json): meter groups, channels, orientation and curves.
- Pages: `profiles/8700hd/1.0.2.161/layouts.json`. There are 23 pages, including MX distortion, MX speech and the HD equalizer, multiband, compressors, band mix, 2-band and speech pages.

## Model shape

An FM+HD processor with a separate HD chain. `HD COUPLING` is `FM->HD` or `Indepen.`. Like the 5700i, coupled mode makes the HD counterparts follow FM, while `IBOC EQ GAIN`, `IBOC EQ FREQ`, `HD DE ESS` and `HD COUPLING` stay independent. The FM side also has MX and ULL structure controls (`MX …`, `ULL SWITCH`, `STD SWITCH`). MX is standard on the 8700HD (its 1.0.2 readme describes the MX presets); on the 5700i it is a paid upgrade. A control whose field is absent from the live document is shown as unavailable.

Not writable, because PC Remote and the factory presets disagree: `DWNWRD EXP`, `B5 DWNWRD EXP`, `HD DWNWRD EXP`, `B3 CLIP THRSH`, `MX BASS CLIP`, `B12 CROSSOVER`, `HD B12 CROSSOVER`, `IBOC LIM DR` and `MPX PWR OFFSET`.

## Presets on the processor

The 8700HD firmware documents no terminal save or delete command. Its PC Remote keeps presets in a local folder and syncs it with file messages whose effect is unverified, so do not send them. Offer recall, preset files (`.orb86user`, a plain AP document) and the backup/restore of system settings instead.

## Safety rules for this model

The rules are the same as for the 5500:

1. Write only after the live field and its current value match the profile.
2. Read the full document back after every write. On a mismatch, show the processor value and never retry.
3. Recall uses `RP` plus AP and requires `ON AIR:`.
4. Only a hardware check on this exact firmware can mark the profile verified.
