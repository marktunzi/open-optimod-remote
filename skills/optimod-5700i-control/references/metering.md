# Live metering

## Wire behavior

Opcode 228 supplies bank 1 or 2 plus exactly 112 raw bytes. Both banks contain channels from both FM and HD paths; the bank number is not an FM/HD selector. The current UI renders bank 1. Peak/current meaning between the banks remains unverified.

Verified channel identifiers include:

| Function | Channels |
|---|---|
| Input L/R | 1, 2 |
| AGC bass/master L/R pairs | 3/10, 4/11 |
| FM multiband 1–5 L/R | 5–9 / 14–18 |
| HD limiting L/R | 19, 20 |
| FM output L/R | 27, 28 |
| Composite | 29 |
| FM HF enhance | 34, 35 |
| HD HF enhance | 36, 37 |
| HD multiband 1–5 L/R | 39–43 / 44–48 |
| HD output L/R | 51, 52 |
| HD loudness mono/stereo | 55, 56 |
| HD loudness gain reduction | 57 |
| FM loudness mono/stereo | 58, 59 |
| FM loudness gain reduction | 60 |

Use [meter-curves.ts](meter-curves.ts) for the verified numeric curves. Do not apply one generic 0–100 clamp to every channel.

| Channels | Conversion/orientation |
|---|---|
| Input 1/2 and FM output 27/28 | 101-entry level curve |
| HD output 51/52 | Separate 256-entry level curve |
| HD limiting 19/20 | 101-entry reduction curve |
| HD loudness GR 57 | Reversed reduction; raw 100 means no reduction |
| FM loudness GR 60 | Separate reversed reduction curve |
| Loudness 55/56/58/59 | Direct 256-entry lookup |
| HF enhance 34–37 | Linear and grows upward from the bottom |
| AGC/multiband GR | Linear and grows downward from the top |

Known axes cover input, FM output, HF enhance, AGC/multiband and composite. Label HD output, limiting, and loudness as relative percent until their absolute dB/LUFS axes are independently verified. The current UI derives an independent display peak from every converted lane, holds it for 900 ms and releases it at 35 percentage points per second. This is verified display behavior, not proof that bank 2 or another device field supplies peak values. Device peak semantics, gating, overload, locks and other indicators remain open.

## Cadence and animation

Use a 50 ms poll target. A short hardware run averaged 55.31 ms between distinct frames (median 51.24 ms, p95 136.29 ms, maximum 245.64 ms). Polling every 25 ms did not increase delivered frame rate.

Render at `requestAnimationFrame` cadence and interpolate converted display percentages, not raw wire state. The verified implementation uses a critically damped velocity-continuous model with 40 ms attack and 70 ms release smoothing. This bridges uneven packets without pretending to sample the processor more often.

Keep these liveness rules:

- validate exactly 112 integer bytes in the range 0–255;
- clear the motion state for malformed packets;
- clear and mark not live after 1.2 seconds without a valid frame;
- reset on disconnect or device-session change;
- bypass interpolation for reduced-motion preference;
- never animate indefinitely toward a last-known value after the stream is stale.

A short comparison reduced the longest terminal-refresh-related meter gap from about 2.85 seconds to 777 ms by continuing PC Remote polling during the terminal operation. This is evidence from a short run, not a soak guarantee.
