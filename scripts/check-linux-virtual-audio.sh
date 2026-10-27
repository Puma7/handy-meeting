#!/usr/bin/env bash
# Real PulseAudio check with virtual devices only. Starts its own private
# PulseAudio server (separate socket and state), so a desktop's running audio
# service, its defaults and physical devices are never touched. The actual
# `pactl` device listing and the actual `parec` capture path are exercised;
# this is not a physical microphone, PipeWire or long-session test.
set -euo pipefail
if [[ $# != 2 ]]; then
  echo 'Use: check-linux-virtual-audio.sh DEVICE_CAPTURE_EXAMPLE NEW_OUTPUT_DIRECTORY' >&2
  exit 2
fi
example=$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$1")
output=$(python3 -c 'import os,sys; print(os.path.abspath(sys.argv[1]))' "$2")
if [[ ! -x "$example" || -e "$output" ]]; then
  echo 'Expected the built device_capture example and a new output directory.' >&2
  exit 2
fi
for tool in pulseaudio pactl parec pacat python3; do command -v "$tool" >/dev/null; done
mkdir -p "$output"
private=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/handymeeting-pulse.XXXXXXXX")
mkdir -m 700 "$private/run" "$private/state"
export PULSE_RUNTIME_PATH="$private/run" PULSE_STATE_PATH="$private/state" PULSE_SERVER="unix:$private/run/native"
unset PULSE_SINK PULSE_SOURCE
server=''
players=()
cleanup() {
  # Stop only the processes this script started, then its private directory.
  for pid in "${players[@]}" $server; do kill "$pid" 2>/dev/null || true; done
  for pid in "${players[@]}" $server; do wait "$pid" 2>/dev/null || true; done
  rm -rf -- "$private"
}
trap cleanup EXIT
pulseaudio -n --daemonize=no --use-pid-file=no --exit-idle-time=-1 --log-target=stderr \
  --load="module-native-protocol-unix auth-anonymous=1 socket=$private/run/native" \
  2> "$output/pulseaudio.log" &
server=$!
for _ in $(seq 100); do pactl info >/dev/null 2>&1 && break; sleep 0.1; done
pactl info | tee "$output/pulseaudio-info.txt"
pactl --version | tee "$output/pactl-version.txt"
# System output: a null sink; its monitor is the system track.
pactl load-module module-null-sink sink_name=handymeeting_ci_output rate=16000 channels=1 sink_properties=device.description=HandyMeeting-CI-output >/dev/null
# Microphone: a separate null sink exposed as an ordinary (non-monitor) source.
pactl load-module module-null-sink sink_name=handymeeting_ci_microphone_feed rate=16000 channels=1 sink_properties=device.description=HandyMeeting-CI-microphone-feed >/dev/null
pactl load-module module-remap-source source_name=handymeeting_ci_microphone master=handymeeting_ci_microphone_feed.monitor source_properties=device.description=HandyMeeting-CI-microphone >/dev/null
pactl set-default-sink handymeeting_ci_output
pactl set-default-source handymeeting_ci_microphone
"$example" --list-devices > "$output/devices.json"
python3 - "$output/devices.json" <<'PY'
import json, sys
devices = json.load(open(sys.argv[1]))
print(json.dumps(devices, indent=2))
def find(source, device):
    return [d for d in devices if d["source"] == source and d["id"] == device]
system = find("system", "handymeeting_ci_output.monitor")
microphone = find("microphone", "handymeeting_ci_microphone")
assert len(system) == 1 and system[0]["isDefault"], "Default system output monitor is not listed"
assert len(microphone) == 1 and microphone[0]["isDefault"], "Default virtual microphone is not listed"
assert not [d for d in devices if d["source"] == "microphone" and d["id"].endswith(".monitor")], "A monitor was listed as a microphone"
print("Real pactl listing: system monitor and separate microphone found")
PY
# Two distinct tones, one per virtual device, so track separation is checkable.
python3 - "$output" <<'PY'
import math, struct, sys
for name, frequency in [("system", 440), ("microphone", 1000)]:
    with open(f"{sys.argv[1]}/{name}-tone.raw", "wb") as tone:
        tone.write(b"".join(struct.pack("<h", int(8000 * math.sin(2 * math.pi * frequency * i / 16000))) for i in range(16000 * 30)))
PY
pacat --playback --raw --format=s16le --rate=16000 --channels=1 --device=handymeeting_ci_output "$output/system-tone.raw" &
players+=($!)
pacat --playback --raw --format=s16le --rate=16000 --channels=1 --device=handymeeting_ci_microphone_feed "$output/microphone-tone.raw" &
players+=($!)
sleep 1
"$example" "$output/capture" 8
python3 - "$output/capture" <<'PY'
import glob, json, math, struct, sys, wave
root = sys.argv[1]
result = json.load(open(f"{root}/result.json"))
assert result["systemMs"] >= 4000 and result["microphoneMs"] >= 4000, result
checkpoints = sorted(glob.glob(f"{root}/checkpoints/checkpoint-*.json"))
session = json.load(open(checkpoints[-1]))
parts = session["parts"]
sources = sorted({part["source"] for part in parts})
assert sources == ["microphone", "system"], sources
def dominant(source):
    samples = []
    for part in [p for p in parts if p["source"] == source]:
        with wave.open(f"{root}/sessions/{session['id']}/audio/{part['file']}") as audio:
            assert (audio.getnchannels(), audio.getframerate(), audio.getsampwidth()) == (1, 16000, 2)
            data = audio.readframes(audio.getnframes())
            samples += struct.unpack(f"<{len(data) // 2}h", data)
    middle = samples[len(samples) // 4: len(samples) * 3 // 4]
    rms = math.sqrt(sum(s * s for s in middle) / len(middle))
    crossings = sum(1 for a, b in zip(middle, middle[1:]) if (a < 0) != (b < 0))
    return rms, crossings * 16000 / 2 / len(middle)
system_rms, system_hz = dominant("system")
microphone_rms, microphone_hz = dominant("microphone")
print(json.dumps({"systemMs": result["systemMs"], "microphoneMs": result["microphoneMs"], "systemRms": round(system_rms), "systemHz": round(system_hz), "microphoneRms": round(microphone_rms), "microphoneHz": round(microphone_hz), "virtualDevicesOnly": True}))
assert system_rms > 1000 and microphone_rms > 1000, "A virtual track recorded silence"
assert abs(system_hz - 440) < 40 and abs(microphone_hz - 1000) < 60, "Tracks are not separate"
print("Real parec capture: two separate tracks with their own virtual signals")
PY
