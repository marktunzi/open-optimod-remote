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

npm ci --prefix packages/ui
npm run build --prefix packages/ui
"$cargo_bin" build --release -p orban-web

app_dir="$project_dir/dist/Open Optimod Remote.app"
stage_root=$(mktemp -d "$project_dir/target/open-optimod-macos.XXXXXX")
stage_app="$stage_root/Open Optimod Remote.app"
trap 'rm -rf "$stage_root"' EXIT HUP INT TERM

mkdir -p \
  "$stage_app/Contents/MacOS" \
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
  native/macos/ErrorLog.swift \
  native/macos/LauncherCore.swift \
  native/macos/SystemSettingsSpec.swift \
  native/macos/SystemSettingsAPI.swift \
  native/macos/SystemSettingsWindow.swift \
  native/macos/ConnectionsAPI.swift \
  native/macos/NetworkDiscovery.swift \
  native/macos/ConnectionsWindow.swift \
  native/macos/PresetsAPI.swift \
  native/macos/PresetsWindow.swift \
  native/macos/OpenOptimodApp.swift \
  -o "$stage_app/Contents/MacOS/Open Optimod Remote"

cp target/release/orban-web "$stage_app/Contents/Resources/backend/orban-web"
cp -R packages/ui/dist/. "$stage_app/Contents/Resources/ui/"
cp scripts/stop-macos.sh "$stage_app/Contents/Resources/Stop Open Optimod.command"
chmod +x \
  "$stage_app/Contents/MacOS/Open Optimod Remote" \
  "$stage_app/Contents/Resources/backend/orban-web" \
  "$stage_app/Contents/Resources/Stop Open Optimod.command"

cat > "$stage_app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleDisplayName</key><string>Open Optimod Remote</string>
<key>CFBundleExecutable</key><string>Open Optimod Remote</string>
<key>CFBundleIdentifier</key><string>org.openoptimod.remote</string>
<key>CFBundleIconFile</key><string>OpenOptimod</string>
<key>CFBundleName</key><string>Open Optimod Remote</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.2.0</string>
<key>CFBundleVersion</key><string>3</string>
<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
<key>LSMinimumSystemVersion</key><string>13.0</string>
<key>NSAppTransportSecurity</key><dict><key>NSAllowsLocalNetworking</key><true/></dict>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>Open Optimod Remote contributors</string>
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
# shellcheck disable=SC2086
/usr/bin/codesign --force $sign_flags --sign "$sign_identity" --identifier org.openoptimod.remote.backend "$stage_app/Contents/Resources/backend/orban-web"
# shellcheck disable=SC2086
/usr/bin/codesign --force $sign_flags --sign "$sign_identity" --identifier org.openoptimod.remote "$stage_app"
/usr/bin/codesign --verify --deep --strict "$stage_app"
/usr/bin/plutil -lint "$stage_app/Contents/Info.plist"

mkdir -p "$project_dir/dist"
if [ -e "$app_dir" ]; then mv "$app_dir" "$stage_root/previous.app"; fi
mv "$stage_app" "$app_dir"

printf 'Built one-click macOS app: %s\n' "$app_dir"
