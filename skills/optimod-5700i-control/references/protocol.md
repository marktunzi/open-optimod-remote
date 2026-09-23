# 5700i Ethernet protocol

These facts are verified for OPTIMOD 5700i firmware `3.0.1.20`, document family `5700.51`. The implementation must use bounded reads, explicit timeouts, and exact response boundaries.

## Transports

| Port | Purpose | Character |
|---|---|---|
| TCP 6201 | PC Remote session, binary framing, meters and typed changes | Long-lived, single owner |
| TCP 23 | Status/terminal snapshots and preset commands | One bounded exchange per operation |

The sockets are independent. A terminal snapshot can run while the PC Remote owner continues polling meters, provided only the owner writes to the PC Remote socket.

## PC Remote login

Send `connect`, LF, the transformed uppercase access code, LF, NUL. Current validation accepts 1–64 ASCII alphanumeric characters.

Transform each uppercase code byte:

```text
state = 123456
state = (state * 419 + 6173) mod 29282
mask = low_byte(state)
encoded = original when original == mask, otherwise original XOR mask
```

Reject a result containing LF or NUL. The transform is legacy obfuscation, not encryption. Never log input or transformed bytes.

Successful login returns four bounded lines:

1. `connect ok`
2. decimal access level
3. model and firmware, verified as `5700i V 3.0.1.20`
4. decimal session seed

Known negative markers include `password failed`, `no password set`, `in use`, and `busy`. Access level `0` is the only write level verified by this project; other levels remain read-only.

## Binary frames

A frame is a five-byte header followed by 1–65535 payload bytes. For the big-endian payload-length bytes `hi, lo`:

```text
h0 = hi XOR 0x1a
h1 = (-hi modulo 256) XOR 0xc2
h2 = lo XOR 0x67
h3 = (-lo modulo 256) XOR 0xa8
h4 = -(h0 + h1 + h2 + h3) modulo 256
```

Validate both complement relationships, the zero-sum header checksum, and nonzero length. Decode incrementally because TCP may split or combine frames. Once framing is corrupt, reconnect rather than scanning for a possible next header.

The payload begins with two right-aligned eight-character decimal fields:

```text
request field 1 = opcode - 204
field 2 = protocol type 6
```

Known operations:

| Opcode | Meaning | Expected response |
|---|---|---|
| 218 | Disconnect | Connection closes |
| 219 | Poll/heartbeat | 220 and/or subscribed meter events |
| 222 / 223 | Enable/disable session meters | Subscription behavior |
| 227 | Typed parameter change | Ordered boundary; terminal readback on failure or explicit verification |
| 239 | Fetch on-air archive | File response 229 |
| 240 | Fetch setup archive | File response 233 |
| 250 | Ordered response boundary | 251 |

Do not assume any other operation is harmless. In particular, `226` and bulk preset operations may have save/flush side effects.

## Message bodies

Opcode `228` meter data contains one bank byte, a decimal count terminated by LF, then exactly that many bytes. The verified count is 112 and the bank must be 1 or 2.

File responses `229`–`235` begin with two decimal timestamp lines, a filename/path metadata line, a decimal byte-count line, then exactly that many binary bytes. Firmware 3.0.1.20 can return a Windows-style absolute path with a drive colon and backslashes; accept that line when it is nonempty and contains no ASCII control characters. Treat it only as untrusted metadata. Never resolve it, join it to a directory, open it, or reuse it as the name of a local file. Generate and sanitize local filenames inside the app.

## Archives and documents

Observed archives are AES-256 CBC with ciphertext stealing. The first 16 bytes are the IV. Form the 32-byte key by repeating the ASCII access-code bytes. The format has no authentication, so successful decryption is not proof of integrity.

A valid document:

- is at most 65535 bytes and valid UTF-8;
- starts with `OptimodVersion=<5700.`;
- ends with `End Preset<end>\r\n`;
- has a `Preset Name=<...> size=...` line;
- contains unique fields formatted as `C:<NAME>TYPE:VALUE;D:INDEX;`.

Supported value types are `Int`, `Cent`, `String:<choice>`, and `UserString:<text>`. Preserve the integer device index and the typed value separately.

## Terminal exchanges

Require a banner beginning `Orban Optimod 5700i `. Commands terminate with CRLF. Read until the complete `End Preset<end>` marker, cap the response at 131070 bytes, normalize line endings, send `disconnect`, and close. Do not use an idle timeout as the document boundary.

| Command | Meaning |
|---|---|
| `AP [CODE]??` | Active processing document |
| `AS [CODE]??` | System/setup document |
| `LP [CODE]` followed by `AP [CODE]??` | Preset list plus active document |
| `RP NAME[CODE]` followed by `AP [CODE]??` | Recall and verify active document |

Keep the code uppercase in commands and in a zeroizing buffer. A terminal timeout during a write-capable exchange means the action may already have happened.

## Local preset files

Exporting the current processing state does not require an undocumented preset-save opcode: read a fresh authenticated AP document and write that validated text under an app-generated `.orb57user` filename.

Applying a local 5700 processing document uses only the verified field-write route:

1. Parse and structurally validate the complete document and firmware family.
2. Require the same active host, application session and expected current preset.
3. Resolve every imported field through the active firmware profile and validate its delimiters and typed value.
4. Apply `HD COUPLING` before dependent HD fields. When the imported target is `FM->HD`, omit only HD counterpart fields that follow FM. Continue applying the five unique **HD Limiting** controls: `IBOC EQ GAIN`, `IBOC EQ FREQ`, `IBOC LIM DR`, `HD DE ESS`, and `HD COUPLING`.
5. Send each field once using opcode `227` and cross an ordered boundary.
6. Read a fresh complete AP document and require every imported field to match exactly. A delayed readback may be polled briefly, but never replay a write automatically.
7. Only after full confirmation may the imported document become the comparison baseline.

This is local export/application, not named on-device Save or Save As. Named save, rename, delete, previous-preset, bulk backup and restore remain unverified. Do not guess commands or use opcode `226` for them.
