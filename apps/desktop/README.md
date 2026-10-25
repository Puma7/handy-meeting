# Rust desktop frontend

HandyMeeting's `1.0.0-beta.1` frontend uses the same React, TypeScript, Vite, Lucide family and shared theme palette as Handy. The backend and native application package are maintained in `src-tauri/` and the repository's Rust workspace.

The palette is reused verbatim at the pinned upstream revision documented in [HANDY_STYLE_NOTICE.md](HANDY_STYLE_NOTICE.md), including its MIT license. The app retains HandyMeeting's existing independent logo.

## Build

```sh
npm ci
npm run build
npm test
```

`npm run dev` serves on `127.0.0.1:1420` for Tauri development. In an ordinary browser the interface deliberately reports an unavailable desktop core and disables recording; it does not simulate a successful recording or invent transcript data. The `DesktopApi` interface in `src/types.ts` defines the typed IPC boundary.

All package versions are exact and `package-lock.json` is committed. Production code has no browser network client, raw HTML rendering or automatic model downloads. Cloud text generation requires a new, explicit approval dialog per run. API keys remain in component memory for the app session. Preferences and template instructions use separate versioned beta storage keys; unreadable saved templates are preserved without replacement.

`get_app_state` may return a bounded live preview. Full transcript copying explicitly requests `get_meeting` first. Saved-audio counters use confirmed samples; backlog for a limited preview requires the backend's global counter. Original source dialogs require the quoted text to exist verbatim.

Bounded status previews update the selected transcript only while recording or catching up. Completed sessions retain a full history snapshot. While `stop_meeting` waits for pending recognition, an explicit ending state remains visible even if backend recording has already stopped; another capture stays disabled. The backend's optional `transitioning` state also suppresses readiness during startup transitions.

Returning focus to an idle app refreshes the installed Handy catalogue, with a 30-second cooldown after actual queries and coalescing for an in-flight query. This waits for the backend's first startup catalogue and rechecks current backend state before querying Handy. Recording, ending and ASR catch-up block such queries. The initial catalogue may choose the first compatible model once. If that selected model later disappears or becomes incompatible, selection is cleared and the user must choose explicitly; another model is never silently substituted.

## Validation scope

`npm test` passes twenty-one regressions using a synthetic IPC adapter and isolated storage. They cover selection of separately captured tracks, refreshed upstream model lists, focus-refresh throttling/coalescing and fresh-state guards, explicit reselection after a removed model, unsupported/uninstalled model exclusion, unavailable core behavior, full transcript copying beyond the live preview, confirmed counters/null text, completed-history preservation, asynchronous stop/bootstrap states, read-only legacy handling, preservation of damaged template storage, per-run cloud consent, no stored keys, literal rendering of model text, source quote navigation, editable analysis steps and keyboard focus containment/restoration.

These are frontend regressions, not real audio-device tests, real cloud-provider tests or native WebView compatibility evidence. Native Windows/Mac/Linux checks and two-hour capture/recovery validation must be reported separately. No synthetic content is bundled into the application.
