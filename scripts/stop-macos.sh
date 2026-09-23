#!/bin/sh
set -eu
cache_dir="$HOME/Library/Caches/OpenOptimodRemote"
pid_file="$cache_dir/service.pid"
[ -f "$pid_file" ] || exit 0
pid=$(cat "$pid_file")
case "$pid" in ''|*[!0-9]*) exit 1;; esac
process=$(/bin/ps -p "$pid" -o command= || true)
case "$process" in
 */Open\ Optimod\ Remote.app/Contents/MacOS/orban-web|*/Open\ Optimod\ Remote.app/Contents/Resources/backend/orban-web)
  /usr/bin/curl --silent --max-time 6 -X POST -H 'Origin: http://127.0.0.1:5701' http://127.0.0.1:5701/api/disconnect > /dev/null || true
  kill -TERM "$pid"
  rm -f "$pid_file"
 ;;
 *) printf 'No matching Open Optimod process. Nothing stopped.\n';;
esac
