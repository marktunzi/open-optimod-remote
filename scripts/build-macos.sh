#!/bin/sh
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_dir"

case "$(uname -s)" in
 Darwin) ;;
 *) echo 'This bundle script requires macOS.' >&2; exit 1 ;;
esac

architecture=$(uname -m)
case "$architecture" in
 arm64|x86_64) ;;
 *) echo "Unsupported macOS architecture: $architecture" >&2; exit 1 ;;
esac

cargo_bin=$(command -v cargo || true)
if [ -z "$cargo_bin" ]; then cargo_bin="$HOME/.cargo/bin/cargo"; fi
swiftc_bin=$(command -v swiftc || true)
if [ -z "$swiftc_bin" ]; then echo 'swiftc is required to build the macOS app.' >&2; exit 1; fi

# Sparkle 2 (signed automatic updates), pinned by version and SHA-256 and kept
# in target/ so later builds need no network.
sparkle_version=2.10.0
sparkle_sha256=c2bf58aa8387266ac179357b1415d6f2635f044da8be41042af32425dae6da0c
sparkle_dir="$project_dir/target/sparkle-$sparkle_version"
if [ ! -d "$sparkle_dir/Sparkle.framework" ]; then
  mkdir -p "$project_dir/target"
  sparkle_archive="$project_dir/target/Sparkle-$sparkle_version.tar.xz"
  [ -f "$sparkle_archive" ] || /usr/bin/curl -fsSL -o "$sparkle_archive" \
    "https://github.com/sparkle-project/Sparkle/releases/download/$sparkle_version/Sparkle-$sparkle_version.tar.xz"
  echo "$sparkle_sha256  $sparkle_archive" | /usr/bin/shasum -a 256 -c - >/dev/null || {
    echo "Sparkle $sparkle_version does not match its pinned SHA-256." >&2; rm -f "$sparkle_archive"; exit 1; }
  rm -rf "$sparkle_dir.partial"; mkdir -p "$sparkle_dir.partial"
  /usr/bin/tar -xJf "$sparkle_archive" -C "$sparkle_dir.partial"
  mv "$sparkle_dir.partial" "$sparkle_dir"
fi

# Version and build number; a release sets them (Sparkle compares the build number).
app_version=${OPEN_OPTIMOD_VERSION:-0.3.0}
app_build=${OPEN_OPTIMOD_BUILD:-4}
# Tests only: another bundle identifier keeps a test build's settings apart from the installed app's.
bundle_id=${OPEN_OPTIMOD_BUNDLE_ID:-org.openoptimod.remote}
case "$app_build" in ''|*[!0-9]*) echo "OPEN_OPTIMOD_BUILD must be a whole number." >&2; exit 1 ;; esac
# The public half of the update signing key (the private half never enters the repository).
update_public_key=jUANhYRpr64JLX9XR5Z4QmsfRn2c6oPOXCdSpqweGD0=
update_feed=https://raw.githubusercontent.com/marktunzi/open-optimod-remote/appcast/appcast.xml

npm ci --prefix packages/ui
npm run build --prefix packages/ui
"$cargo_bin" build --release -p orban-web

# OPEN_OPTIMOD_APP_DIR builds elsewhere, so a running dist/ app is not replaced.
app_dir=${OPEN_OPTIMOD_APP_DIR:-"$project_dir/dist/Open Optimod Remote.app"}
stage_root=$(mktemp -d "$project_dir/target/open-optimod-macos.XXXXXX")
stage_app="$stage_root/Open Optimod Remote.app"
trap 'rm -rf "$stage_root"' EXIT HUP INT TERM

mkdir -p \
  "$stage_app/Contents/MacOS" \
  "$stage_app/Contents/Frameworks" \
  "$stage_app/Contents/Resources/backend" \
  "$stage_app/Contents/Resources/ui"

iconset="$stage_root/OpenOptimod.iconset"
mkdir -p "$iconset"
for spec in \
  '16 icon_16x16.png' \
  '32 icon_16x16@2x.png' \
  '32 icon_32x32.png' \
  '64 icon_32x32@2x.png' \
  '128 icon_128x128.png' \
  '256 icon_128x128@2x.png' \
  '256 icon_256x256.png' \
  '512 icon_256x256@2x.png' \
  '512 icon_512x512.png' \
  '1024 icon_512x512@2x.png'
