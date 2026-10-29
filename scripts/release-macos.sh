#!/bin/sh
# Builds a distributable macOS release: Developer ID signature, hardened runtime,
# Apple notarization and stapled tickets on both the app and the disk image, plus
# the signed automatic update: a zip of the stapled app and a Sparkle appcast,
# both signed with the update key (EdDSA). Nothing is published.
#
# Usage: release-macos.sh VERSION BUILD [RELEASE_NOTES.html]
#
# Required environment:
#   CODESIGN_IDENTITY  "Developer ID Application: Name (TEAMID)"
#   ASC_KEY_ID         App Store Connect API key ID (Developer role or higher)
#   ASC_ISSUER_ID      App Store Connect issuer ID
# Optional:
#   ASC_KEY_PATH       defaults to ~/.appstoreconnect/private_keys/AuthKey_$ASC_KEY_ID.p8
#   SPARKLE_ED_KEY_FILE  the update signing key (a file, never in the repository);
#                      defaults to ~/.config/open-optimod-remote/sparkle-ed25519.key
#
# Output in dist/release/VERSION/ (the dev build in dist/ is left alone).
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

: "${CODESIGN_IDENTITY:?Set CODESIGN_IDENTITY to a Developer ID Application identity}"
: "${ASC_KEY_ID:?Set ASC_KEY_ID to an App Store Connect API key ID}"
: "${ASC_ISSUER_ID:?Set ASC_ISSUER_ID to the App Store Connect issuer ID}"
asc_key_path=${ASC_KEY_PATH:-"$HOME/.appstoreconnect/private_keys/AuthKey_$ASC_KEY_ID.p8"}
[ -f "$asc_key_path" ] || { echo "App Store Connect key not found: $asc_key_path" >&2; exit 1; }
version=${1:?Usage: release-macos.sh VERSION BUILD [RELEASE_NOTES.html]}
build=${2:?Usage: release-macos.sh VERSION BUILD [RELEASE_NOTES.html]}
notes=${3:-}
[ -z "$notes" ] || [ -f "$notes" ] || { echo "Release notes not found: $notes" >&2; exit 1; }
ed_key=${SPARKLE_ED_KEY_FILE:-"$HOME/.config/open-optimod-remote/sparkle-ed25519.key"}
[ -f "$ed_key" ] || { echo "Update signing key not found: $ed_key" >&2; exit 1; }

case "$CODESIGN_IDENTITY" in
 'Developer ID Application:'*) ;;
 *) echo 'CODESIGN_IDENTITY must be a "Developer ID Application: …" identity.' >&2; exit 1 ;;
esac

out="$project_dir/dist/release/$version"
app="$out/Open Optimod Remote.app"
mkdir -p "$out"
CODESIGN_IDENTITY="$CODESIGN_IDENTITY" OPEN_OPTIMOD_VERSION="$version" OPEN_OPTIMOD_BUILD="$build" \
  OPEN_OPTIMOD_APP_DIR="$app" ./scripts/build-macos.sh

