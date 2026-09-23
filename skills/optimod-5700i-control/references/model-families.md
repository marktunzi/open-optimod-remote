# Other OPTIMOD model families

Use this boundary when extending the 5700i implementation. These are research findings from official Orban manuals, product pages and PC Remote packages. They do not grant write support for another model.

## Family map

| Family | Models observed | Transport evidence | Reuse decision |
|---|---|---|---|
| Serial legacy | 8200 | Direct RS-232 null modem or compatible modem | New serial adapter |
| Legacy TCP/UDP | 8400 | TCP 51200 control; UDP 16540 metering | New session and meter adapter |
| PC Remote TCP | 8500, 5500/5500i, 5700 FM/HD, 5700i, 6300, 8600, 8700i, 9300, 9400 | Official binaries share login/document markers and expose 6201; some manuals also document terminal 23 | Share only fixture-proven codecs; keep model/firmware profiles separate |
| HTML5 Web UI | 5750/5750 HD, 5950, Trio | Browser control; model-dependent SNMP and other protocols | New HTTP/WebSocket adapter after authorized capture or official API docs |

## Non-negotiable boundary

- Never remove the 5700i model check and fall through to its parameter profile.
- Never reuse field indices, meter lengths, channel bindings, opcodes, archive crypto or preset commands because product names or field names look similar.
- Preset import compatibility is not live-control compatibility. Orban documents best-effort conversion on some targets, which can omit unsupported features.
- Unknown firmware stays read-only until its own fixtures and parameter/meter profiles exist.
- A cached model in the connection book is informational. The active session must identify the model and firmware again before writes.

## Per-family facts

### 8200

The official 8200PC manual describes Windows 95/98 and serial or modem connections. It says the Remote shows all meters and controls, recalls and saves presets, and archives setup. Implement serial selection, exclusive locking and bounded record parsing. A serial-to-IP bridge changes only the transport path; it does not make the device a 6201 model.

### 8400

The official 3.0.5 manual explicitly assigns TCP 51200 and UDP 16540, with UDP used for metering. Treat TCP health and meter health independently. The official `.orb` presets use an older `OptimodVersion=<00.08>` document dialect that resembles later field records but needs its own validator.

### 8500/5500/5500i/5700 and later PC Remote models

Official main executables contain `connect ok`, known password-failure text, `connected 12345678`, `OptimodVersion=<`, `End Preset<end>` and `6201`. This is evidence for a shared broad lineage only. Build a reusable `pc-remote-v2` core after packet fixtures prove each component. Keep every model's parameters, meters, presets, capabilities and supported firmware separate.

The official executables expose these concrete login prefixes: `5500i V`, `5500 V`, `5700FM V`, `5700HD V`, `5700i V`, `6300 V`, `8500 V`, `8600 V`, `8700i V`, `9300 V` and `9400 V`. The official manuals for 5500, 5500i, 5700, 6300, 8500, 8600 and 8700i document read-only `AP [PASSCODE]??`, `AS [PASSCODE]??` and `LP [PASSCODE]` queries; the official 9300/9400 executables contain the same command strings. Official factory presets establish document families `8300.10` for 5500/5500i, `5700.50` for 5700 FM/HD, `6300.50`, `8500.40`, `8600.40`, `8700.51`, `9300.30` and `9400.30`. These facts justify bounded read-only adapters; they do not justify writes or meter reuse.

The 5500/5500i are FM processors with stereo encoder functions. Diversity delay on 5500i is not a second HD processing chain. The 8500 has FM/HD capabilities. The 5700 archive has separate FM and HD Remote packages. The 9300/9400 are AM processors and require a different UI capability set.

Treat presentation as adapter data. Each model requires its own `skin_id`, product mark, material palette, display colors, signal-path label, meter grouping and visible pages. Choose the skin from the live detected model. Unknown identity uses a neutral read-only skin, never a 5700i fallback.

### HTML5 generation

Orban describes 5750 and 5750 HD as controllable from an HTML5 browser. Do not assume a public or stable API. Start read-only from official documentation or from sanitized captures made against authorized hardware. SNMP may cover monitoring and a small control subset, but is not proof of full editor parity.

## Required verification gate

Before enabling writes for one exact model and firmware:

1. record identification, login, disconnect and busy/error fixtures;
2. parse complete processing and setup documents;
3. verify preset list and active preset read-only;
4. map meter records, cadence and stale behavior;
5. verify one reversible non-audio write and exact restoration;
6. verify one processing write in a test window;
7. verify recall without losing the control or meter session;
8. run a long session/meter soak;
9. add parameter, meter and capability profiles plus documentation.

Keep the adapter read-only until the reversible hardware write succeeds. Never auto-retry an uncertain write.

## Repository references

In Open Optimod Remote, the detailed source matrix and package hashes are in `docs/research/2026-09-22-multi-model-optimod-support.md`. The staged implementation is in `docs/plans/2026-09-22-multi-model-adapter-plan.md`.
