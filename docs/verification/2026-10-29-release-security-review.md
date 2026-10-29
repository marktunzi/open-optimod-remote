# 0.3.0 release security review

> **Evidence status:** 29 October 2026, `main` after PR #19 (`1570d4c`) plus the fixes below. A manual review of the code that 0.3.0 adds; the automated review of `af0156c` had stopped with HTTP 429 and is replaced by this one. Tests named here pass on the release commit.

## Scope

Everything a processor or a chosen file can put into a file name, a ZIP entry or an upload header; the local service's request guard; credentials and the error log; the update chain (feed, archive, keys) and the release script.

## Findings

| # | Finding | Result |
|---|---|---|
| 1 | **Backup ZIP names could collide.** `af0156c` flattens `/`, `\`, `.` and `..` in entry names, but two processor names could then end on one file (`a/b.orb` and `a_b.orb`, `.` and `..`), and so could names that differ only in case or Unicode form, which the Mac's file system treats as one name. Unzipping would overwrite one preset with another. | **Fixed:** `PresetBackup.uniqueEntryNames` gives a later colliding name ` 2`, ` 3`, … before its extension, comparing names case-insensitively and in decomposed form. Tests in `PresetsAPITests`. |
| 2 | **Extraction stays in the folder.** Every entry is `<folder>/<name>` with no separator, `.` or `..` left in either part, and no absolute path. | PASS (`af0156c` plus finding 1). |
| 3 | **Restore file names were not checked for control characters.** The name of a chosen file is a line of the upload's header (`FILETIME low`, `high`, `name`, `size`); a name with a line break would have added header lines. | **Fixed:** names with control characters are refused before anything is sent (test in `preset_files_round_trip`). The FILETIME of a chosen file's date now saturates instead of overflowing. |
| 4 | Names the app sends otherwise (Save, Save As, Rename, Delete, Recall, 8400 Restore) | PASS: `validate_store_name` / `validate_name` refuse control characters, `[ ]`, path characters and the `modif ` prefix; Delete only names a user preset from the processor's own list; 8400 state-file names are refused. |
| 5 | Names the processor sends in file messages | PASS: frames are limited to 65 535 bytes, names with control characters are refused, and a name is never used as a local path except through the ZIP writer (findings 1–2) or a save panel the user confirms. |
| 6 | Local service (`127.0.0.1` only) | PASS for browsers: the `Host` must be the service's own address (DNS rebinding is refused), a cross-site `Sec-Fetch-Site` or a foreign `Origin` is refused, and every change needs the service's own `Origin`; responses carry `no-store` and a strict Content-Security-Policy. **Unchanged by design since 0.1.0:** another program of the same macOS user can call the service, as it can read that user's files. |
| 7 | Credentials and error log | PASS: the application folder is `0700` and the credential file `0600`; access codes never go to the Keychain or into the repository; the only strings that carry a code are the terminal commands themselves (zeroized after use), never an error text; the native error log also hides local addresses. |
| 8 | Updates | PASS: the feed URL is HTTPS (`raw.githubusercontent.com`), `SURequireSignedFeed` and `SUVerifyUpdateBeforeExtraction` are set, every archive and the feed are EdDSA-signed, Sparkle 2.10.0 is pinned by SHA-256. The private update key lives only in `~/.config/open-optimod-remote` (`0600`), never in git (`scripts/check-update-key.sh`). `release-macos.sh` refuses a key whose public half is not the app's `SUPublicEDKey` and verifies the archive's signature. |
| 9 | Signing | PASS: Developer ID with hardened runtime and a secure timestamp on both executables and Sparkle's parts; no entitlements that weaken the runtime. |

No open finding blocks the release.
