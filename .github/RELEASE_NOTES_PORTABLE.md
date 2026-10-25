## Portable preview 0.5.0-preview.1

Experimental companion app for local meeting transcription and source-linked notes. Independently developed; no official Handy endorsement or integration agreement.

### Packages

- `osx-arm64.dmg`: Apple Silicon, macOS 15 or newer.
- `osx-x64.dmg`: Intel, macOS 15 or newer.
- `linux-amd64.deb`: Ubuntu/Debian x64 with the declared ICU/graphics/PulseAudio dependencies.

Install Handy and a compatible streaming model first. Model weights are reused, not included. Optional local notes need an additional local text-model service. Portable API keys remain in memory for the session.

Mac packages include their .NET runtime, native capture helper and architecture-specific transcribe.cpp libraries. They are **ad-hoc signed, not Developer-ID signed or Apple-notarized**. macOS can block them by default. Do not disable Gatekeeper system-wide to run a prototype. Microphone and system-audio permissions are required when recording.

### Evidence and limits

The packages are built on their actual target architectures. CI verifies Mac bundle signatures and DMG integrity, and runs synthetic PCM, transcript recovery, source-validation, transport-boundary and agent checks on both Mac architectures and Linux. A separate local Linux synthetic process-abort test recovered confirmed audio prefixes byte-for-byte and both transcript checkpoints. A local native Linux CPU test transcribed a synthetic recording.

**Real microphone/system-audio capture, device loss, native GUI behavior and two-hour sessions on these portable targets still need hardware testing.** Build success and synthetic recovery do not establish reliable capture on your particular device. Use disposable test meetings first; this preview is not approved for important recordings. An unconfirmed tail can be missing after an interruption. No power-loss safety guarantee is made.

Speaker diarization and exact word timestamps are absent. Source-linked notes remain reviewable AI drafts. The portable UI has incomplete localization and feature parity. See `PORTABILITY.md`, `TEST_REPORT.md` and the tracked roadmap issues.
