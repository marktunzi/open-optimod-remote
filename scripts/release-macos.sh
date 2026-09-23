#!/bin/sh
# Builds a distributable macOS release: Developer ID signature, hardened runtime,
# Apple notarization and stapled tickets on both the app and the disk image.
#
# Required environment:
#   CODESIGN_IDENTITY  "Developer ID Application: Name (TEAMID)"
#   ASC_KEY_ID         App Store Connect API key ID (Developer role or higher)
#   ASC_ISSUER_ID      App Store Connect issuer ID
# Optional:
#   ASC_KEY_PATH       defaults to ~/.appstoreconnect/private_keys/AuthKey_$ASC_KEY_ID.p8
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

: "${CODESIGN_IDENTITY:?Set CODESIGN_IDENTITY to a Developer ID Application identity}"
: "${ASC_KEY_ID:?Set ASC_KEY_ID to an App Store Connect API key ID}"
: "${ASC_ISSUER_ID:?Set ASC_ISSUER_ID to the App Store Connect issuer ID}"
asc_key_path=${ASC_KEY_PATH:-"$HOME/.appstoreconnect/private_keys/AuthKey_$ASC_KEY_ID.p8"}
[ -f "$asc_key_path" ] || { echo "App Store Connect key not found: $asc_key_path" >&2; exit 1; }

case "$CODESIGN_IDENTITY" in
 'Developer ID Application:'*) ;;
 *) echo 'CODESIGN_IDENTITY must be a "Developer ID Application: …" identity.' >&2; exit 1 ;;
esac

CODESIGN_IDENTITY="$CODESIGN_IDENTITY" ./scripts/build-macos.sh

app="$project_dir/dist/Open Optimod Remote.app"
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$app/Contents/Info.plist")
architecture=$(uname -m)
dmg="$project_dir/dist/OpenOptimodRemote-$version-$architecture.dmg"
work=$(mktemp -d "$project_dir/target/open-optimod-release.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

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

# 3. Check the result the way Gatekeeper will on another Mac.
xcrun stapler validate "$app"
xcrun stapler validate "$dmg"
/usr/sbin/spctl --assess --type execute --verbose=2 "$app"
/usr/sbin/spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"

printf 'Notarized release: %s\n' "$dmg"
/usr/bin/shasum -a 256 "$dmg"
