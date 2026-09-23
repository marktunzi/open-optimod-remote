# OPTIMOD 5500 evidence

## Primary package

- Source: `https://www.orban-europe.com/downloads/5500/Software/setup5500_1.2.8.24_pc_remote.zip`
- Downloaded package SHA-256: `2240d156809424408200d094026f4f00017c6051292b2be44c8de6a54d8aead3`
- `5500PC.exe` SHA-256: `c8f138357e9b8758af16e1c756e88258cecb41b762b9daf7112b66649565a6a1`
- Worksheet SHA-256: `2b9faeea0a466739b0c01c74fb139c9fff1d55259642937570f47588926c7097`
- Operating manual SHA-256: `a486e40641f73e51d7bc6438eda1b7a49f669233f22be4aa3727ad9ec3b4b8ea`

The package contains PC Remote 1.2.8.24, worksheet 1.2.7, the 1.2 operating manual and 294 factory preset files. No Orban executable, manual, worksheet or preset is stored in this repository.

## Extracted facts

- PC Remote login prefix: `5500 V `.
- Default PC Remote endpoint: TCP 6201.
- Terminal/status endpoint documented by the manual: TCP 23.
- Factory preset document family: `OptimodVersion=<8300.10>`.
- Factory presets use typed `C:<NAME>Type:value;D:index;` processing records.
- The official preset corpus contains 108 distinct processing field names. A given observed `D` index never mapped to conflicting values for the same field in the extracted corpus.
- The worksheet contains 322 non-empty functional entries after its column header and covers processing plus system setup.

## Evidence levels

| Evidence | Permitted use |
|---|---|
| Official worksheet label/range/unit | Page organization and candidate display metadata |
| Official factory-preset field/type/value/index | Read-only parsing and candidate value mappings |
| Live AP/AS fixture from exact hardware | Exact wire names and current values for that firmware |
| Reversible hardware write plus acknowledgement/readback | Write enablement for that one field/profile |
| Captured meter records with known injected levels | Meter channel and curve enablement |

The worksheet and presets do not prove the complete accepted value range, PC Remote write acknowledgement, or meter format. Keep those features fail-closed until the higher evidence level exists.
