# 5700i meter display conversions

Target: PC Remote / processor 3.0.1.20. The runtime contains only functional numeric conversion facts and independently written rendering code. It does not load the reference executable or artwork.

The bargraph's reference conversion uses `table[maxIndex - rawByte]`. The embedded arrays in `packages/ui/src/meter-curves.ts` are already normalized into direct raw-byte order. The loudness helper uses a different direct lookup. Applying one 0–100 clamp to every channel is incorrect.

| Channels | Conversion | Static reference |
|---|---|---|
| Input 1/2, FM output 27/28 | 101-entry level curve | table 0x58b850, conversion 0x442d70 |
| HD output 51/52 | 256-entry level curve | table 0x58c448; binding 0x4458fc / 0x445957 |
| HD limiting 19/20 | 101-entry reduction curve | table 0x58bf80 |
| HD loudness GR 57 | Reversed reduction; raw 100 = no reduction | table 0x58b520 |
| FM loudness GR 60 | Separate reversed reduction curve | table 0x58b6b8; helper 0x444440 mode 1 |
| FM/HD loudness 55/56/58/59 | Direct 256-entry lookup | table 0x58b120; helper 0x444440 mode 2 |
| HF enhance 34/35/36/37 | Linear, rises from bottom | initialization 0x442810 orientation 2 |
| AGC and multiband GR | Linear, grows downward | initialization orientation 1 |

Reference bargraph setup is 0x442eb0. The 0x444190 helper binds two loudness measurements and selects mode 2. The 0x444260 helper binds FM limiter/loudness measurements and selects mode 1; its second measurement is channel 60, converted separately from limiter channel 26. FM and HD loudness GR do not use identical tables.

Canvas rendering uses a critically damped, velocity-continuous interpolation at the browser refresh rate. The attack and release smoothing times are 40/70 ms; these describe display motion and are not additional device measurements. Every displayed lane derives an independent peak from its converted display value, holds it for 900 ms, then releases it at 35 percentage points per second. A missing, malformed or stale stream clears both current and peak state after 1.2 seconds. Reduced-motion preference removes interpolation but retains peak behavior. The polling target is 50 ms. A short live run averaged 55.3 ms between distinct incoming frames (median 51.2 ms, p95 136.3 ms); a 25 ms request period did not improve that device-limited cadence and was reverted.

Input, FM output, HF, AGC/multiband and composite axes follow known reference scales. HD output, limiting and loudness axes retain the labels shown in the final supplied instrument reference, but their absolute calibration still needs simultaneous PC Remote verification. There are no simulated live values. Peak hold is display-derived rather than a separately identified device peak bank; device gating, overload/lock indicators and a simultaneous Windows comparison remain outstanding.
