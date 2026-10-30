# Local text model residency — 2026-10-30

Scope: HandyMeeting 1.0.7 development sources, based on `f2ccb8a8a268323da5ea0ad8ed1eb69102694da6`. This change does not publish or install a new application version.

## Behavior

Previously, keeping a text client alive did not prevent the provider's idle policy from unloading its model between live updates. Known local Ollama and LM Studio connections now reserve their selected model explicitly. Generic local OpenAI-compatible servers retain their existing transport behavior; residency is not claimed for them.

The live worker loads before waiting for transcript updates. The reservation survives capture ending, recognition catching up, final windows and final overview persistence. Failed or partial finalization retains the reservation for retry. A successful post-meeting analysis can release an inactive live reservation only when its source hash covers the current completed transcript. An older resumed snapshot cannot release newer work. Completion in another meeting cannot release this meeting's unfinished work.

Live notes and post-meeting analysis share per-model ownership. Releasing an anchor does not unload a model while an in-flight client or another job still uses it. Explicit cancellation also releases its anchor; recording and recognition remain independent.

Cleanup is also deferred across separate loopback spellings of the same provider/model. Their actual routes, credentials and instance IDs remain separate. Conservatively grouping distinct local servers may keep one warm longer; it does not redirect requests between servers.

## Provider boundaries

- Ollama: app-owned native `/api/generate` loads and `/api/chat` inference use a finite 300-second lifetime. Each finite slot has one weakly owned worker that renews it every 60 seconds, including idle intervals without inference and failed/partial work awaiting retry. Provider delays are isolated by slot; no global reservation lock is held during management. Messages, JSON schema and output budget are unchanged. Final release unloads an initially absent model. An initially present indefinitely loaded model is left loaded. An initially present finite lifetime is restored with at least five minutes' idle grace, rather than forcibly unloaded.
- LM Studio: native REST v1 explicitly loads the chosen catalog key. Inference addresses the returned instance ID. Only a newly returned instance absent from the pre-load inventory is eligible for app cleanup. A reused existing instance is rejected with a clear error because the app cannot establish its pin or ownership. Cleanup must acknowledge the exact owned instance ID. Native REST v1 support is required; no silent provider or API fallback occurs.
- Native loading is bounded to 300 seconds. Once submitted, its acknowledgement is read even after cancellation so a created instance is tracked and cleaned up. User cancellation is still recorded immediately; a replacement live worker waits for its predecessor to finish. Inference requests remain cancellable.
- Management uses the existing loopback-only, no-proxy, no-redirect transport. It receives provider/model metadata and credentials only, never session objects or audio-file access. Management errors do not expose server response bodies. Cloud consent and source validation remain unchanged.

These are application-owned reservations, not exclusive locks against another program or a user changing the model server. Server shutdown, manual unload, app termination, a management timeout without an acknowledgement or concurrent external instance changes can invalidate residency or leave an unconfirmed server-side load. No foreign instance is guessed and unloaded to repair such uncertainty. A failed cleanup prevents unconfirmed reuse and requires checking the local service.

## Evidence and remaining gates

The new component tests use synthetic loopback HTTP services. They cover virtual idle expiry, load-once behavior, shared users, foreign instance preservation, native/OpenAI request contracts, truncated responses, failure/retry, cancellation during loading, acknowledgement validation, credential/route boundaries, and retained live windows through overview retry. Coordinator tests cover separate meetings, independent live/post owners, stale completion and retained in-flight clients. The virtual clock is not a real-duration, hardware or performance measurement.

The idle regression fails with the previous unreserved client; the cancellation regression fails when a submitted native load is abandoned before its acknowledgement. Both pass with the corrected implementation.

For published candidate 02, the prescribed `verify.ps1` passed: 47 PCM, 35 notes, 28 template and 8 agent checks, plus monitor checks. The unchanged frontend passed 225 tests, TypeScript and production build. All-target Rust tests passed: 365 passed, zero failed, two existing dedicated entry points normally ignored (native credential-provider roundtrip and crash-writer child). Workspace/all-target Clippy passed with `-D warnings`, and formatting and diff checks passed. The final local run used four test threads and two build jobs; no test deadline changed.

A concurrent repeat of the legacy checks timed out during an agent exchange; the sequential repeat passed with the same check and unchanged deadline. The initial full Rust build exhausted local disk space; only this task's fresh build cache was removed, then the checks were rebuilt with debug symbols and incremental compilation disabled. Prior logs remain retained. No product test, warning policy or CI threshold was weakened.

Actual Ollama and the configured LM Studio server were unreachable during the read-only probes. **The real local reference test with positive and nonbinding statements remains a merge gate**, including verification of the installed provider's load/instance behavior and residency across an actual idle period. No real model performance, GPU memory, capture hardware or combined long-duration result is claimed. Existing capture interruption, signing/notarization and platform limitations remain open. The full historical `TEST_REPORT.md` is unchanged.

