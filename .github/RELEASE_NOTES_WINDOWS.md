## Windows prototype 0.4.1

Independent meeting companion for Handy. This is an early prototype, not an official Handy release or a production reliability guarantee.

1. Download and fully extract `HandyMeeting-0.4.1-windows-x64.zip`.
2. Install .NET Desktop Runtime 10 and Handy with a compatible streaming model. Nemotron Streaming 3.5 was used for local verification.
3. Double-click `Start HandyMeeting.cmd`. Keep the full extracted directory together.

The package includes a pinned local recognition runtime. No model weights, recordings, transcripts, settings or API keys are included. System audio and the optional microphone are saved separately. Local notes need a separate local text-model service such as LM Studio or Ollama.

The Windows verification suite passed 86 named checks plus monitor status-file checks locally and on GitHub. Local CPU streaming also transcribed a synthetic reference. The earlier two-hour synthetic test applies to the old 0.2.1 runtime and retains an open GPU private-memory trend warning; it is not a new two-hour validation of this version.

Confirmed checkpoint recovery after a process crash is not a power-loss guarantee; an unconfirmed tail can be missing. Speaker diarization and exact word timestamps are not implemented. Notes are reviewable AI drafts. See `TEST_REPORT.md` and `PORTABILITY.md` for scope and limits.

This package is Windows x64 only. macOS/Linux packages belong to the separate portable preview.
