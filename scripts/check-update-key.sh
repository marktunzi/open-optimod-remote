#!/bin/sh
# Checks the update signing key (EdDSA) without showing it: the file lives
# outside the repository, only its owner can read it, it is the key whose
# public half the app trusts (SUPublicEDKey in build-macos.sh), and Sparkle's
# sign_update can sign with it and verify the result. Prints no key material.
#
# Usage: check-update-key.sh   (SPARKLE_ED_KEY_FILE overrides the location,
#                               as in release-macos.sh)
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
ed_key=${SPARKLE_ED_KEY_FILE:-"$HOME/.config/open-optimod-remote/sparkle-ed25519.key"}
fail() { echo "FAIL: $1" >&2; exit 1; }

[ -f "$ed_key" ] || fail "update signing key not found: $ed_key"
key_dir=$(CDPATH='' cd -- "$(dirname -- "$ed_key")" && pwd)
case "$key_dir/" in "$project_dir"/*) fail "the key is inside the repository: $key_dir" ;; esac
[ "$(stat -f %Lp "$ed_key")" = 600 ] || fail "the key must be readable by its owner only (chmod 600 $ed_key)"
[ "$(stat -f %Lp "$key_dir")" = 700 ] || fail "the key's folder must be private (chmod 700 $key_dir)"
git -C "$project_dir" check-ignore -q --no-index "$project_dir/sparkle-ed25519.key" \
  || fail ".gitignore does not exclude Sparkle private keys"

trusted=$(sed -n 's/^update_public_key=//p' "$project_dir/scripts/build-macos.sh")
derived=$(/usr/bin/base64 -D < "$ed_key" | /usr/bin/xcrun swift -e 'import CryptoKit; import Foundation
let seed = FileHandle.standardInput.readDataToEndOfFile()
print((try? Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey.rawRepresentation.base64EncodedString()) ?? "")' 2>/dev/null)
[ -n "$trusted" ] && [ "$trusted" = "$derived" ] || fail "the key does not match SUPublicEDKey in build-macos.sh"

sign_update="$project_dir/target/sparkle-2.10.0/bin/sign_update"
if [ -x "$sign_update" ]; then
  work=$(mktemp -d)
  trap 'rm -rf "$work"' EXIT HUP INT TERM
  printf 'open optimod remote update key check\n' > "$work/probe"
  signature=$("$sign_update" --ed-key-file "$ed_key" -p "$work/probe")
  "$sign_update" --verify --ed-key-file "$ed_key" "$work/probe" "$signature" >/dev/null 2>&1 \
    || fail "sign_update could not verify its own signature"
  echo "OK: key outside the repository, private, trusted by the app; sign_update signs and verifies"
else
  echo "OK: key outside the repository, private, trusted by the app (sign_update not built yet: run build-macos.sh)"
fi