architecture=$(uname -m)
dmg="$out/OpenOptimodRemote-$version-$architecture.dmg"
work=$(mktemp -d "$project_dir/target/open-optimod-release.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM
sparkle_bin="$project_dir/target/sparkle-2.10.0/bin"

# The update key must be the one whose public half the app trusts (SUPublicEDKey).
# The key file holds the 32-byte Ed25519 seed; CryptoKit derives its public half.
trusted=$(/usr/libexec/PlistBuddy -c 'Print :SUPublicEDKey' "$app/Contents/Info.plist")
derived=$(/usr/bin/base64 -D < "$ed_key" | /usr/bin/xcrun swift -e 'import CryptoKit; import Foundation
let seed = FileHandle.standardInput.readDataToEndOfFile()
print((try? Curve25519.Signing.PrivateKey(rawRepresentation: seed).publicKey.rawRepresentation.base64EncodedString()) ?? "")' 2>/dev/null)
[ "$trusted" = "$derived" ] || { echo "The update key does not match SUPublicEDKey in the app." >&2; exit 1; }

# Every Mach-O must carry the Developer ID signature, hardened runtime and timestamp.
for binary in "$app/Contents/MacOS/Open Optimod Remote" "$app/Contents/Resources/backend/orban-web"; do
  details=$(/usr/bin/codesign -dvv "$binary" 2>&1)
  printf '%s\n' "$details" | grep -q "^Authority=$CODESIGN_IDENTITY\$" || { echo "Not Developer ID signed: $binary" >&2; exit 1; }
  printf '%s\n' "$details" | grep -q 'flags=.*runtime' || { echo "Hardened runtime missing: $binary" >&2; exit 1; }
  printf '%s\n' "$details" | grep -q '^Timestamp=' || { echo "Secure timestamp missing: $binary" >&2; exit 1; }
done
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"

notarize() {
  result="$work/notary-$(basename "$1").json"
  xcrun notarytool submit "$1" \
    --key "$asc_key_path" --key-id "$ASC_KEY_ID" --issuer "$ASC_ISSUER_ID" \
    --wait --output-format json > "$result"
  if ! grep -Eq '"status" *: *"Accepted"' "$result"; then
    cat "$result" >&2
    submission=$(sed -n 's/.*"id" *: *"\([^"]*\)".*/\1/p' "$result")
    if [ -n "$submission" ]; then
      xcrun notarytool log "$submission" \
        --key "$asc_key_path" --key-id "$ASC_KEY_ID" --issuer "$ASC_ISSUER_ID" >&2 || true
    fi
    echo "Notarization failed for $1" >&2
    exit 1
  fi
}

# 1. Notarize the app itself and staple its ticket, so the app stays valid outside the DMG.
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$app" "$work/app.zip"
notarize "$work/app.zip"
xcrun stapler staple "$app"

# 2. Disk image with the stapled app and an Applications shortcut; it needs its own ticket.
mkdir "$work/dmg"
/usr/bin/ditto "$app" "$work/dmg/Open Optimod Remote.app"
ln -s /Applications "$work/dmg/Applications"
rm -f "$dmg"
/usr/bin/hdiutil create -volname 'Open Optimod Remote' -srcfolder "$work/dmg" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null
/usr/bin/codesign --force --timestamp --sign "$CODESIGN_IDENTITY" "$dmg"
notarize "$dmg"
xcrun stapler staple "$dmg"

# 3. The signed update: a zip of the stapled app, and an appcast that names it
#    at this version's GitHub release. generate_appcast signs both the archive
#    and the feed itself (the app requires a signed feed).
mkdir "$work/feed"
update_zip="OpenOptimodRemote-$version.zip"
/usr/bin/ditto -c -k --sequesterRsrc --keepParent "$app" "$work/feed/$update_zip"
if [ -n "$notes" ]; then cp "$notes" "$work/feed/OpenOptimodRemote-$version.html"; fi
"$sparkle_bin/generate_appcast" --ed-key-file "$ed_key" \
  --download-url-prefix "https://github.com/marktunzi/open-optimod-remote/releases/download/v$version/" \
  --embed-release-notes -o "$work/feed/appcast.xml" "$work/feed"
signature=$(sed -n 's/.*sparkle:edSignature="\([^"]*\)".*/\1/p' "$work/feed/appcast.xml")
"$sparkle_bin/sign_update" --ed-key-file "$ed_key" --verify "$work/feed/$update_zip" "$signature"
grep -q "<sparkle:version>$build</sparkle:version>" "$work/feed/appcast.xml" || { echo "The appcast does not list build $build." >&2; exit 1; }
cp "$work/feed/$update_zip" "$work/feed/appcast.xml" "$out/"

# 4. Check the result the way Gatekeeper will on another Mac.
xcrun stapler validate "$app"
xcrun stapler validate "$dmg"
/usr/sbin/spctl --assess --type execute --verbose=2 "$app"
/usr/sbin/spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"

printf 'Notarized release in %s\n' "$out"
(cd "$out" && /usr/bin/shasum -a 256 "$(basename "$dmg")" "$update_zip" | tee SHA256SUMS)
