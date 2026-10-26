# A small Handy companion boundary

Status: consumer implementation and upstream proposal, not a negotiated Handy API.
Related: [issue1](https://github.com/Puma7/handy-meeting/issues/1).

## What works today

HandyMeeting starts Handy's existing `--list-models --json` CLI with an argument
array, closed input and bounded output/time. Each new recording revalidates a
fresh model advertisement. The installed application supplies the complete model
catalog; no copied model list or automatic model substitution is maintained here.
Weights remain in the existing cache, read-only. The companion uses its own
version-pinned official transcribe.cpp runtime and safe Rust bindings. Two tracks
use separate model/worker instances; Handy's running inference is not shared.

`HandyMeeting --companion-capabilities` describes this consumer contract as
schemaVersion1. The optional argument `--contract-version 1` explicitly selects
it. Unknown versions fail; installed paths, model names/IDs, source objects,
credentials and meeting text are excluded. Catalog counts, observation time,
fingerprint and optional package version are diagnostic observations, not proof
of native model compatibility. A model still has to load successfully.

The protocol owner is HandyMeeting and `upstreamNegotiated` is false. This
description is safe for local agent capability discovery, but cannot negotiate
new capabilities that Handy itself does not expose. Existing
[compatibility tests](HANDY_COMPATIBILITY.md) reject missing flags, changed
envelopes, unsafe paths, changed identities and stale observations.

## The remaining unstable part

Handy's CLI does not currently return an authoritative absolute weight path or a
versioned companion envelope. The bridge therefore resolves documented observed
cache layouts and records a content hash and resolved revision before capture.
Those layouts are implementation dependencies. Unsupported layouts or a changed
model/runtime produce an actionable error; they never authorize downloading,
modifying weights or silently switching recognizers. A Handy update that adds a
new native model family can require a tested companion runtime update.

## Proposed upstream addition

A minimal future command could return one versioned envelope containing:

- protocol major/minor and application identity;
- advertised capabilities, including streaming, sample rate and native family;
- selected installed model ID, immutable revision and authoritative read-only
  resolved weight path, only when the caller explicitly requests that model;
- runtime family/ABI identity sufficient to decide whether the companion's
  separately bundled runtime is compatible;
- stable typed failures for not installed, downloading, unsupported and busy.

The command name, schema and permissions must be agreed with Handy's maintainer.
No proposed command is executed by the current application. Major versions must
be explicitly supported; additive minor metadata can be ignored. A failed query
cannot be relabeled as a current catalog. A short-lived response never replaces
the immutable model identity of an existing meeting.

An optional inference service would be a separate, larger proposal. It would
need explicit local authorization, request/session ownership, resource limits,
stream scheduling, cancellation and worker-failure isolation. It is not needed
for the initial boundary, and sharing loaded model memory is not promised.

## What remains for issue1

Consumer behavior, versioned description, update/error semantics and the proposal
are implemented. Upstream acceptance and a supported resolved-path/ABI endpoint
remain external work. No maintainer has been contacted automatically, and the
issue must not be presented as a jointly approved Handy protocol until that
agreement exists.
