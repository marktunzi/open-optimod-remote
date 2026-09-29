# OPTIMOD 5500 evidence

## Primary package

- Source: `https://www.orban-europe.com/downloads/5500/Software/setup5500_1.2.8.24_pc_remote.zip`
- Installer `Setup5500_1.2.8.24_PC_Remote.exe` SHA-256: `8d43f31689ca1278a9682e8715cd1cdb22b431cc574a3cc65753f994d8b8ee59`
- `5500PC.exe` SHA-256: `c8f138357e9b8758af16e1c756e88258cecb41b762b9daf7112b66649565a6a1`
- Firmware `update.zip` → `bin/_optimod.exe` (1.2.8.24) SHA-256: `a3c7d67a000bdf16d1317e9bfa86354175f12c6283b35de066d7c9b868bf5e6d`
- Worksheet SHA-256: `2b9faeea0a466739b0c01c74fb139c9fff1d55259642937570f47588926c7097`
- Operating manual SHA-256: `a486e40641f73e51d7bc6438eda1b7a49f669233f22be4aa3727ad9ec3b4b8ea`

The package contains PC Remote 1.2.8.24, firmware 1.2.8.24, worksheet 1.2.7, the 1.2 operating manual and 294 factory preset files. None of them is stored in this repository.

## Extracted facts

- PC Remote login banner `5500 V ` plus firmware version `1.2.8.24`, both present in the firmware.
- Terminal banner `Orban Optimod 5500`. The firmware implements `AP`, `AS`, `LP` and `RP` and answers recall with `ON AIR:`.
- Login obfuscation and framing constants are identical to the 5700i.
- Factory preset document family: `OptimodVersion=<8300.10>` (292 files) and `<5500.10>` (2 files).
- 280 parameter registrations. Their conversion routines were emulated for every index.
- Of 108 preset fields: 89 match exactly, 10 are explained by one consistent transform, and 9 are excluded.
- Meter store: channels 0–78, banks 1 and 2 with variable length.

## Evidence levels

| Evidence | Permitted use |
|---|---|
| Worksheet label/range/unit | Page organization and display metadata |
| PC Remote conversion routine | Candidate index-to-value table |
| Factory preset value at an index | Confirms or corrects the wire format for that field |
| Live AP/AS field equal to the profile at its current index | Allows a single write for that session |
| Full AP/AS readback after the write | Confirms that one write; a mismatch is shown and never retried |
| Reversible hardware test on this firmware | Needed to mark the adapter `Hardware` |

The current adapter is `Static`: writes are enabled for the exact banner, and every write relies on the last two runtime checks.
