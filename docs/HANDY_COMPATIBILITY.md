# Handy integration contract for the Rust migration

The installed Handy application is the source of truth for model IDs, catalog entries, custom models, downloaded state, languages and advertised streaming capability. HandyMeeting does not maintain a copied model catalog and does not download or delete model weights. It is an independently distributed companion, not an official Handy component.

## Verified upstream references

Inspected on 25 October 2026:

- Handy commit [`73c095857bc4eadff097f64767c40ad129cc372d`](https://github.com/cjpais/Handy/tree/73c095857bc4eadff097f64767c40ad129cc372d).
- [CLI arguments](https://github.com/cjpais/Handy/blob/73c095857bc4eadff097f64767c40ad129cc372d/src-tauri/src/cli.rs): `--list-models --json`; this CLI currently does not provide a general `--version` argument.
- [Headless list implementation](https://github.com/cjpais/Handy/blob/73c095857bc4eadff097f64767c40ad129cc372d/src-tauri/src/lib.rs): emits the complete `ModelInfo` array, including catalog, installed and custom entries.
- [Model metadata and path resolution](https://github.com/cjpais/Handy/blob/73c095857bc4eadff097f64767c40ad129cc372d/src-tauri/src/managers/model.rs).
- [Portable cache redirection](https://github.com/cjpais/Handy/blob/73c095857bc4eadff097f64767c40ad129cc372d/src-tauri/src/portable.rs).
- transcribe.cpp release [`v0.4.1`, commit `4d7c1ccefd39e5a9ea36cca5f8fefc6dcc14b99b`](https://github.com/handy-computer/transcribe.cpp/tree/4d7c1ccefd39e5a9ea36cca5f8fefc6dcc14b99b).
- Maintained [safe Rust bindings](https://github.com/handy-computer/transcribe.cpp/tree/4d7c1ccefd39e5a9ea36cca5f8fefc6dcc14b99b/bindings/rust/transcribe-cpp), MIT licensed. `handy-bridge` pins the registry dependency to `=0.4.1`; `Cargo.lock` records registry checksums. No locally invented FFI layout or recognizer is introduced.

These references document the inspected implementation. They do not claim a stable public companion API promised by Handy's maintainers.

## Refresh and update behavior

`HandyBridge::refresh()` starts an owned, headless Handy CLI query every time. Run it on application start, on explicit model refresh, and immediately before validating a new recording's model choice. Additive upstream metadata and new model IDs are accepted without changing a local catalog. No model name is interpreted as proof of streaming support.

The returned catalog records an observation time, SHA-256 fingerprint of the JSON output, and executable size/modification identity. An executable replacement during the query is rejected. Because Handy does not currently expose a version argument, file identity is not presented as a semantic application version.

Malformed JSON, a changed top-level envelope, missing or incorrectly typed capability flags, duplicate model IDs, duplicate security-relevant fields, overlarge output and failed queries return errors. A caller may show an earlier catalog as stale information, but must not silently use it as a successful refresh. Missing or removed selected IDs require an explicit new choice; there is no automatic model substitution.

The CLI is called through an argument array, without a command shell. Standard input is closed. Output, diagnostics and execution time are bounded. A timeout terminates only the CLI process created by this query. Existing Handy processes and recordings are not enumerated for termination. Raw Handy diagnostic text is not persisted or exposed by bridge errors. The bridge itself writes no Handy settings, cache references, metadata or weights; Handy's own startup and registry refresh behavior remains under Handy's control.

New catalog entries can appear automatically. A model family requiring a newer native inference runtime can still require a tested HandyMeeting update. Reusing Rust/Tauri and model weights does not make an arbitrary future native ABI compatible, nor does it share a model instance already loaded in Handy. Unknown source variants are shown as unsupported at resolution rather than guessed or downloaded.

## Pinned identity of existing sessions

Every new Rust recording receives a schema-1 `model-snapshot.json` before capture starts. Its immutable identity records the canonical weight path, byte count, SHA-256, selected Handy model descriptor, actual resolved revision and the native runtime version/commit/public-header contract. Weight hashing uses a fixed 64-KiB buffer. It adds a file-read pass, not a model load.

Resume reads that snapshot and pins the original actual weight path. It compares the selected catalog entry's ID, filename and source, verifies the original file digest and native runtime contract, and refuses a mismatch before writing new transcript checkpoints. An unrelated added model or an editorial model-name/language change does not block resume. A changed pinned revision, same-size local weight edit, missing original file or changed runtime requires an explicit future migration workflow; this beta does not silently create one or substitute a model.

Each isolated recognition child repeats the snapshot/file/runtime check before loading a model, including GPU restarts and CPU replacement. Synthetic native probes create an actual selected-model identity too; production validation has no test bypass. Old unversioned snapshots remain unchanged and are refused for recognition resumption with a clear message.

## Installed weight resolution

The bridge implements a small read-only adapter around the current layout. `source.HuggingFace` entries resolve against configured/shared cache roots. It prefers the requested revision or its named ref, then the `main` ref, then other deterministic cached snapshots, matching Handy's documented resolver order. The returned result records the actual revision and whether a fallback snapshot was used. A complete manually placed or mirror-fallback file in Handy's models directory is also supported.

`Local` and `Url` entries resolve only to existing complete files in Handy's models directory. URLs are metadata and are never fetched. An active download, missing file, empty file or partial local download is rejected. Model names, filenames, repository IDs, revisions and ref contents cannot introduce absolute paths, parent traversal, Windows alternate streams or control characters. Canonical paths must remain inside the selected models root or HF repository root; legitimate HF snapshot symlinks to that repository's blobs are supported.

The standard installation roots cover Windows, macOS and Linux, including Handy portable mode and its `Data/huggingface/hub` cache. An explicitly selected executable or roots can support other packaging layouts. Directory layout is an implementation dependency until Handy provides a stable model-path discovery API.

## Streaming and isolation

The optional `native-asr` feature uses only the official safe `transcribe-cpp` API. It verifies actual loaded-model streaming capability and the required 16 kHz input rate rather than trusting catalog metadata alone. CPU replacement workers register only CPU backends, so a faulty GPU module need not be loaded for fallback.

The current safe Rust binding permits one active stream per loaded `Model`, across all its sessions. Creating another `Session` or cloning `Model` does not create an independent inference engine. Two simultaneously open track streams therefore require two isolated worker/model instances, with their additional memory cost reported honestly, or serialized bounded windows. The companion must retain capture and durable PCM writing independently of either recognition worker.

Native model and stream errors do not authorize deleting audio or stopping an already running capture. The adapter does not promise that dropping a native handle repairs every GPU-driver failure; worker process isolation and recording recovery remain higher-level responsibilities.

## Checks performed

- Fourteen synthetic Rust compatibility tests cover new IDs/additive fields, missing capabilities, changed envelopes, duplicate IDs/fields, traversal, named refs, recorded cache fallback, partial files, unknown sources, refresh replacement and no stale success after failure.
- A bounded-reader regression check confirms that oversized pipe data does not accumulate unbounded stored output. With `native-asr`, an additional actual DLL check confirms the packaged runtime matches the pinned 0.4.1 safe bindings. `cargo check`, tests and Clippy with warnings rejected passed against the prepared official Windows 0.4.1 runtime SDK; this is not an audio-capture or model-quality test.
- Six identity regression cases cover unchanged snapshot roundtrips, the same model ID at a new revision, in-place weight edits retaining their byte count, changed native runtime, unrelated catalog additions/editorial changes, and preservation/refusal of older unversioned snapshots.
- A real read-only query of the installed Handy 0.9.8 returned **85 model entries**. The installed Nemotron Streaming 3.5 Q8 model was recognized and resolved to the requested cached revision. No model was loaded or downloaded in that probe; no microphone or system-audio device was opened.
- `examples/inspect.rs` reproduces that query without printing local model paths or raw diagnostics. Tests use synthetic temporary roots; no existing session or model files are changed.

Native build/load checks and end-to-end recording evidence must be reported separately by the application's release verification. Cross-platform path candidates tested on Windows do not establish actual macOS or Linux capture or installation behavior.

## Next upstream collaboration

The smallest useful stable extension would expose machine-readable installation/runtime identity and an explicit resolved model path plus capabilities. A separate optional shared inference service would be a larger integration contract requiring ownership, versioning, resource limits and failure semantics. Neither extension is assumed to exist today.
