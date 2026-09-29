# State, settings, coupling, routing, and presets

## Current verified state model

The reference firmware returns 222 processing fields and 178 system fields. The bundled profile supplies 366 static index-to-value mappings; 34 current fields are typed text. A match against one device's current values validates the profile at those points, not every possible hardware write or every processing structure.

Use the full [parameter profile](parameters-3.0.1.20.json) for exact value order. Important rules:

- The field's device `index` selects a profile entry; the transmitted typed value must match that entry.
- `Cent` is stored in integer hundredths and displayed with its unit separately.
- Choice labels such as `Off` are not numeric values with units appended.
- Text writes retain the current index and use `UserString`. Reject ASCII control characters and the delimiters `;`, `<`, and `>`.
- Unknown, invisible, or optional fields remain unavailable rather than receiving guessed defaults.

## Typed changes

Opcode `227` carries:

```text
NAME;INDEX;TYPE:VALUE;SCOPE;
```

Scopes are `1` for Processing and `2` for System. Numeric examples are `Int:2;` and `Cent:-125;`. Choice and text examples use `String:<value>;` and `UserString:<value>;`.

Before sending:

1. Confirm the same host and randomly generated application session token are still active.
2. Select the last confirmed AP or AS document owned by that session. Refresh it explicitly when opening/reconnecting, after preset/file operations, when recovering stale state, or for a verification run.
3. For Processing, require the expected on-air preset name.
4. Require an exact match for the prior field value, type, and index in that guarded document.
5. Resolve the requested index through the firmware-specific profile. For text, permit only a validated text value at the unchanged index.
6. Require verified write access.

After an interactive single-field send, issue opcode 250 and drain events until 251. On success, update only that field in the guarded document and preserve the immutable preset comparison baseline. If the boundary fails, take one exact AP/AS readback to determine whether the write applied; do not queue or replay it. A field that is only statically derived (the 5700i supplement, or any field of a static adapter such as the 5500 and 8700HD) always takes that readback, and a mismatch publishes the processor's value. Manual refresh, export, preset application and explicit hardware verification still require exact documents.

Do not take full terminal snapshots on both sides of every successful single-field write. The reference 5700i suspends PC Remote meter replies for roughly 570 ms while producing each AP/AS document, even though the sockets are independent. In hardware measurement, removing those two routine snapshots reduced write-related meter gaps to 103–189 ms from the processor's own change handling. Continue rendering only interpolated real meter values during that interval; never synthesize new signal.

## FM and HD are three separate concepts

1. **Display choice:** FM, HD, or Both changes only which processing pages/meters are visible.
2. **Processing relationship:** processing field `HD COUPLING` is index `0` / choice `FM->HD` or index `1` / choice `Indepen.`. Coupling applies to the HD equalizer and multiband compressor/limiter controls that have FM counterparts. Do not use an `HD ` name-prefix block: the unique controls in the original **HD Limiting** page remain separately adjustable while coupled: `IBOC EQ GAIN`, `IBOC EQ FREQ`, `IBOC LIM DR`, `HD DE ESS`, and `HD COUPLING`. The 5700i 3.0 Operating Manual states this explicitly in “Unique HD Audio Controls,” pages 3-70 through 3-72. Setting coupling to `FM->HD` makes counterpart HD controls take their FM values; switching back to `Indepen.` restores the earlier independent HD edits. Independent mode disables `LESS MORE` for that preset.
3. **Physical output routing:** system fields `AO1 SOURCE`, `DO1 SOURCE`, `DO2 SOURCE`, and `PHONES OUT SOURCE` select among values such as `FM`, `FM+Delay`, `Monitor`, and `HD` according to the profile.

Never translate a display-tab click into coupling or routing. Coupling and routing can change program audio and need their own explicit control and confirmation.

## Preset list and Recall

`LP [CODE]` returns lines ending in `factory`, `user`, or `unsaved`. Names must be unique, ASCII, 1–80 bytes, and exclude control characters, `[` and `]`. Cap the list at 1024 entries and reject an empty or malformed list.

Selecting or double-clicking a preset in the UI must not send a command. Recall requires an explicit Recall action followed by one confirmation that names the on-air and target presets; the service refuses a request without `confirmed: true`. (Until 29 September the native window treated the Recall button itself as the confirmation.) Then use this sequence:

1. Use the catalog and active AP document already owned by the current application session. Refresh them only when they are absent or explicitly stale; do not add an LP/AP preflight to every Recall.
2. Confirm the expected prior on-air name still matches.
3. Confirm the target exists and is factory or user, not unsaved.
4. Pause PC Remote heartbeat/meter polling, then send `RP NAME[CODE]` and `AP [CODE]??` in the same bounded terminal exchange.
5. Require a reply line `ON AIR: NAME` and an AP document whose name is exactly the target.
6. Publish that AP document as the processing state and new preset baseline, send opcode 222 once to restore the meter subscription, and schedule the next normal poll 50 ms later. Do not perform redundant system or catalog reads.

If confirmation fails after a write-capable exchange, one direct AP read while PC Remote polling remains paused may resolve whether Recall occurred. Never retry Recall automatically and do not drop a healthy PC Remote session merely because terminal confirmation failed. A validation or stale-state rejection before any write is safe to report without changing the session.

Local current-document export and local file application are available without guessing a named-preset command. For export, read and validate a fresh AP document and save it under an app-generated local name. For application, validate every imported field against the active profile, send verified opcode-227 writes in coupling-safe order, and require a complete final AP match before accepting the file as the new comparison baseline. Never replay an unconfirmed write.

Named on-device Save, Save As, rename, delete, previous-preset, automation and other bulk preset operations are not verified on the 5700i. Do not infer them from Recall or local file application. A backup of the AP and AS documents plus the LP names, and a field-by-field restore of system settings with AS readback, are implemented from verified reads and opcode-227 writes; they have not been run on hardware.

## Preset comparison state

Keep the first confirmed processing document as an immutable field baseline for the session, including when the device is already in a `modif …` state at connection time. A confirmed Recall or successfully applied local file replaces it with the new preset baseline. Ordinary device readback updates the live document but must not silently replace that baseline. A control is modified when its confirmed live field differs from the baseline; it remains highlighted after a successful write and clears only when restored to the baseline or after a new Recall/file application establishes a new baseline.

## Concurrency pattern

Use one actor/task as the only owner of the PC Remote session. Feed it bounded commands over a channel. A periodic 50 ms tick sends opcode 219 and drains to a 250/251 boundary. Ordinary read-only terminal work can coexist with that owner, although a full AP/AS response may still pause meter delivery inside the processor. During a write-capable RP/AP Recall exchange, pause PC Remote polling until confirmation, then resubscribe once and resume after 50 ms. Reject concurrent mutations instead of building an unverified replay queue.
