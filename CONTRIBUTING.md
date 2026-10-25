# Contributing

Thanks for helping test an independent Handy companion prototype. Reliability, privacy and inspectable evidence take priority over feature count.

1. Read [AGENTS.md](AGENTS.md), [PORTABILITY.md](PORTABILITY.md) and [TEST_REPORT.md](TEST_REPORT.md).
2. Explain the user-visible problem and intended behavior in an issue or pull request.
3. Keep data models, validation, transport, orchestration and UI separate. Reuse the existing recognition libraries rather than implementing another engine.
4. Run `./verify.ps1` before distributing a Windows build. Preserve package locks; treat compiler warnings as errors.
5. Add a reproducible regression check for data loss, source integrity, privacy or recovery defects. Capture changes also need appropriate device, crash and long-session evidence. Label synthetic tests honestly.
6. Text-model transport or instruction changes need real local positive and nonbinding reference cases. Fake HTTP tests do not establish compatibility with a real cloud provider.

Never attach private recordings, meeting transcripts, journals, model weights, API keys or credentials to an issue, pull request or CI artifact. Use short synthetic reproductions and redact environment details. Keep originals and previous analysis versions intact while investigating a problem.

Report the exact app version, OS, architecture, relevant local model/runtime version, scope and reproduction steps. Separate build success, synthetic recovery, real-device capture and long-session behavior. Do not describe a draft workflow as a successful CI run.

Changes are provided under the repository's MIT license. Preserve upstream notices and review licenses before adding dependencies or redistributing models. This is not an official Handy release; do not imply endorsement by Handy's maintainers.
