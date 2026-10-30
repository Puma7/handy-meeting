# 30 October 2026 – first real portable-ASR CI outcomes and runtime correction

On PR35 head `b4c6494787d74ccaf846aac369ce35968f47167e`, the complete
[Mac ARM job](https://github.com/Puma7/meeting-for-handy/actions/runs/41020929728/job/123947869618)
passed the real Handy 0.9.8/Q8 CPU test: 45.133471083 seconds, 31 reference words per
track and zero word errors on either, owned worker restart, audio hashes, confirmed
journal prefix and reopened text preserved. This is short generated speech, not
physical capture, GPU, real-time performance or a two-hour result.

On head `bfce18849590fd3328a2d880eb20dbc20ced9666`, the
[Linux preflight](https://github.com/Puma7/meeting-for-handy/actions/runs/41024330019/job/123958786715)
proved a GLIBC mismatch in the upstream Ubuntu 24.04-built AppImage on our Ubuntu 22.04 host.
The official Ubuntu 22.04-built DEB is now pinned instead: 41,941,410 bytes,
SHA256 `4efe1fdcc2b68e95612e866810ccb23e06bbdf004a4e857de0862b0c6927caa8`.
Its downloaded bytes and regular `usr/bin/handy` were independently inspected,
without execution or installation. A regression requires private extraction and
rejects AppImage execution or package installation.

The same head's [Intel job](https://github.com/Puma7/meeting-for-handy/actions/runs/41024330019/job/123958786818)
exceeded the 45-second real Handy catalog limit. No Intel ASR success is claimed.
The next CPU-only attempt disables child Metal enumeration, as supported by the
verified source of Handy's transcribe-cpp-sys 0.2.4 (crate SHA256
`1c6946c7bf90046fc51e5bc25373a4f6270ca10fdcf4f35b1174075e43c535b5`).
This removes an unnecessary GPU initialization boundary; the timeout itself does
not prove where Handy stalled. New Linux/Intel success still requires actual CI.
No deadlines, content, preservation or completeness criteria were relaxed.

The extracted DEB executable uses `$ORIGIN/../lib/Handy:$ORIGIN/../lib`
for its shipped transcribe/ONNX libraries. Its additional `libgtk-layer-shell.so.0`
system dependency is explicitly installed in the disposable Linux job.
All 110 local Python guards ran (109 passed; one Unix-link case skipped on
Windows). The required project check passed. The Rust/frontend product code
retains its previously checked bytes apart from the documented diagnostic input.
These new notes contain only public CI references and synthetic checks. The
previously published full PR35 report is unchanged.


## Linux runtime dependency follow-up

The [Linux job on25aad](https://github.com/Puma7/meeting-for-handy/actions/runs/41031813793/job/123983123795)
reached the real catalog preflight but failed with a missing shared-library flag.
Reading the actual pinned DEB control file and ELF dependencies showed the remaining
external requirements: libopenblas0 and libappindicator3-1, in addition to the
already installed gtk-layer-shell/GTK/WebKit dependencies. The CI installation
list now covers these declared requirements. The private preflight can expose
only explicitly allowlisted missing-library names, never arbitrary paths/errors.
The new Linux run still has to pass; no deadline or success criterion was changed.

Both macOS jobs on historical head `25aad484` completed successfully: [ARM](https://github.com/Puma7/meeting-for-handy/actions/runs/41031813793/job/123983123654) and [Intel](https://github.com/Puma7/meeting-for-handy/actions/runs/41031813793/job/123983123681). The real pinned CPU recognizer took 30.663633958 seconds on ARM and 24.841663909 seconds on Intel. Each generated track had 31 reference words and zero word errors; both checks preserved audio hashes and journals through the owned-worker restart. These are accelerated generated-speech checks, without physical capture, GPU or endurance claims. They do not prove the next head.

After the dependency/diagnostic correction, all 111 local Python guards ran (110 passed; one Unix-only case skipped on Windows), and `./verify.ps1` passed. The missing-library diagnostic regression failed before the fix.
