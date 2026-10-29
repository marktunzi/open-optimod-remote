#!/bin/sh
# Makes an encrypted backup of the update signing key: an AES-256 disk image
# with a passphrase you type in two macOS dialogs. The passphrase goes to
# hdiutil through a pipe only; neither it nor the key is printed or stored
# elsewhere. Keep the passphrase in your password manager: without it the
# backup cannot be opened.
#
# Usage: backup-update-key.sh [FOLDER]
#   FOLDER defaults to iCloud Drive/Open Optimod Remote (a different place
#   than the key itself, ~/.config/open-optimod-remote).
# Restore: open the .dmg, enter the passphrase, copy sparkle-ed25519.key to
#   ~/.config/open-optimod-remote/ and chmod 600 it.
set -eu

ed_key=${SPARKLE_ED_KEY_FILE:-"$HOME/.config/open-optimod-remote/sparkle-ed25519.key"}
dest=${1:-"$HOME/Library/Mobile Documents/com~apple~CloudDocs/Open Optimod Remote"}
[ -f "$ed_key" ] || { echo "Update signing key not found: $ed_key" >&2; exit 1; }

ask() {
  /usr/bin/osascript -e "text returned of (display dialog \"$1\" with title \"Update key backup\" default answer \"\" with hidden answer buttons {\"Cancel\", \"OK\"} default button \"OK\")"
}
pass=$(ask "Passphrase for the encrypted backup of the Open Optimod Remote update key (at least 12 characters):") || exit 1
again=$(ask "Type the passphrase again:") || exit 1
[ "$pass" = "$again" ] || { echo "The passphrases differ; nothing was written." >&2; exit 1; }
[ ${#pass} -ge 12 ] || { echo "The passphrase is shorter than 12 characters; nothing was written." >&2; exit 1; }

mkdir -p "$dest"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
mkdir "$work/Open Optimod Remote update key"
cp "$ed_key" "$work/Open Optimod Remote update key/sparkle-ed25519.key"
[ ! -f "${ed_key%.key}.pub" ] || cp "${ed_key%.key}.pub" "$work/Open Optimod Remote update key/"
image="$dest/open-optimod-remote-update-key-$(date +%Y%m%d).dmg"
[ ! -e "$image" ] || { echo "Backup already exists: $image" >&2; exit 1; }
printf '%s\0' "$pass" | /usr/bin/hdiutil create -quiet -encryption AES-256 -stdinpass \
  -volname "Open Optimod Remote update key" -fs APFS \
  -srcfolder "$work/Open Optimod Remote update key" "$image"
/usr/bin/hdiutil isencrypted "$image" | grep -q 'encrypted: YES' || { echo "The image is not encrypted: $image" >&2; rm -f "$image"; exit 1; }
echo "Encrypted backup written: $image"
