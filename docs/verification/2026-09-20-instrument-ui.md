# Final instrument UI verification, 20 September 2026

> **Evidence status:** Point-in-time instrument-geometry record. It remains the measured upper-interface baseline; later native auxiliary-window work is documented separately.

Target: the installed native macOS app, final user-supplied 1132×371 SVG/PNG composition, vector product identity, Orban mark and Liquid Crystal Display font. A live processor session populated the meters for visual inspection; no processing, routing, preset, Setup or coupling mutation was submitted.

## Reference geometry

At a 1132 px viewport, browser measurements matched the supplied coordinate system:

| Element | Measured rectangle |
|---|---:|
| Instrument header | `0, 0, 1132 × 112.39` |
| 5700 identity | `31.98, 35.19, 120 × 49.94` |
| Blue LCD | `188, 38.19, 371 × 43.98` |
| FM/HD status | `579, 48.69, 55.98 × 36.97` |
| Connection status | `655, 48.69, 55.98 × 36.97` |
| Orban mark | `974, 31.98, 118 × 56.50` |
| Meter chassis | `16, 112.39, 1100 × 242` |

The same composition scaled proportionally at 1440 px. The LCD loaded the supplied dotmatrix TTF, while all remaining interface and canvas text used SF Compact with the macOS system fallback. Both logos came from the supplied runtime assets rather than reconstructed text or CSS shapes.

## Meter behavior

- The final artwork geometry uses fixed 8 px illuminated lanes, 23.5 px stereo wells, 13 px mono wells and the thirteen measured group widths summing to 1100 px.
- Every illuminated lane and peak marker stays 2.5 px inside its 168 px well. Scale text uses the final SVG's `#656565`; separators use `#191919`.
- Input and output lanes use green/yellow/red zones; gain reduction is blue; HF, loudness and composite lanes are amber.
- Each lane maintains its own peak marker. A new high value replaces its peak immediately, the marker holds for 900 ms, then releases toward the current value at 35 percentage points per second.
- Browser animation runs independently of the approximately 50 ms device polling cadence. Malformed or data older than 1.2 seconds clears current values and peaks.
- FM, HD and Both use the same uniform reference scale; filtered canvases keep their 8 px lanes instead of stretching to fill the chassis. These remain display choices only. Physical output routing stays in Outputs.

## Automated and visual checks

- All 12 TypeScript tests passed, including exact final geometry/palette assertions, separate peak hold/release, channel mapping, stale clearing, coupling staleness and the complete 178-field Setup inventory.
- The production Vite build passed.
- A Playwright traversal covered 27 Processing/Setup states: the global interface, all ten Setup categories and every available FM and HD processing page. It separately checked FM, HD and Both meter modes.
- At 1440×900 and 1180×760, that traversal reported zero clipped control labels, zero unintended page overflow and no browser navigation exceptions.
- Direct screenshots at 1132 and 1440 px confirmed the asset sizes, LCD scope, separator positions, rounded chassis, live meter colors and peak markers.
- The 1132 px raster comparison measured the LCD inset at 10 px with a 22 px two-line glyph box. Both status labels matched the reference height and position; the FM–chain–HD label measured 38 px and `Connected` measured 49–50 px.

## Boundary

The peak is derived from the converted live value because a separate verified device peak bank has not been identified. Exact Windows-side peak timing, absolute HD/loudness calibration, device gate/overload/lock indicators and long-duration session behavior remain compatibility work. The installed development bundle is ad-hoc signed and remains outside a notarized public release workflow.
