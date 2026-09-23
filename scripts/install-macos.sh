#!/bin/sh
set -eu

project_dir=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
source_app="$project_dir/dist/Open Optimod Remote.app"
destination="/Applications/Open Optimod Remote.app"

DEVELOPER_DIR=${DEVELOPER_DIR:-/Library/Developer/CommandLineTools}
export DEVELOPER_DIR
sh "$project_dir/scripts/build-macos.sh"

stage_root=$(mktemp -d "${TMPDIR:-/tmp}/open-optimod-install.XXXXXX")
stage_app="$stage_root/Open Optimod Remote.app"
trap 'rm -rf "$stage_root"' EXIT HUP INT TERM

/usr/bin/ditto "$source_app" "$stage_app"
/usr/bin/codesign --verify --deep --strict "$stage_app"

if [ -e "$destination" ]; then
  # Stop both installed processes before replacing their signed executables.
  sh "$project_dir/scripts/stop-macos.sh" || true
  launcher="$destination/Contents/MacOS/Open Optimod Remote"
  backend="$destination/Contents/Resources/backend/orban-web"
  running_pids=$(/bin/ps ax -o pid= -o command= | /usr/bin/awk -v launcher="$launcher" -v backend="$backend" '
    {
      pid=$1
      $1=""
      sub(/^[[:space:]]+/, "")
      if ($0 == launcher || $0 == backend) print pid
    }
  ')
  if [ -n "$running_pids" ]; then
    kill -TERM $running_pids 2>/dev/null || true
    for attempt in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
      alive=false
      for process_id in $running_pids; do
        if kill -0 "$process_id" 2>/dev/null; then alive=true; fi
      done
      [ "$alive" = false ] && break
      sleep 0.1
    done
    if [ "$alive" = true ]; then
      echo 'Open Optimod Remote could not be stopped before the update.' >&2
      exit 1
    fi
  fi
  mv "$destination" "$stage_root/previous.app"
fi
if ! mv "$stage_app" "$destination"; then
  if [ -e "$stage_root/previous.app" ]; then mv "$stage_root/previous.app" "$destination"; fi
  echo 'Installation failed; the previous application was restored.' >&2
  exit 1
fi

/usr/bin/codesign --force --deep --sign - --identifier org.openoptimod.remote "$destination"
/usr/bin/codesign --verify --deep --strict "$destination"
/usr/bin/plutil -lint "$destination/Contents/Info.plist"

printf 'Installed: %s\n' "$destination"