do
  size=${spec%% *}
  filename=${spec#* }
  /usr/bin/sips -s format png -z "$size" "$size" native/macos/AppIcon.png --out "$iconset/$filename" >/dev/null
done
/usr/bin/iconutil -c icns "$iconset" -o "$stage_app/Contents/Resources/OpenOptimod.icns"

"$swiftc_bin" \
  -parse-as-library \
  -swift-version 5 \
  -O \
  -target "$architecture-apple-macos13.0" \
  -framework AppKit \
  -framework WebKit \
  -framework Network \
  -framework UniformTypeIdentifiers \
  -F "$sparkle_dir" \
  -framework Sparkle \
  -Xlinker -rpath -Xlinker @executable_path/../Frameworks \
  native/macos/ErrorLog.swift \
  native/macos/DiagnosticsWindow.swift \
  native/macos/ToolsWindows.swift \
  native/macos/LauncherCore.swift \
  native/macos/SystemSettingsSpec.swift \
  native/macos/SystemSettingsAPI.swift \
  native/macos/ProfileSystemSettings.swift \
  native/macos/SystemSettingsWindow.swift \
  native/macos/ConnectionsAPI.swift \
  native/macos/NetworkDiscovery.swift \
  native/macos/ConnectionsWindow.swift \
  native/macos/PresetsAPI.swift \
  native/macos/PresetsWindow.swift \
  native/macos/DeviceTabs.swift \
  native/macos/DeviceWindows.swift \
  native/macos/UpdateCore.swift \
  native/macos/Updater.swift \
  native/macos/OpenOptimodApp.swift \
  -o "$stage_app/Contents/MacOS/Open Optimod Remote"

# The XPC services are for sandboxed apps only; this app is not sandboxed.
/usr/bin/ditto "$sparkle_dir/Sparkle.framework" "$stage_app/Contents/Frameworks/Sparkle.framework"
rm -rf "$stage_app/Contents/Frameworks/Sparkle.framework/Versions/B/XPCServices" \
  "$stage_app/Contents/Frameworks/Sparkle.framework/XPCServices"

cp target/release/orban-web "$stage_app/Contents/Resources/backend/orban-web"
cp -R packages/ui/dist/. "$stage_app/Contents/Resources/ui/"
cp scripts/stop-macos.sh "$stage_app/Contents/Resources/Stop Open Optimod.command"
chmod +x \
  "$stage_app/Contents/MacOS/Open Optimod Remote" \
  "$stage_app/Contents/Resources/backend/orban-web" \
  "$stage_app/Contents/Resources/Stop Open Optimod.command"

cat > "$stage_app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleDisplayName</key><string>Open Optimod Remote</string>
<key>CFBundleExecutable</key><string>Open Optimod Remote</string>
<key>CFBundleIdentifier</key><string>$bundle_id</string>
<key>CFBundleIconFile</key><string>OpenOptimod</string>
<key>CFBundleName</key><string>Open Optimod Remote</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$app_version</string>
<key>CFBundleVersion</key><string>$app_build</string>
<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSAppTransportSecurity</key><dict><key>NSAllowsLocalNetworking</key><true/></dict>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Open Optimod Remote contributors</string>
<key>SUFeedURL</key><string>$update_feed</string>
<key>SUPublicEDKey</key><string>$update_public_key</string>
<key>SURequireSignedFeed</key><true/>
<key>SUVerifyUpdateBeforeExtraction</key><true/>
<key>SUEnableAutomaticChecks</key><true/>
<key>SUScheduledCheckInterval</key><integer>86400</integer>
<key>NSLocalNetworkUsageDescription</key><string>Open Optimod Remote connects to your Optimod on the local network to control settings and display live meters.</string>
</dict></plist>
PLIST

# Ad-hoc signing by default. Set CODESIGN_IDENTITY to a "Developer ID Application: …"
# identity for a distributable build; that also enables the hardened runtime and a
# secure timestamp, both required for notarization (see scripts/release-macos.sh).
sign_identity=${CODESIGN_IDENTITY:--}
if [ "$sign_identity" = - ]; then
  sign_flags=''
else
  sign_flags='--options runtime --timestamp'
fi
# Sparkle inside-out, as its documentation asks for an app that is not sandboxed.
sparkle_signed="$stage_app/Contents/Frameworks/Sparkle.framework"
for part in "$sparkle_signed/Versions/B/Autoupdate" "$sparkle_signed/Versions/B/Updater.app" "$sparkle_signed"; do
  # shellcheck disable=SC2086
  /usr/bin/codesign --force $sign_flags --sign "$sign_identity" "$part"
done
# shellcheck disable=SC2086
/usr/bin/codesign --force $sign_flags --sign "$sign_identity" --identifier "$bundle_id.backend" "$stage_app/Contents/Resources/backend/orban-web"
# shellcheck disable=SC2086
/usr/bin/codesign --force $sign_flags --sign "$sign_identity" --identifier "$bundle_id" "$stage_app"
/usr/bin/codesign --verify --deep --strict "$stage_app"
/usr/bin/plutil -lint "$stage_app/Contents/Info.plist"

mkdir -p "$(dirname "$app_dir")"
if [ -e "$app_dir" ]; then mv "$app_dir" "$stage_root/previous.app"; fi
mv "$stage_app" "$app_dir"

printf 'Built one-click macOS app: %s\n' "$app_dir"
