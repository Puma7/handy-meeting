# Local text model residency — 2026-10-30

Scope: HandyMeeting 1.0.7 development sources, based on `f2ccb8a8a268323da5ea0ad8ed1eb69102694da6`. This change does not publish or install a new application version.

## Behavior

Previously, keeping a text client alive did not prevent the provider's idle policy from unloading its model between live updates. Known local Ollama and LM Studio connections now reserve their selected model explicitly. Generic local OpenAI-compatible servers retain their existing transport behavior; residency is not claimed for them.

The live worker loads before waiting for transcript updates. The reservation survives capture ending, recognition catching up, final windows and final overview persistence. Failed or partial finalization retains the reservation for retry. A successful post-meeting analysis can release an inactive live reservation only when its source hash covers the current completed transcript. An older resumed snapshot cannot release newer work. Completion in another meeting cannot release this meeting's unfinished work.

Live notes and post-meeting analysis share per-model ownership. Releasing an anchor does not unload a model while an in-flight client or another job still uses it. Explicit cancellation also releases its anchor; recording and recognition remain independent.

Cleanup is also deferred across separate loopback spellings of the same provider/model. Their actual routes, credentials and instance IDs remain separate. Conservatively grouping distinct local servers may keep one warm longer; it does not redirect requests between servers.

## Provider boundaries

- Ollama: native `/api/generate` preloads with `keep_alive: -1`; `/api/chat` uses the same indefinite lifetime, messages, JSON schema and output budget. Final release unloads an initially absent model. An initially present indefinitely loaded model is left loaded. An initially present finite lifetime is restored with at least five minutes' idle grace, rather than forcibly unloaded.
- LM Studio: native REST v1 explicitly loads the chosen catalog key. Inference addresses the returned instance ID. Only a newly returned instance absent from the pre-load inventory is eligible for app cleanup. A reused existing instance is rejected with a clear error because the app cannot establish its pin or ownership. Cleanup must acknowledge the exact owned instance ID. Native REST v1 support is required; no silent provider or API fallback occurs.
- Native loading is bounded to 300 seconds. Once submitted, its acknowledgement is read even after cancellation so a created instance is tracked and cleaned up. User cancellation is still recorded immediately; a replacement live worker waits for its predecessor to finish. Inference requests remain cancellable.
- Management uses the existing loopback-only, no-proxy, no-redirect transport. It receives provider/model metadata and credentials only, never session objects or audio-file access. Management errors do not expose server response bodies. Cloud consent and source validation remain unchanged.

These are application-owned reservations, not exclusive locks against another program or a user changing the model server. Server shutdown, manual unload, app termination, a management timeout without an acknowledgement or concurrent external instance changes can invalidate residency or leave an unconfirmed server-side load. No foreign instance is guessed and unloaded to repair such uncertainty. A failed cleanup prevents unconfirmed reuse and requires checking the local service.

## Evidence and remaining gates

The new component tests use synthetic loopback HTTP services. They cover virtual idle expiry, load-once behavior, shared users, foreign instance preservation, native/OpenAI request contracts, truncated responses, failure/retry, cancellation during loading, acknowledgement validation, credential/route boundaries, and retained live windows through overview retry. Coordinator tests cover separate meetings, independent live/post owners, stale completion and retained in-flight clients. The virtual clock is not a real-duration, hardware or performance measurement.

The idle regression fails with the previous unreserved client; the cancellation regression fails when a submitted native load is abandoned before its acknowledgement. Both pass with the corrected implementation.

The prescribed `verify.ps1` passed: 47 PCM, 35 notes, 28 template and 8 agent checks, plus monitor checks. The unchanged frontend passed 225 tests, TypeScript and production build. All-target Rust tests passed: 365 passed, zero failed, two existing dedicated entry points normally ignored (native credential-provider roundtrip and crash-writer child). Workspace/all-target Clippy passed with `-D warnings`, and formatting and diff checks passed. The final local run used four test threads and two build jobs; no test deadline changed.

A concurrent repeat of the legacy checks timed out during an agent exchange; the sequential repeat passed with the same check and unchanged deadline. The initial full Rust build exhausted local disk space; only this task's fresh build cache was removed, then the checks were rebuilt with debug symbols and incremental compilation disabled. Prior logs remain retained. No product test, warning policy or CI threshold was weakened.

Actual Ollama and the configured LM Studio server were unreachable during the read-only probes. **The real local reference test with positive and nonbinding statements remains a merge gate**, including verification of the installed provider's load/instance behavior and residency across an actual idle period. No real model performance, GPU memory, capture hardware or combined long-duration result is claimed. Existing capture interruption, signing/notarization and platform limitations remain open. The full historical `TEST_REPORT.md` is unchanged.

Official protocol sources: [Ollama keep-alive](https://docs.ollama.com/faq), [native chat](https://docs.ollama.com/api/chat), [LM Studio idle TTL and auto-evict](https://lmstudio.ai/docs/developer/core/ttl-and-auto-evict), [load](https://lmstudio.ai/docs/developer/rest/load), [list](https://lmstudio.ai/docs/developer/rest/list), [unload](https://lmstudio.ai/docs/developer/rest/unload).
