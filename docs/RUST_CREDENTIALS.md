# Native credentials and template storage

The Rust desktop defaults to session-only keys. Storing a key requires a separate explicit action. Selecting a saved key for an analysis is also explicit, and cloud analysis still requires text consent for that run. Changing the endpoint or local/cloud mode changes its credential identity; localhost is normalized to loopback and the base path and port are included. There is no provider fallback.

`keyring = 4.2.0` is pinned with only its `v1` feature, using macOS Keychain Services, Windows Credential Manager or Unix Secret Service. No keyring CLI, SQLite, file-based fallback, sample/mock store or Linux kernel keyring is selected by HandyMeeting. The upstream library and stores are MIT/Apache licensed; their original texts are included in the target license collections. Source: [upstream 4.2.0 feature selection](https://github.com/open-source-cooperative/keyring-rs/blob/v4.2.0/Cargo.toml), [native v1 store contract](https://docs.rs/keyring/4.2.0/keyring/v1/index.html).

The web frontend only receives `missing`, `stored` or `unavailable`, its provider name and a generic message. It never receives a saved key. The backend loads a selected key immediately for the validated text connection. It keeps its key buffers in `zeroize 1.8.2` wrappers and clears connection key strings on drop; this reduces retained backend copies and is not a promise that all OS, HTTP or JavaScript memory can be erased. No key enters application settings, template files, snapshots, result exports or error messages. Native error objects are deliberately not formatted, since malformed-secret errors can carry secret bytes.

Before any update or removal, the existing entry must be readable. Missing native services, locked stores, ambiguous entries, malformed stored values or denied access block the operation. Readable existing values require explicit replacement confirmation. A shared process lock serializes HandyMeeting instances; edits performed directly in the operating system's own tools remain outside that application lock. There is no automatic repair by deletion or plaintext storage. If the library's first store initialization fails, reopening the app after the native service is available may be necessary.

## Commands

- `credential_status {endpoint, mode}` returns `{state, provider, message}` without a key.
- `save_credential {request: {endpoint, mode, apiKey, replaceExisting}}` explicitly creates or replaces a readable entry.
- `remove_credential {endpoint, mode, confirm}` explicitly removes a readable entry.
- `generate_notes.request.useSavedKey` defaults to false. If true, `apiKey` must be empty and the backend retrieves only the exact endpoint/mode entry. Per-run cloud text consent remains required.
- `list_templates` returns `{revision, templates}`; an empty native store returns revision zero and an empty list.
- `save_templates {library: {revision, templates}}` rejects a stale revision. `import_templates {templates}` explicitly merges; identical entries are skipped and changed colliding IDs become separate copies.

Template files contain only templates and a revision. Each confirmed library is an immutable, atomically published revision with a file sync. Writers hold a cross-process lock; unreadable or missing revisions block reset/overwrite. Limits are 100 templates, 1–100 steps per template, 100-character names, 8,000-character prompts and an 8 MB library. Original browser template storage is preserved during migration. Analysis snapshots retain the exact old template even after the reusable library changes.

## Actual versus simulated evidence

The normal Rust tests simulate locked, unreadable, ambiguous and write-denied stores; these are failure-path simulations, not native Mac/Linux checks. They also check stale template writes, cross-process exclusion, immutable prior bytes, corrupt-library preservation and explicit collision import. Loopback HTTP tests verify text-only requests, credential scope binding, absent key persistence and refused redirects/remote local-mode addresses.

The explicit `--credential-checks NEW_OUTPUT_DIRECTORY` mode uses a unique service named `org.handymeeting.credential-check.<random UUID>`. It creates only synthetic values inside Rust, checks native read-back, refuses unconfirmed replacement, explicitly replaces/reopens the entry and deletes only that invocation's synthetic entry. It writes `credential-checks.json` with provider, result and five named checks; no secret is in the JSON, shell arguments or logs. It does not call a cloud provider or touch the production credential namespace.

Windows Credential Manager was actually exercised on this Windows host on 25 October 2026: `.build/credential-native-windows-20261025-225123/credential-checks.json` for beta.2 and `.build/credential-native-windows-beta3-20261025-225934/credential-checks.json` for beta.3. Five checks passed in each, `actualNativeStore: true`, `simulated: false`. These checks do not prove behavior under every account policy or OS permission failure.

Actual Windows Credential Manager, Mac Keychain on both Apple Silicon and Intel, and Linux Secret Service each passed five isolated checks in the completed beta.3 runs: [Windows](https://github.com/Puma7/handy-meeting/actions/runs/40239590566), [Unix targets](https://github.com/Puma7/handy-meeting/actions/runs/40239590551). All records report actualNativeStore=true, simulated=false, usesUserEntries=false and cloudRequests=false. Local downloaded records are under `.build/ci-beta3-207678a/<rid>/`; physical desktop account/permission failures remain simulated separately. For GitHub-hosted Unix runners only, after building the native application:

```bash
# Linux prerequisites: dbus and gnome-keyring, in addition to app dependencies.
bash scripts/check-native-credentials.sh PATH_TO_APP .build/credential-checks-TARGET
```

The wrapper refuses to run outside GitHub Actions. Linux uses a new D-Bus session and isolated XDG directories with an unlocked synthetic Secret Service. Mac uses a newly created temporary test keychain as the default and restores the prior default on exit. The pinned Mac store searches that exact default keychain rather than all keychains in a search list. Each headless application check has a 60-second external deadline so a native permission dialog cannot hang CI indefinitely; timeout is a failed check, not successful native evidence. No existing keychain is deleted or unlocked, and no Apple SDK content is redistributed. Upload only `credential-checks.json` as the test record. The temporary native-store files contain no real provider credentials and are left for the runner's normal lifecycle cleanup.
