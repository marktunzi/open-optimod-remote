# Open Optimod Remote

## Register

product

## Users

OPTIMOD operators who need local control from macOS or a browser. The initial models are the OPTIMOD 5500 (1.2.8.24), 5700i (3.0.1.20) and OPTIMOD-FM 8700HD (1.0.2.161), all with parameter writes, preset recall, live meters and every system setting. The 5700i is verified on hardware; the 5500 and 8700HD profiles are statically derived and labelled as such until they are. More models follow. The read-only PC Remote expansion covers the 5500i, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 9300 and 9400, and other firmware of the three writable models. Other operators should be able to build and contribute to an independent open-source implementation.

## Product Purpose

Implement the approved compatibility plan in docs: the Windows PC Remote workflows, settings and live meters, including separate FM and HD views. The current implementation is a development build. Completion means verified feature parity, not merely displaying parameters.

## Brand Personality

Precise, familiar, restrained. Use the device terminology and the existing Windows tab and control organization. Independent project branding, own graphics, no proprietary binaries or runtime.

## Anti-references

The user explicitly excludes CrossOver. The approved plan excludes a generic modern dashboard that rearranges the Windows functions, simulated meters, and a claim of completeness while controls are missing.

## Design Principles

Preserve Windows control order and interaction. Distinguish visible FM/HD selection from device coupling and routing. Show confirmed device values and explicitly identify pending, unknown, stale and disconnected states. Keep metering responsive without delaying control acknowledgements. Preserve unknown data when reading documents.

## Accessibility & Inclusion

The approved plan requires keyboard control, Tab and Ctrl-Tab navigation, +/- and arrows, Retina scaling and a reference canvas of at least 1024 by 768 logical pixels. Expose accessible control names, visible keyboard focus and textual connection state. Do not rely solely on meter color to communicate loss of data.

## Updated user direction

The supplied Windows PC Remote screenshot remains the functional reference for control organization and simultaneous grouped FM/HD meters. The approved instrument artwork is the visual reference for the main processing window. Connections, Presets and System Settings use native macOS windows. Multiple named connections and their settings persist locally. Access codes live outside connection metadata in an owner-only Application Support credential file and appear only as bullets; the app does not use Keychain.

FM → HD coupling is a deliberate processor change with confirmation. FM/HD/Both meter tabs are display choices, and analog/digital/headphone source selectors control physical routing. Setup uses clear functional groups. Meter movement follows real packets with smooth browser-frame interpolation so an uneven device cadence does not appear as large visual steps.

Future model support uses explicit transport, firmware, parameter, meter and UI-capability adapters. The 8200 serial, 8400 TCP/UDP, later PC Remote TCP and current HTML5 Web UI generations must remain separate where their verified protocols differ. An unidentified model or firmware can never fall back to the 5700i write profile.

Every processor model has its own skin and product identity. A skin defines the model logo, chassis material, gradients, LCD colors, signal-path state, meter grouping and visible control pages. Skin selection follows live model detection, not a generic theme choice.
