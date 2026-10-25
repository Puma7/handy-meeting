#!/usr/bin/env bash
# GitHub-hosted runner check only. Uses a unique synthetic service/entry inside
# an isolated native store; never use this to change a user's desktop keychain.
set -euo pipefail
if [[ "${GITHUB_ACTIONS:-}" != true || $# != 2 ]]; then
  echo 'Run only on GitHub Actions with APP_BINARY and NEW_OUTPUT_DIRECTORY.' >&2
  exit 2
fi
app=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$1")
output=$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$2")
if [[ ! -x "$app" || -e "$output" ]]; then
  echo 'Expected an executable and a new output directory; existing evidence is preserved.' >&2
  exit 2
fi
run_native_check() {
  # A native permission dialog must fail CI within a deadline rather than
  # leave an invisible prompt indefinitely. Python terminates only this
  # explicitly launched headless check process on timeout.
  python3 -c 'import subprocess,sys; subprocess.run([sys.argv[1],"--credential-checks",sys.argv[2]],check=True,timeout=60)' "$1" "$2"
}
export -f run_native_check
case "$(uname -s)" in
  Linux)
    command -v dbus-run-session >/dev/null
    command -v gnome-keyring-daemon >/dev/null
    state=$(mktemp -d "${RUNNER_TEMP:?}/handymeeting-credential-state.XXXXXXXX")
    export XDG_DATA_HOME="$state/data" XDG_CONFIG_HOME="$state/config" XDG_CACHE_HOME="$state/cache" XDG_RUNTIME_DIR="$state/runtime"
    mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_RUNTIME_DIR"
    chmod 700 "$XDG_RUNTIME_DIR"
    export HM_CREDENTIAL_APP="$app" HM_CREDENTIAL_OUTPUT="$output"
    dbus-run-session -- bash -c '
      set -euo pipefail
      printf %s handymeeting-isolated-ci-test | gnome-keyring-daemon --unlock --components=secrets
      run_native_check "$HM_CREDENTIAL_APP" "$HM_CREDENTIAL_OUTPUT"
    '
    ;;
  Darwin)
    state=$(mktemp -d "${RUNNER_TEMP:?}/handymeeting-credential-state.XXXXXXXX")
    keychain="$state/isolated-test.keychain-db"
    original=$(security default-keychain -d user | python3 -c 'import json,sys; print(json.loads(sys.stdin.read().strip()))')
    restore_default() { security default-keychain -d user -s "$original"; }
    # This fixed password protects a disposable synthetic CI store. It is not
    # a provider API key; application test keys are created inside Rust only.
    security create-keychain -p handymeeting-isolated-ci-test "$keychain"
    trap restore_default EXIT
    security default-keychain -d user -s "$keychain"
    security unlock-keychain -p handymeeting-isolated-ci-test "$keychain"
    run_native_check "$app" "$output"
    ;;
  *) echo 'Use the Windows native check directly; this wrapper is Unix CI-only.' >&2; exit 2 ;;
esac
python3 - "$output/credential-checks.json" <<'PY'
import json,sys
with open(sys.argv[1],encoding='utf-8') as f:
    result=json.load(f)
assert result['success'] is True and result['actualNativeStore'] is True
assert result['simulated'] is False and result['checks'] == 5
assert result['usesUserEntries'] is False and result['cloudRequests'] is False
assert result['provider'] in ('linux-secret-service','macos-keychain')
PY
