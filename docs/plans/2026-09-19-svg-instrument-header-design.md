# SVG Instrument Header Design

> **Status:** Implemented. Current visual rules are maintained in [DESIGN.md](../../DESIGN.md); verification is recorded in [2026-09-20-instrument-ui.md](../verification/2026-09-20-instrument-ui.md).

## Approved reference

The user supplied a final 1132×371 SVG/2264×741 PNG, the 5700 vector identity, a high-resolution Orban mark and a Liquid Crystal Display TTF and explicitly approved these as the target appearance. The SVG supplies exact colors and coordinates; the PNG is the raster comparison. Together they define the complete upper instrument area: product identity, status display, FM/HD coupling, connection state, Orban mark and live meters. The existing connection manager and control workspace remain below this area and inherit its visual language.

## Chosen approach

Rebuild the reference with semantic React, CSS and the existing live canvas meter renderer. The SVG remains a visual reference and is not embedded as a static screenshot. Dynamic values, controls, accessibility labels and live meter data therefore remain functional.

Two alternatives were rejected:

- Embedding the supplied SVG would match a single frozen frame but could not represent real device state or accessible controls.
- Restyling only the current compact header would preserve the existing layout but would miss the large LCD, product identity and instrument-panel proportions that define the approved design.

## Geometry and visual tokens

The reference coordinate system is 1132×371. At the minimum native app width, the upper area keeps these proportions:

- approximately 112 px for the identity/status row;
- a rounded meter chassis inset 16 px from each side;
- approximately 242 px for the meter chassis;
- fixed 8 px meter lanes, 23.5 px stereo wells and 13 px mono wells;
- horizontal scrolling when the viewport cannot show every meter group without changing lane width.

Use the reference colors directly:

- app gradient from `#353432` to `#32312f`;
- meter chassis gradient from `#20201e` to `#252523`;
- meter wells `#141414`;
- LCD gradient from `#101a24` to `#182431` and LCD text `#7e95b1`;
- active reduction meters `#0088ff`;
- loudness/enhance/composite meters `#ffcc00`;
- input/output meters with green/yellow/red zones and peak marks;
- live indicator `#34c759`;
- product accent `#a62e9c`;
- scale strokes `#656565` and separator strokes `#191919`;
- primary instrument labels `#bfbfbf`.

## Header composition

The left block uses the supplied `5700 DIGITAL / OPTIMOD` SVG at x=32, y=35.2 and 120 px wide. The centre 371×44 LCD displays the on-air preset on the first line and current input/mode information on the second line. When disconnected it displays a clear unavailable state. The supplied high-resolution transparent Orban mark occupies x=974, y=32 at 118×56.4. No static full-interface screenshot is layered over the UI.

Coupling and connection use the two 56×22 pill indicators at x=579 and x=655. Their gradients, border gradients, inner/drop shadows, dots and FM–chain–HD mark follow the supplied SVG. The coupling dot shows linked/independent state and remains the guarded couple/decouple action. The connection dot shows confirmed device state; its label reads `Connected` or `Offline`.

## Meter composition

The meter renderer keeps the existing real SSE data, validated channel map, curves, stale-data clearing and animation-frame interpolation. Drawing changes only presentation:

- group backgrounds and full-height vertical separators;
- 168 px high black wells with fixed 8 px illuminated lanes;
- small grey ticks and labels placed like the reference;
- blue downward reduction bars with an independent peak marker per lane;
- yellow loudness, enhance and composite bars;
- zoned green/yellow/red input and output bars;
- fixed lane width in FM, HD and Both views.

The FM/HD/Both display selector remains available as a compact instrument control without changing physical routing.

## Typography and exactness

The supplied Liquid Crystal Display face is embedded with `@font-face` and scoped only to the blue LCD. All other instrument text uses `SF Compact Text`, with the macOS system sans-serif fallback used only when that face is unavailable. Canvas labels use the same family. Meter wells, 8 px lanes, thirteen group widths, separators, reference ticks and gradients are measured directly from the approved files. Approximate `oklch` substitutions are not used in the instrument header or meter chassis.

## Lower workspace

Connections, processing, presets, outputs and Setup retain their current information architecture and safety behavior. Their surfaces adopt the reference background, border, radius, typography and blue/magenta active states. No device field, preset workflow or write guard is removed.

## Safety and validation

The redesign must not change protocol behavior. Coupling continues to require captured field, session and preset preconditions. Visual regression covers disconnected and live states at the 1132 px reference size, 1440×900 and 1180×760. Automated tests retain exact lane/well widths, peak hold/release, ballistics, stale-data behavior, FM/HD channel separation and all 178 Setup fields. The live-device visual pass remains read-only.