Official protocol sources: [Ollama keep-alive](https://docs.ollama.com/faq), [native chat](https://docs.ollama.com/api/chat), [LM Studio idle TTL and auto-evict](https://lmstudio.ai/docs/developer/core/ttl-and-auto-evict), [load](https://lmstudio.ai/docs/developer/rest/load), [list](https://lmstudio.ai/docs/developer/rest/list), [unload](https://lmstudio.ai/docs/developer/rest/unload).


## Crash and idle-renewal follow-up

An app crash formerly abandoned an app-owned `keep_alive: -1` load. On restart,
its far-future expiry could be mistaken for an unrelated indefinite model and
conservatively retained forever. App-owned loads and inference now set 300 seconds
and renew while an actual owner remains. This avoids creating that indefinite
post-crash state without guessing ownership of any existing foreign pin. A
preexisting indefinite model remains indefinite. A finite borrowed model retains
its prior longer finite grace. Each slot serializes renewal with final release,
so a late refresh cannot reload a released slot. Renewal failure is checked before
inference; a successful retry must receive a fresh provider acknowledgement.

A new test exits only its own synthetic child process without destructors. It
genuinely failed before the correction with two indefinite request lifetimes;
the corrected load and inference have bounded lifetimes, and a subsequent
synthetic app can own and release a fresh load after simulated provider expiry.
Two additional synthetic HTTP tests wait through the production 60-second timer:
an owner renews without inference, and failed renewal blocks text until retry.
All 16 provider protocol tests passed in 60.09 seconds. These are not real-model
residency, memory, performance, GPU, device or long-meeting measurements.

Actual Ollama and configured LM Studio remained unreachable in probe 05. The real
positive/nonbinding local reference is still required; the PR remains a draft.
Unexpected app suspension longer than the finite lease, server shutdown/manual
unload, unacknowledged management and external server changes remain boundaries.
Existing foreign indefinite models are deliberately not unloaded to hide them.

The follow-up also passed 368 Rust workspace/all-target tests, with the same two
existing dedicated entry points normally ignored. Strict workspace/all-target
Clippy passed with warnings as errors. Frontend source bytes are unchanged from
the prior 225-test proof; no new frontend run is claimed. The final required
project check and publication fingerprint are recorded with the exact candidate.


## Candidate 04: provider retry and conservative LM Studio admission

A transient preload failure no longer terminates the live worker. Cancellation
still releases its owner and stops it; other failures stay visible while the
existing update loop can prepare again through its normal retry/backoff. A
synthetic 503 provider refusal made the former worker end with failed; the same
actual worker now stays waiting with its owner retained and ends only after an
explicit stop. This does not show that an unavailable real server becomes ready.

An idle Ollama route can recover from an unconfirmed finite release. A new
reservation discards only the idle cleanup-failure latch; prepare must re-read
/api/ps and obtain a fresh load acknowledgement before inference. LM Studio's
unconfirmed exact-instance unload remains blocked. The synthetic Ollama retry
failed with the former permanent latch and now demonstrates the new provider
state query; foreign indefinite/finite policies remain preserved.

Before native LM Studio loading, the selected catalog entry must have a readable
empty loaded_instances list. A pre-existing selected-key instance causes a clear
failure before a load, unload or inference request. It is not borrowed, claimed,
forcibly evicted or duplicated, and no residency guarantee is made for it. The
user must resolve that selected model's use in LM Studio before a dedicated app
reservation is possible. Other catalog keys and foreign instances remain intact.
The synthetic already-loaded-key regression failed before this preflight check.
This policy deliberately avoids depending on unverified REST duplicate-load
semantics; actual provider compatibility remains a draft gate.

A submitted native LM Studio load with no validated new instance acknowledgement
is now an uncertain outcome, retained on that route. Automatic retry never submits
a second load and never guesses an instance to unload. A synthetic unacknowledged
load previously produced two load submissions; it now produces only one. The
existing bounded 300 s acknowledgement wait, cancellation handling and exact unload
identity verification are unchanged. An HTTP error is conservatively uncertain,
not proof that the provider created nothing.

### Manual recovery of an uncertain LM Studio load

1. Stop initiating further app analyses on that route. Inspect the provider's
   actual loading status; a client timeout does not prove server-side cancellation.
2. Wait until LM Studio confirms that pending loading finished or was aborted.
   Check its instance list and its own operation records. Do not unload an instance
   merely because it appeared near the failed request or has the same model key.
3. Remove an instance only when those provider records establish that it belongs
   to this app's failed operation and no parallel user needs it. If ownership
   cannot be established, leave it untouched; recovery is not proven safe.
4. Retry app-owned loading only after the selected key has no loaded instances
   and the provider confirms no pending load. Restart the app to reset that
   uncertain route, then prepare and validate a fresh acknowledgement.

This is explicit manual recovery, not an automatic ownership reconstruction or a
crash-cleanup guarantee. Native provider versions, real idle residency, foreign
instance preservation, positive/nonbinding content and durable final cleanup
still need the real local reference test. The P1 remains open and the PR remains
a draft while both configured services are unreachable. No service, model or
private meeting was started/used by these protocol regressions.

The corrected candidate passed 371 workspace/all-target Rust tests, with the two
existing special entry points normally ignored. All 18 provider protocol cases
passed, including the actual 60-second renewal timers and finite-TTL own-child
termination test. The exact native preload regression passed. Frontend bytes
remain identical to the earlier 225-test/TypeScript/build proof; no new frontend
run or real-provider result is claimed. Full lint/format/verify outcomes are
recorded with the immutable candidate proof.
