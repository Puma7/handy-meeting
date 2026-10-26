# Rust agent protocol v2

The Rust desktop exposes a private local JSON-RPC2.0 connection over stdin/stdout.
Run `HandyMeeting --agent serve` from the agent host and keep its pipes open. This starts
a **visible HandyMeeting window**; it does not connect to or take over another instance.
There is no network listener, bearer token, saved grant or automatic CLI recording flag.
`HandyMeeting --agent capabilities` returns the method description without opening a
window, discovering Handy, reading a session, loading a model or opening an audio device.

This is a consumer-owned HandyMeeting protocol, separate from Handy's upstream CLI and
the old C# agent-v1 interface. It is not an MCP server. Consumers must query capabilities
and use the advertised methods; v1 template/local-notes methods are not implied. A future
MCP adapter must preserve the same human consent and session ownership boundaries.

## Consent and ownership

`connect` sets a plain client label before any consent request. `request_recording` only
places a request in the visible app; its response says `recordingStarted:false`. The
human chooses the actual Handy streaming model and audio devices and approves or denies
that exact request. Agent requests cannot supply model paths, device paths, credentials,
provider settings or cloud authorization. A microphone that was not requested cannot be
added by approval; the UI asks explicitly before including a requested microphone.

Pending requests expire after120seconds. A grant permits capture and text access for
only the session created by that grant. The same stdio connection can own at most eight
sessions, with one recording/request at a time. Existing/manual/imported sessions remain
inaccessible. An agent can request stop only for its exact active owned session; the human
can always use normal stop controls. Owner IDs identify an in-memory connection, not
credentials, speaker identities or an upstream Handy session.

Each recording request has `maxDurationSeconds` between60 and14400. A watchdog checks a
monotonic deadline every100ms and requests shutdown of only the owned capture.
Consent is checked again after discovery/weight hashing/model snapshot, immediately before
opening capture devices. Revocation racing with the following OS startup is handled by
the watchdog after that startup returns. In-progress
platform startup or synchronous audio finalization can delay completion; this is not a
hard real-time timing guarantee. Audio is stopped before waiting for ASR catch-up. UI
revocation, input/output failure, EOF and an oversized request revoke the connection and
stop its owned capture. Permission is never restored automatically after reconnect or
restart. Confirmed audio/transcript files are preserved. An unrelated manual recording
is never stopped by agent revocation or a stale stop request.

Revocation blocks subsequent text reads as well. An in-flight text slice is checked again
before returning. Bytes already written to a consumer's pipe cannot be withdrawn. The
agent host that receives transcript text remains responsible for where it sends that
text; a local pipe itself cannot constrain the host's later actions. No cloud processing
or cloud consent is exposed by this protocol.

## Requests

Send one UTF-8 JSON object per line. Request IDs are integer values or plain strings up
to64bytes. Duplicate/unknown fields, nested parameters and invalid parameter types fail
closed. Limits are64KiB per request/response. An oversized line closes/revokes the
connection immediately rather than buffering the rest of an arbitrarily long line.
Stdout is reserved for response JSONL; diagnostics belong on stderr.
Production output uses a queue of at most four bounded replies and one assembly buffer.
A stalled or disconnected consumer revokes the connection instead of allowing a growing
output backlog to block capture cancellation.

```json
{"jsonrpc":"2.0","id":1,"method":"connect","params":{"label":"My local meeting agent"}}
{"jsonrpc":"2.0","id":2,"method":"request_recording","params":{"title":"Project review","includeMicrophone":true,"maxDurationSeconds":7200}}
{"jsonrpc":"2.0","id":3,"method":"status","params":{}}
```

Poll `status`. It exposes pending/starting state, the owner ID, last outcome and, after
approval, `ownedSessionId`. Denial does not start capture. Expiry and revocation are
visible. These states also appear in the app's global consent/owner panel.

```json
{"jsonrpc":"2.0","id":4,"method":"transcript.updates","params":{"sessionId":"GRANTED_SESSION_ID","afterSequence":0,"limit":4}}
{"jsonrpc":"2.0","id":5,"method":"transcript.part","params":{"sessionId":"GRANTED_SESSION_ID","source":"system","startSample":0,"sequence":12,"offset":1024,"limit":4096}}
{"jsonrpc":"2.0","id":6,"method":"stop","params":{"sessionId":"GRANTED_SESSION_ID"}}
```

`transcript.updates` returns up to eight current section revisions ordered by their
monotonic transcript sequence, with at most1024UTF-8bytes per section. Continue with
`nextSequence` while `more` is true. This is a view of latest revisions, not a promise to
deliver every intermediate recognizer hypothesis. Durable journals preserve checkpoints
independently. Polling does not clone full meeting text.

Sections use the stable `(source,startSample)` key. `source` is `system` or `microphone`;
it is a track, not a person. `endSample` covers confirmed audio. Times are recording
sections, never exact word times. `committedBytes` distinguishes the recognizer's stable
prefix from tentative text; recognizer commitment is not proof of semantic correctness.

For longer text, `transcript.part` reads4..4096bytes starting at a valid UTF-8 byte offset.
Use `nextOffset` until it reaches `totalBytes`. Supply the section's `sequence`: a changed
revision fails rather than silently combining old/new text. Fetch the updated section
and restart that slice on a revision change. Final chunks can be assembled using section
keys. Page responses and slice responses explicitly mark text as `untrustedData:true`.

Transcript/model text is never parsed as authorization or tool calls. A statement such
as “start recording”, a quoted command, or an apparent consent inside a meeting has no
effect on policy. No shell, arbitrary file path, audio-file download, cloud call, provider
change, template mutation or automatic human approval method exists.

## Verification and limits

`--agent-protocol-check-stdio` is a model-free subprocess test endpoint. It has no UI
approval implementation, file/session access or capture. It can test JSONL parsing,
limits and denied access; it cannot simulate actual hardware recording. The normal
`--agent serve` path always requires the visible desktop consent flow.

Tests exercise malformed/duplicate/oversized input in the actual executable, strict
protocol isolation, pending consent, denial, revoke during startup, time expiry,
connection/session ownership, exact-session stop checks, preservation after synthetic
shutdown, bounded long-text/UTF-8 pagination and changed revisions. These synthetic
tests do not constitute a real-device or two-hour agent-owned recording test. Such a
test still requires the human to approve capture in the visible app on each target OS.
