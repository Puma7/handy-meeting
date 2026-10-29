# Legacy C# checks during the Rust migration

The required `verify.ps1` suite still checks the preserved C# prototype alongside the new Rust work. Its results must not be presented as new Rust capture or hardware evidence.

## Confirmed legacy writer limitation

The run in `.build/verification-20261025-192925-20cb5a/monitor/failure.json` failed while the monitor test fixture called the original `JsonData.WriteAtomic` / `File.Replace` writer during 1,000 concurrent status replacements. Windows reported that the replaced file could not be removed. An earlier full run had passed, so this is intermittent filesystem-contention behavior. No specific background application or security product has been established as its cause.

This is a real availability limitation of the old production metadata writer: a replacement failure propagates instead of being retried. It is not proof that all legacy writes are reliable, nor by itself proof that acknowledged audio or journal bytes were lost. The old C# `Models.cs` writer and existing releases remain unchanged by this test correction. Rust recording journals and the Rust metadata implementation are separate and require their own evidence. Production hardening of the old writer remains open.

## What the monitor fixture now verifies

`RealtimeSoakTest.MonitorChecksAsync` is specifically a test of `ReadMonitorStateAsync`: temporary exclusive locks, missing/corrupt files, concurrent atomic replacements and the final status value. Its input writer is now an explicitly test-only fixture generator in a fresh owned directory. The 1,000 replacements, actual concurrent reads and final value 1,000 remain required.

The fixture writer retains superseded snapshots under unique backup names and preserves failed staging files. It performs no explicit deletion and refuses paths outside its owned fixture scope. Each replacement has a five-second deadline. Only Windows sharing/locking errors 32/33 and replacement error 1175 are eligible for a bounded retry; other failures propagate. Microsoft documents that 1175 leaves the replaced and replacement files under their original names. Errors 1176/1177 may involve changed names and are not blindly retried. [Microsoft ReplaceFileW documentation](https://learn.microsoft.com/windows/win32/api/winbase/nf-winbase-replacefilew).

Controlled deny-delete leases verify both recovery after a temporary release and failure at a deadline while the lease remains active. The latter must leave the protected original bytes unchanged. Separate checks retain unrelated I/O failures and reject a target outside the fixture scope. `monitor-checks.json` records actual replacement retry counts and HRESULTs separately from those deliberately induced regression cases. `LegacyProductionWriterValidated` is explicitly false.

The failed original report, prior successful runs, fixture backups and all recordings remain preserved. A passing reader test is not a claim that the unchanged legacy production writer has been fixed.

## Targeted confirmation on 25 October 2026

The C# 0.4.1 diagnostic source was built into a new `.build/monitor-fixture-fixed-20261025-193829/app` directory using the pinned .NET 10 toolchain, with compiler warnings treated as errors. This did not update an installed application or a running recording.

Two serial `--soak-monitor-checks` runs passed in the sibling `monitor-1` and `monitor-2` directories. Each completed all 1,000 atomic replacements and reached final value 1,000; 339 and 324 concurrent reads were recorded. Neither uncontrolled write loop needed a fixture retry in these runs. The deliberately held temporary leases produced six and seven retries; deliberately held deadline leases produced nine and ten retries and failed as required. Those induced errors were HRESULT `0x80070020` (sharing violation). The protected original remained unchanged, unrelated I/O failure propagated, and an outside-scope target was rejected without creating it.

These are synthetic status-file checks, not a new audio, hardware, memory or two-hour measurement. Error 1175 is covered by the documented retry policy; these particular controlled leases produced error 32 rather than a reproduced 1175. The original failed full verification remains available separately, and the full required suite must still be rerun after this source change.
