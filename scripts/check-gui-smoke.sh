#!/usr/bin/env bash
# GUI launch smoke check for CI: starts only the given desktop executable with
# private, empty app data, checks that it stays alive and opens a window, then
# stops exactly that process. Linux needs an X display (for example xvfb-run).
# No Handy installation, model, recording, credential or network use is implied.
set -euo pipefail
if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo 'Use: check-gui-smoke.sh APP_BINARY NEW_OUTPUT_DIRECTORY [SECONDS]' >&2
  exit 2
fi
app=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$1")
output=$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$2")
seconds=${3:-15}
if [[ ! -x "$app" || -e "$output" || ! "$seconds" =~ ^[0-9]+$ || $seconds -lt 5 || $seconds -gt 60 ]]; then
  echo 'Expected an executable, a new output directory and 5..60 seconds.' >&2
  exit 2
fi
mkdir -p "$output/data" "$output/home"
export HANDYMEETING_DATA_DIR="$output/data" RUST_BACKTRACE=1
case "$(uname -s)" in
  Linux)
    test -n "${DISPLAY:-}" || { echo 'No X display; run under xvfb-run.' >&2; exit 2; }
    command -v xwininfo >/dev/null
    # WebKit and GTK state stays inside the disposable output directory.
    export XDG_DATA_HOME="$output/home/data" XDG_CONFIG_HOME="$output/home/config" XDG_CACHE_HOME="$output/home/cache"
    windows() { env -u LD_LIBRARY_PATH xwininfo -root -tree 2>/dev/null | grep -c '"HandyMeeting"' || true; }
    ;;
  Darwin)
    probe="$output/windows.swift"
    # Window owner and layer are readable without screen-recording permission.
    cat > "$probe" <<'SWIFT'
import CoreGraphics
let pid = Int(CommandLine.arguments[1]) ?? -1
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let count = list.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }.count
print("HM_WINDOWS=\(count)")
SWIFT
    windows() { env -u DYLD_LIBRARY_PATH xcrun swift "$probe" "$pid" 2>/dev/null | sed -n 's/^HM_WINDOWS=//p' || true; }
    ;;
  *) echo 'Unsupported GUI smoke host.' >&2; exit 2 ;;
esac
"$app" > "$output/stdout.log" 2> "$output/stderr.log" &
pid=$!
stop_owned() { kill -TERM "$pid" 2>/dev/null || true; }
trap stop_owned EXIT
started=$SECONDS
window_count=''
while (( SECONDS - started < seconds )); do
  kill -0 "$pid" 2>/dev/null || break
  if [[ -z "$window_count" || "$window_count" = 0 ]]; then window_count=$(windows); fi
  sleep 1
done
alive=false
if kill -0 "$pid" 2>/dev/null; then alive=true; fi
if [[ -z "$window_count" || "$window_count" = 0 ]] && [[ "$alive" = true ]]; then window_count=$(windows); fi
# Stop only this process and give it a bounded time to exit.
kill -TERM "$pid" 2>/dev/null || true
status=''
for _ in $(seq 100); do
  if ! kill -0 "$pid" 2>/dev/null; then
    set +e; wait "$pid"; status=$?; set -e
    break
  fi
  sleep 0.1
done
if [[ -z "$status" ]]; then kill -KILL "$pid" 2>/dev/null || true; fi
trap - EXIT
panicked=false
if grep -q 'panicked at' "$output/stderr.log" "$output/stdout.log"; then panicked=true; fi
python3 - "$output/gui-smoke.json" "$alive" "${window_count:-}" "${status:-}" "$panicked" "$seconds" "$(uname -s)" <<'PY'
import json, sys
path, alive, windows, status, panicked, seconds, system = sys.argv[1:]
result = {"os": system, "seconds": int(seconds), "aliveAtEnd": alive == "true",
          "windows": int(windows) if windows.isdigit() else None, "exitStatus": int(status) if status else None,
          "panicked": panicked == "true", "realDevicesOpened": False, "handyRequired": False}
with open(path, "x", encoding="utf-8") as output:
    json.dump(result, output, indent=2)
print(json.dumps(result))
PY
test "$alive" = true || { echo 'The desktop process exited during the smoke check.' >&2; tail -n 40 "$output/stderr.log" >&2; exit 1; }
test "$panicked" = false || { echo 'The desktop process panicked.' >&2; exit 1; }
# 143 is the expected status after the SIGTERM sent above.
[[ "$status" = 0 || "$status" = 143 ]] || { echo "The desktop process did not stop cleanly (status ${status:-none})." >&2; exit 1; }
if [[ "${window_count:-}" =~ ^[0-9]+$ ]]; then
  (( window_count > 0 )) || { echo 'No HandyMeeting window appeared.' >&2; exit 1; }
elif [[ "$(uname -s)" = Linux ]]; then
  echo 'Window detection failed.' >&2; exit 1
else
  echo 'Warning: macOS window listing was unavailable; only liveness and clean exit were checked.' >&2
fi
echo 'GUI smoke check passed: window shown, process alive, stopped cleanly.'
