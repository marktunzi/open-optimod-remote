# OPTIMOD-FM 8700HD evidence

## Primary package

- Installer `Setup8700_1.0.2.161_PC_Remote.exe` ("Optimod 8700 PC Remote") SHA-256: `46a170fe1f545ced9f058e99c17a0d12307d4eab01b1d1e2858917738ab039ea`
- `8700PC.exe` SHA-256: `6ff1f203b45345553459e70f043ae681514c98ba130058d677e70170a72ee5b5`
- Firmware `update.zip` → `bin/_optimod.exe` (1.0.2.161) SHA-256: `7237681ead460387d9332b30bbe1acfc22681a69ba363a2c64cc87c64f91bfc6`

The package contains PC Remote 1.0.2.161, firmware 1.0.2.161, worksheet and operating manual 1.0.2, and 712 factory preset files. None of them is stored in this repository.

## Identity

This package is for the original OPTIMOD-FM **8700HD**, not the 8700i:

- the PC Remote and firmware banner is `8700HD V `;
- the About dialog and registry keys say `Optimod 8700HD PC Remote`;
- the telnet greeting is `Welcome to the Orban Optimod-FM 8700HD.`

It shares the `8700.51` document family with the 8700i, but it is a separate model and adapter.

## Extracted facts

- Login obfuscation and framing constants are identical to the 5700i. The firmware implements `AP`, `AS`, `LP` and `RP` with `ON AIR:`.
- 460 parameter registrations; four use register-pushed arguments that were resolved from their call sites.
- The factory presets are 8600 documents (`8600.40` and `8600.10`). Of 236 preset fields, 225 match exactly, 2 are explained by a transform and 8 are excluded. This is weaker evidence than for the 5500, because the 8600 and 8700HD firmware may differ.
- Meter store: channels 0–104, banks 1 and 2. The channel layout follows the 5700i scheme (AGC 3/10 and 4/11, FM bands 5–9/14–18, HD bands 39–43/44–48, HD limiting 19/20, HD output 51/52).

Evidence levels and the runtime checks are the same as for the [5500](../../optimod-5500-control/references/evidence.md#evidence-levels).
