---
name: optimod-5700i-control
description: Use when connecting to, diagnosing, implementing control software for, or safely testing an Orban OPTIMOD 5700i or adapting its verified Ethernet work to a 5500/5500i, 5700 FM/HD, 6300, 8500, 8600, 8700i, 9300, 9400 or other model, including PC Remote framing, terminal snapshots, presets, parameter changes, FM/HD coupling, output routing, live meters, model detection and skin boundaries.
---

# OPTIMOD 5700i Control

Use the independently verified 5700i protocol facts in this skill. Keep observed behavior, reference-derived mappings, and hypotheses clearly separated. The verified target is firmware `3.0.1.20`; reject a different model or firmware until it has its own fixtures and profile.

Open Optimod Remote now has read-only adapters for 5500i, 5500, 5700 FM, 5700 HD, 6300, 8500, 8600, 8700i, 8700HD, 9300 and 9400. They may reuse proven login/framing and evidence-backed AP/AS/LP reads, but their writes, recall and meter decoding stay disabled until the model has its own profile. The exact banners `5500 V 1.2.8.24` and `8700HD V 1.0.2.161` have statically derived, writable profiles. The 5700i itself also carries 199 statically derived fields (two-band, MX, clock, test tones and more; MX is a paid upgrade, so its fields exist only on upgraded units) in `profiles/5700i/3.0.1.20/static-parameters.json`, next to the unchanged verified profile. The same extraction reproduced 363 of the 366 verified mappings and got none wrong. Use the `optimod-5500-control` and `optimod-8700hd-control` skills for those models, and `docs/adding-a-model.md` to add another. Every model also owns a separate skin ID and must never inherit the 5700i visual/control profile by fallback.

## Route the task

- Read [protocol.md](references/protocol.md) for login, framing, opcodes, archives, terminal commands, parsing limits, and disconnect behavior.
- Read [state-and-controls.md](references/state-and-controls.md) before implementing field editors, FM/HD coupling, physical routing, preset Recall, or any write.
- Read [metering.md](references/metering.md) for meter banks, channel bindings, conversions, polling, animation, and known unknowns.
- Read [verification.md](references/verification.md) before testing against hardware or claiming compatibility.
- Read [model-families.md](references/model-families.md) before adapting this work to an 8200, 8400, 8500, 5500/5500i, 5700 variant, later PC Remote model, or HTML5-generation OPTIMOD.
- Consult [parameters-3.0.1.20.json](references/parameters-3.0.1.20.json) only when an exact field index, value sequence, or unit is needed. Consult [static-parameters-3.0.1.20.json](references/static-parameters-3.0.1.20.json) for fields outside the verified set (two-band, MX, clock, test tones); each carries an `evidence` value, and writes to them must be confirmed by readback. Consult [meter-curves.ts](references/meter-curves.ts) when implementing the verified display conversions.

## Non-negotiable invariants

1. Use one owner for the PC Remote socket. Preserve frame boundaries across fragmented and coalesced TCP reads.
2. Treat corrupt framing, an unknown model/firmware, malformed documents, and unrecognized access levels as fail-closed conditions.
3. Never infer that an undocumented opcode is read-only. Opcodes `226` and bulk preset operations may save or flush state.
4. Before an interactive single-field change, compare the expected host, connection-session token, on-air preset for processing changes, field type, value, and index against the last confirmed document owned by the session. Use a fresh AP/AS document for manual refresh, preset/file operations, explicit verification, or stale-state recovery.
5. After an interactive change, cross an ordered 250/251 response boundary and update the guarded cache. If that boundary fails, or the field is only statically derived, use one exact AP/AS readback to resolve the outcome. Never surround every successful single write with full terminal snapshots: firmware 3.0.1.20 pauses live meters while producing them. A timeout has an unknown outcome; never retry automatically or disconnect an otherwise healthy PC Remote session solely because terminal confirmation failed.
6. Keep local FM/HD/Both display selection separate from `HD COUPLING` and from physical output-source fields.
7. Treat real meter packets as authoritative. Interpolate only display motion, clear stale data, and never invent signal.
8. Do not log access codes, obfuscated login bytes, decrypted station documents, saved presets, or private device addresses. Zeroize credential buffers and use Keychain or an equivalent owner-only local secret store. If the user explicitly rejects Keychain prompts, an app-local credential file must be separate from connection metadata, restricted to the current user, and rendered only as bullets.
9. Device-supplied file names may be Windows paths. Accept them only as bounded metadata and never use them as local filesystem paths; generate local names in the app.

When working in Open Optimod Remote, prefer its tested Rust implementation under `crates/orban-protocol` and `crates/orban-web` over rewriting the codecs. Use `optimod-diagnose` for read-only hardware checks. The contrast utility is intentionally write-capable and requires its explicit command-line guard.
