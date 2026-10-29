# Phase 9: platform QA of the 0.3.0 release candidate

> **Evidence status:** run on 29 October 2026 on release candidate **0.3.0 build 9** (commit `168144a`, Developer ID, notarized; DMG SHA-256 `9274b4ad…e0e1`). Real-app tests used an identical twin of that commit with its own bundle identifier, data directory and port (`org.openoptimod.remote.p8test`, 15701), connected to three virtual units running original firmware without audio devices: 6300 4.1.1.19 at .173 and .188, and 8400 3.0.5.7 at .176. The owner's installed app, preferences, connections and credentials were not used, and their checksums were identical before and after. Nothing was published.

## Suites on the candidate source

| Suite | Result |
|---|---|
| Rust (`cargo test --workspace --release`) | 151 passed, 1 failed: `one_meter_port_serves_every_8400_by_its_address` needs UDP 16540, which the soak app held at the time (it passed before the soak; re-run below) |
| Clippy `-D warnings`, rustfmt | PASS |
| UI (`node --test`, `tsc -b`, build) | 75/75 PASS |
| Swift test programs (10) | PASS, including `UpdateCoreTests` and `ConnectionsWindowTests` |
| Documentation audit | PASS |

## Real-app matrix (candidate twin)

| Item | Result |
|---|---|
| Three processors at once, two of the same model (6300 ×2) and the 8400 (UDP meters) | PASS: three tabs, three sessions, three TCP connections, one login per unit (01) |
| Preset recall in one tab | PASS: COUNTRY-LIGHT recalled on 6300 .188 after the confirmation sheet (02); its revision went 1 → 2, while 6300 .173 (CLASSICAL-2 BAND) and the 8400 kept revision 1. Restored to CLASSICAL-2 BAND and read back |
| Update check against the candidate's own signed appcast | PASS: *Up to date* (0.3.0 build 7/9 vs the feed's same build) |
| Gatekeeper on a separate Mac (Mac mini, macOS 26.6.2), quarantined DMG of the candidate | PASS: DMG and app *Notarized Developer ID*, stapled, `syspolicy_check distribution` passes |
| Connections window, selected row | Fixed during this phase: the green status and the info button vanished on the blue selection; now white (04) |
| Transient *No route to host* on one unit after a quick disconnect/reconnect | Seen once on .173 (ARP and ping fine moments later); the next Connect succeeded. Not retried automatically, by design |

## Fixed during this phase (owner's review)

- **Release safety:** the original 5500 (1.2.8.24, static evidence only) is read-only. A single service guard (`require_writes`) now also covers parameter changes, automation and passcode files, Setup open/save, preset-file restore and applying a preset file. Before, these checked only the access level.
- **Gate lamps on every model:** they were drawn only for the 5700i. Each client registers its meter controls as `push INDEX; push "o_name"; call`. The extractor now reads the `o_gateIndicate*` lamps from that table, and on the 5700i it gives exactly the firmware-proven 62/63/38. Per client: 6300 69/70, 8700HD and 8700i 62/63 (HD 38), 5500i and 5500 36/37, 2300 38/39 (03). Not covered: the 8400, whose client is a different program, and the 8700i's ITU-412 lamp (111), because its group is not established. Whether these lamps light correctly needs live audio.
- **Meter captions:** lane, channel and unit labels now share one 8 px baseline (they were 6 px on three heights), and units are centred under the bars.
- **Processing Settings *Not connected* strip:** now follows the Figma design.

## Open questions recorded for Phase 10

- **6300 Loudness Level dB vs LU (owner's question):** the bars use different channels, exactly as the client registers them: dB = `o_LoudnessLevelL/R` (61/62), LU = BS.1770 `o_ST_BS1770LdnsLvl_1_l/r` (65/67). The client also has a short-term pair (66/68). Both share one tick scale here. The scratch units have no audio, so their values say nothing (LU pegged at 255). This needs a live-audio comparison with the original 6300 client.
- Gate lamps lit-state on the 6300, 8700HD, 8700i, 5500i and 2300 (static mapping; needs live audio).

## 8-hour multi-session soak

Started 09:13:10 on the build-9 twin with the three sessions above, sampled every minute: app and service memory, threads, open files, TCP connections to the units, session state and revision per processor, the units' own login and connection counters, and error-log entries. Result: *to be added when the run ends*.

## Screenshots

![Three processors in tabs, 8400 selected](../images/p9-01-three-processors-8400.png)
![Recall confirmation in the Presets window](../images/p9-02-recall-confirm.png)
![6300 meters: gate lamps, one caption row](../images/p9-03-6300-gates-captions.png)
![Connections with two connected processors](../images/p9-04-connections.png)
