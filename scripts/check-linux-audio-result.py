#!/usr/bin/env python3
"""Fail closed on unexpected virtual-capture failures; retain known startup gaps."""
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import sys
import wave

TIMING = "Portable audio track ranges use approximate receipt timing, not exact device or word timestamps."
START = ": the source started late; leading silence is estimated and can contain missing audio."
ALLOWED = {TIMING, "system" + START, "microphone" + START}


def require(condition, message):
    """Fail closed on missing capture evidence or mismatched saved audio."""
    if not condition:
        raise ValueError(message)


def verify(root):
    """Verify complete distinct PCM tracks while retaining explicit startup-gap limits."""
    result = json.loads((root / "result.json").read_text(encoding="utf-8"))
    checkpoints = sorted((root / "checkpoints").glob("checkpoint-*.json"))
    require(checkpoints, "No final checkpoint")
    session = json.loads(checkpoints[-1].read_text(encoding="utf-8"))
    require(re.fullmatch(r"[a-f0-9]{32}", session["id"]), "Invalid owned session")
    require(session["status"] == "recorded", "Capture did not stop normally")
    require(type(result["captureIncomplete"]) is bool
            and result["captureIncomplete"] == session["captureIncomplete"], "Capture flags disagree")
    warnings = result["warnings"]
    require(warnings == session["warnings"] and set(warnings) <= ALLOWED,
            "Unexpected capture warning; inspect retained raw evidence")
    startup_gaps = any(w.endswith(START) for w in warnings)
    require(result["captureIncomplete"] == startup_gaps, "Unexplained incomplete capture")
    require(result["trackDifferenceWithinLimit"] is True
            and abs(result["systemMs"] - result["microphoneMs"]) <= 500,
            "Track duration difference exceeds preregistered 500 ms")
    parts = session["parts"]
    require(len(parts) == 2 and sorted(p["source"] for p in parts) == ["microphone", "system"],
            "Expected one closed part per source in the eight-second probe")
    tracks = {}
    for source, frequency, tolerance in (("system", 440, 40), ("microphone", 1000, 60)):
        part = next(p for p in parts if p["source"] == source)
        require(re.fullmatch(source + r"-\d{12}\.wav", part["file"]), "Invalid audio filename")
        require(part["audioClosed"] is True and not part.get("error"), "Audio part not finalized")
        with wave.open(str(root / "sessions" / session["id"] / "audio" / part["file"]), "rb") as audio:
            require((audio.getnchannels(), audio.getframerate(), audio.getsampwidth(), audio.getcomptype())
                    == (1, 16000, 2, "NONE"), "Unexpected PCM format")
            count = audio.getnframes()
            require(8 * 16000 <= count <= 10 * 16000, "Eight-second capture was truncated or overran")
            data = audio.readframes(count)
        require(len(data) == count * 2 and part["confirmedSampleCount"] == count
                and part["sampleCount"] == count
                and part["confirmedSha256"] == hashlib.sha256(data).hexdigest(),
                "Confirmed audio changed or is incomplete")
        require(result[source + "Ms"] == count * 1000 // 16000, "Result duration differs from saved PCM")
        samples = struct.unpack(f"<{count}h", data)[count // 4: count * 3 // 4]
        rms = math.sqrt(sum(s * s for s in samples) / len(samples))
        crossings = sum((a < 0) != (b < 0) for a, b in zip(samples, samples[1:]))
        hz = crossings * 16000 / 2 / len(samples)
        require(rms > 1000 and abs(hz - frequency) < tolerance, "Silent or mixed-up virtual source")
        tracks[source] = {"durationMs": result[source + "Ms"], "rms": round(rms), "hz": round(hz)}
    return {"success": True, "verdict": "passed-with-startup-gaps" if startup_gaps else "passed",
            "completeCapture": not result["captureIncomplete"], "startupGaps": startup_gaps,
            "virtualDevicesOnly": True, "physicalHardware": False,
            "allowedTrackDifferenceMs": 500, "tracks": tracks,
            "limits": "Private PulseAudio sources only. Startup gaps remain incomplete audio, not a gap-free device proof."}


if __name__ == "__main__":
    receipt = verify(Path(sys.argv[1]))
    with Path(sys.argv[2]).open("x", encoding="utf-8") as output:
        json.dump(receipt, output, indent=2)
    print(json.dumps(receipt, indent=2))
