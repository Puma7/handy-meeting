# 30 October 2026 – separate interface clarity follow-up

The recording source cards now share one heading/control/meter structure. In a
rendered synthetic recording the prior 7 px meter offset became 0 px at 1025 px and
800 px viewport widths. At 600 px the sources intentionally stack; no horizontal
page overflow was observed. Both light and dark views were inspected with synthetic
IPC and text only: no actual capture, transcript, model server or credentials.

Audio-level guidance and live-summary options are named disclosures. Model setup,
per-run cloud consent, capture warnings and incomplete-result notices retain their
gates. Post-meeting configuration and step editing start collapsed; settings group
optional setup and maintenance. Original warning text remains available, with an
explanation that a driver flag proves neither an audible gap nor uninterrupted
capture. CPU/RAM use whole-computer percentages, RAM exposes used/total GiB, and
GPU usage remains unmeasured. Positive transcription backlog is no longer labeled
up to date merely because it is below 30 seconds. Existing transcript order and
30-second recording/section behavior are unchanged.

All 226 frontend tests and TypeScript passed. The production frontend build and
`./verify.ps1` passed locally; final source-bound checks are recorded with the PR.
This is presentation work, not a new physical audio, text-model quality, GPU or
endurance proof. Native recording recovery, provider transport, prompts and model
selection policy are unchanged. `docs/UI_GUIDELINES.md` records the shared layout,
action hierarchy, progressive help and honest status conventions.

The analysis page exposes one start action at a time. The persistent status
bar remains the place for progress, cancellation and errors. The original full
report is retained without modification in this interface change.


## Follow-up after the first review

CodeRabbit correctly found that RAM amounts and the GPU limitation were exposed
only through pointer tooltips. They now appear in a keyboard-accessible native
computer-details disclosure. A separate review reproduced a running archive job
and its cancellation control disappearing inside collapsed maintenance settings.
The same single job observer now renders outside that disclosure, retaining its
existing callbacks and cancellation path without duplicate initial polling.
Both new synthetic regressions failed against the previous UI implementation.
These checks do not perform a real archive operation or measure GPU usage.

All 228 frontend tests, TypeScript, the production build and the required
`./verify.ps1` check passed after these fixes. A synthetic browser check opened
computer details with Enter, kept the running job outside closed settings,
and observed cancellation acknowledgement. No real archive was started.

## Visible local-model warning

Review 5710641785 identified the local-model memory warning inside closed
options. A short warning now remains visible before starting: extra memory use
can affect audio, so analysis can wait until recording has ended. The longer
explanation remains under options. The visibility regression failed before the
fix; all 229 frontend tests, TypeScript, the production build and the required
`./verify.ps1` check pass afterward. A synthetic browser check confirmed the
warning is outside the still-closed options.

The historical [macOS ARM job on ea1e080](https://github.com/Puma7/meeting-for-handy/actions/runs/41035957251/job/123996754171)
failed the synthetic reconnect-progress test's 150-ms test deadline. The actual
macOS application uses a 60-second no-progress readiness limit. Neither deadline
was changed or skipped. Fresh complete target CI remains required; this report
does not turn the historical failure into a success or establish hardware quality.
