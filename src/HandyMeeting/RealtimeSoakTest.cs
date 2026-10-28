using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using NAudio.Wave;

namespace HandyMeeting;

/// <summary>Wall-clock soak with deterministic dual-track PCM; never opens a real microphone.</summary>
public static class RealtimeSoakTest
{
    public sealed record Plan(int DurationSeconds, string ModelId, string ModelPath, DateTimeOffset PreparedAt,
        string Scope = "Echtzeit-Einspeisung zweier synthetischer Spuren, produktive PCM-Sicherung, Streaming und WPF-Live-Ansicht; keine WASAPI-Geräteaufnahme",
        double HostPrivateGrowthLimitMiB = 128, double HostPrivateSlopeLimitMiBPerHour = 64,
        double ManagedGrowthLimitMiB = 32, double WorkerPrivateGrowthLimitMiB = 128,
        double WorkingSetGrowthLimitMiB = 128, string? TestedAssemblyVersion = null);
    public sealed record OsMemory(long PrivateBytes, long WorkingBytes, long CpuTicks)
    {
        public static OsMemory? Read(int? id)
        {
            if (id is null) return null;
            try { using var p = Process.GetProcessById(id.Value); return new(p.PrivateMemorySize64, p.WorkingSet64, p.TotalProcessorTime.Ticks); }
            catch (Exception error) when (error is ArgumentException or InvalidOperationException or System.ComponentModel.Win32Exception) { return null; }
        }
    }
    public sealed record State(DateTimeOffset At, DateTimeOffset StartedAt, double ElapsedSeconds, int HostPid, int? WorkerPid,
        string Session, long WrittenSamples, long SavedMs, long PendingMs, int Parts, int Restarts, int ModelLoads,
        string Backend, bool Failed, MemorySample Host, OsMemory? Worker, double MaxSchedulingDelayMs);
    public sealed record AudioReceipt(string File, string Source, long Samples, string Sha256);
    public sealed record Receipt(DateTimeOffset At, double ElapsedSeconds, string Session, long WrittenSamples,
        long JournalBytes, string JournalSha256, List<AudioReceipt> Audio);
    public sealed record Trend(string Name, int Samples, double FirstMedianMiB, double LastMedianMiB, double GrowthMiB,
        double SlopeMiBPerHour, double PeakMiB);

    [DllImport("kernel32")] private static extern uint SetThreadExecutionState(uint flags);
    private static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    private static bool IsSharingViolation(IOException error) => (error.HResult & 0xffff) is 32 or 33;
    private sealed record MonitorProbe(int Value);
    private static async Task<T> ReadMonitorStateAsync<T>(string path, bool knownExisting = false)
    {
        // Windows can briefly reject an open while ReplaceFile swaps an atomic snapshot.
        // A known snapshot can also briefly disappear during replacement. A genuinely missing
        // file still fails immediately; a persistent rename gap fails after two seconds.
        knownExisting |= File.Exists(path);
        var started = Stopwatch.StartNew(); var retries = 0;
        while (true)
        {
            try
            {
                var value = JsonData.Read<T>(path) ?? throw new InvalidDataException("Leere Statusdatei.");
                if (retries > 0) AppendJson(Path.Combine(Path.GetDirectoryName(path)!, "monitor-io-retries.jsonl"), new { At = DateTimeOffset.Now, File = Path.GetFileName(path), Retries = retries, WaitMs = started.ElapsedMilliseconds });
                return value;
            }
            catch (IOException error) when ((IsSharingViolation(error) && started.Elapsed.TotalSeconds < 60)
                || (error is FileNotFoundException && knownExisting && started.Elapsed.TotalSeconds < 2))
            { retries++; await Task.Delay(50); }
        }
    }
    public static async Task MonitorChecksAsync(string root)
    {
        var fixtureRoot = Path.Combine(root, "reader-fixtures");
        Require(!Directory.Exists(fixtureRoot) && !File.Exists(Path.Combine(root, "monitor-checks.json")), "Monitor checks require a new output directory; previous evidence is retained.");
        var writer = new MonitorFixtureWriter(fixtureRoot); var fixtureStats = new MonitorFixtureWriteStats();
        var path = Path.Combine(fixtureRoot, "monitor-probe.json");
        await writer.WriteAsync(path, JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(731)), fixtureStats, TimeSpan.FromSeconds(5));
        var locked = new FileStream(path, FileMode.Open, FileAccess.ReadWrite, FileShare.None);
        Task<MonitorProbe> reading;
        try { reading = ReadMonitorStateAsync<MonitorProbe>(path); await Task.Delay(250); }
        finally { locked.Dispose(); }
        Require((await reading).Value == 731, "Vorübergehende Statussperre wurde nicht überstanden.");
        try { await ReadMonitorStateAsync<MonitorProbe>(Path.Combine(root, "missing.json")); throw new InvalidOperationException("Fehlende Statusdatei wurde nicht erkannt."); }
        catch (FileNotFoundException) { }
        await writer.WriteAsync(path, Encoding.UTF8.GetBytes("{defekt"), fixtureStats, TimeSpan.FromSeconds(5));
        try { await ReadMonitorStateAsync<MonitorProbe>(path); throw new InvalidOperationException("Defekte Statusdatei wurde nicht erkannt."); }
        catch (JsonException) { }
        await writer.WriteAsync(path, JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(0)), fixtureStats, TimeSpan.FromSeconds(5));
        var writing = Task.Run(async () => { for (var i = 1; i <= 1000; i++) await writer.WriteAsync(path, JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(i)), fixtureStats, TimeSpan.FromSeconds(5)); });
        var reads = 0;
        while (!writing.IsCompleted) { _ = await ReadMonitorStateAsync<MonitorProbe>(path, true); reads++; await Task.Delay(1); }
        await writing;
        Require((await ReadMonitorStateAsync<MonitorProbe>(path)).Value == 1000, "Letzter atomarer Statusstand fehlt.");
        Require(reads > 0, "No concurrent monitor reads were exercised.");
        var releasedStats = new MonitorFixtureWriteStats(); var activeStats = new MonitorFixtureWriteStats();
        var releasePath = Path.Combine(fixtureRoot, "release-regression.json"); var original = JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(313));
        await writer.WriteAsync(releasePath, original, releasedStats, TimeSpan.FromSeconds(5));
        Task releaseWriting;
        using (var held = new FileStream(releasePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
        {
            releaseWriting = writer.WriteAsync(releasePath, JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(314)), releasedStats, TimeSpan.FromSeconds(2));
            await Task.Delay(75); Require(!releaseWriting.IsCompleted, "Fixture writer ignored an active deny-delete lease.");
        }
        await releaseWriting;
        Require((await ReadMonitorStateAsync<MonitorProbe>(releasePath)).Value == 314 && releasedStats.Retries > 0, "Fixture writer did not recover after temporary lease release.");
        var activePath = Path.Combine(fixtureRoot, "active-regression.json");
        await writer.WriteAsync(activePath, original, activeStats, TimeSpan.FromSeconds(5));
        using (var held = new FileStream(activePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite))
        {
            var timedOut = false;
            try { await writer.WriteAsync(activePath, JsonSerializer.SerializeToUtf8Bytes(new MonitorProbe(999)), activeStats, TimeSpan.FromMilliseconds(150)); }
            catch (TimeoutException error) when (error.InnerException is IOException io && MonitorFixtureWriter.IsRetryable(io)) { timedOut = true; }
            Require(timedOut && !held.SafeFileHandle.IsClosed, "Fixture writer must fail its deadline while a lease is still held.");
        }
        Require(original.SequenceEqual(File.ReadAllBytes(activePath)), "Fixture deadline changed the protected original file.");
        var missingFailed = false;
        try { await writer.WriteAsync(Path.Combine(fixtureRoot, "missing-directory", "state.json"), original, new(), TimeSpan.FromSeconds(1)); }
        catch (DirectoryNotFoundException) { missingFailed = true; }
        Require(missingFailed, "Fixture writer swallowed an unrelated I/O failure.");
        var outside = Path.Combine(root, "outside-fixture.json"); var outsideRejected = false;
        try { await writer.WriteAsync(outside, original, new(), TimeSpan.FromSeconds(1)); }
        catch (ArgumentException) { outsideRejected = true; }
        Require(outsideRejected && !File.Exists(outside), "Fixture writer touched a file outside its owned fixture scope.");
        ReportChecks();
        var report = new { Passed = true, ExclusiveLockSurvived = true, MissingFileReported = true, CorruptFileReported = true, AtomicReplacements = 1000, ConcurrentReads = reads,
            FixtureWriteRetries = fixtureStats.Retries, FixtureWriteHResults = fixtureStats.HResults, FixtureWriteNativeErrors = fixtureStats.NativeErrors,
            TemporaryFixtureLeaseSurvived = true, TemporaryFixtureWriteRetries = releasedStats.Retries, TemporaryFixtureHResults = releasedStats.HResults,
            ActiveFixtureLeaseDeadlineReported = true, ActiveFixtureDeadlineRetries = activeStats.Retries, ActiveFixtureHResults = activeStats.HResults,
            UnrelatedFixtureIoFailureReported = true, OutsideFixtureScopeRejected = true, ReportEvidenceChecksPassed = true, FixtureWriterOnly = true, LegacyProductionWriterValidated = false };
        using var reportFile = new FileStream(Path.Combine(root, "monitor-checks.json"), FileMode.CreateNew, FileAccess.Write, FileShare.Read);
        reportFile.Write(JsonSerializer.SerializeToUtf8Bytes(report, JsonData.Options)); reportFile.Flush(true);
    }
    private sealed class MonitorFixtureWriteStats
    {
        public int Retries { get; private set; }
        public Dictionary<string, int> HResults { get; } = [];
        public Dictionary<int, int> NativeErrors { get; } = [];
        public void Record(IOException error)
        {
            Retries++; var code = $"0x{unchecked((uint)error.HResult):X8}"; var native = error.HResult & 0xffff;
            HResults[code] = HResults.GetValueOrDefault(code) + 1; NativeErrors[native] = NativeErrors.GetValueOrDefault(native) + 1;
        }
    }
    // Deliberately independent of JsonData.WriteAtomic: this suite exercises the
    // monitor READER, not the legacy production writer's contention behavior.
    private sealed class MonitorFixtureWriter
    {
        private readonly string scope, staging;
        public MonitorFixtureWriter(string root)
        {
            scope = Path.GetFullPath(root) + Path.DirectorySeparatorChar;
            Directory.CreateDirectory(scope); staging = Path.Combine(scope, "staging"); Directory.CreateDirectory(staging);
        }
        public static bool IsRetryable(IOException error) => error.HResult == unchecked((int)0x80070020)
            || error.HResult == unchecked((int)0x80070021) || error.HResult == unchecked((int)0x80070497);
        public async Task WriteAsync(string path, byte[] bytes, MonitorFixtureWriteStats stats, TimeSpan deadline)
        {
            path = Path.GetFullPath(path);
            if (!path.StartsWith(scope, StringComparison.OrdinalIgnoreCase)) throw new ArgumentException("Fixture target is outside the owned test scope.", nameof(path));
            if (deadline <= TimeSpan.Zero) throw new ArgumentOutOfRangeException(nameof(deadline));
            var clock = Stopwatch.StartNew(); IOException? last = null;
            var next = Path.Combine(staging, $"next-{Guid.NewGuid():N}.json");
            using (var file = new FileStream(next, FileMode.CreateNew, FileAccess.Write, FileShare.Read, 4096, FileOptions.WriteThrough)) { file.Write(bytes); file.Flush(true); }
            while (true)
            {
                if (clock.Elapsed >= deadline) throw new TimeoutException("The owned fixture replacement remained blocked past its deadline.", last);
                try
                {
                    // Each superseded fixture is retained under a new backup name.
                    // ERROR_UNABLE_TO_REMOVE_REPLACED (1175) leaves both original
                    // names unchanged; partially moved 1176/1177 states are not retried.
                    if (File.Exists(path)) File.Replace(next, path, Path.Combine(staging, $"previous-{Guid.NewGuid():N}.json"), true);
                    else File.Move(next, path);
                    return;
                }
                catch (IOException error) when (IsRetryable(error))
                {
                    last = error; stats.Record(error); var remaining = deadline - clock.Elapsed;
                    if (remaining <= TimeSpan.Zero) throw new TimeoutException("The owned fixture replacement remained blocked past its deadline.", error);
                    await Task.Delay(remaining < TimeSpan.FromMilliseconds(10) ? remaining : TimeSpan.FromMilliseconds(10));
                }
            }
        }
    }
    private static void AppendJson(string path, object value)
    {
        using var file = new FileStream(path, FileMode.Append, FileAccess.Write, FileShare.Read, 4096, FileOptions.WriteThrough);
        var bytes = JsonSerializer.SerializeToUtf8Bytes(value); file.Write(bytes); file.WriteByte((byte)'\n'); file.Flush(true);
    }
    private static string HashPrefix(string path, long offset, long count)
    {
        using var file = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        file.Position = offset; using var hash = IncrementalHash.CreateHash(HashAlgorithmName.SHA256);
        Span<byte> buffer = stackalloc byte[16_384];
        while (count > 0) { var read = file.Read(buffer[..(int)Math.Min(count, buffer.Length)]); if (read == 0) throw new EndOfStreamException(path); hash.AppendData(buffer[..read]); count -= read; }
        return Convert.ToHexString(hash.GetHashAndReset());
    }
    private static Receipt TakeReceipt(SessionStore session, long written, double elapsed)
    {
        var audio = session.PartsSnapshot().Select(p => new AudioReceipt(p.File, p.Source, p.SampleCount, HashPrefix(session.AudioPath(p), 44, p.SampleCount * 2))).ToList();
        var length = new FileInfo(session.JournalPath).Length;
        Require(length > 0, "Vor dem Absturz existiert kein gespeichertes Textjournal.");
        return new(DateTimeOffset.Now, elapsed, session.Folder, written, length, HashPrefix(session.JournalPath, 0, length), audio);
    }
    private static void VerifyReceipt(Receipt receipt)
    {
        var journal = Path.Combine(receipt.Session, "transcript-journal.jsonl");
        Require(HashPrefix(journal, 0, receipt.JournalBytes) == receipt.JournalSha256, "Ein zuvor gesicherter Textstand wurde verändert oder ging verloren.");
        foreach (var part in receipt.Audio)
            Require(HashPrefix(Path.Combine(receipt.Session, "audio", part.File), 44, part.Samples * 2) == part.Sha256, "Bereits bestätigtes Audio fehlt oder ist verändert: " + part.File);
    }
    private static byte[] MakeFixture(string root, string phrase, string source)
    {
        var folder = Path.Combine(root, "fixtures", source); Directory.CreateDirectory(folder);
        using var speech = new WaveFileReader(StreamingVerification.Speech(folder, phrase));
        Require(speech.WaveFormat.SampleRate == 16000 && speech.WaveFormat.BitsPerSample == 16 && speech.WaveFormat.Channels == 1, "Teststimme hat ein unerwartetes PCM-Format.");
        var spoken = new byte[checked((int)speech.Length)]; speech.ReadExactly(spoken);
        var pcm = new byte[30 * PcmTimeline.SampleRate * 2];
        for (var i = 0; i < pcm.Length; i += spoken.Length + 16000) spoken.AsSpan(0, Math.Min(spoken.Length, pcm.Length - i)).CopyTo(pcm.AsSpan(i));
        File.WriteAllBytes(Path.Combine(root, "fixtures", source + ".pcm"), pcm); return pcm;
    }
    public static async Task HostAsync(string root)
    {
        var plan = JsonData.Read<Plan>(Path.Combine(root, "plan.json"))!;
        var pcm = new Dictionary<string, byte[]> { ["system"] = File.ReadAllBytes(Path.Combine(root, "fixtures", "system.pcm")), ["microphone"] = File.ReadAllBytes(Path.Combine(root, "fixtures", "microphone.pcm")) };
        var session = SessionStore.Create(Path.Combine(root, "sessions"), "Zwei Stunden Echtzeit-Dauertest", plan.ModelId, new("synthetic-system", "Synthetischer Computerton"), new("synthetic-mic", "Synthetisches Mikrofon"));
        using var lease = session.AcquireLease();
        await using var streaming = new StreamingCoordinator(session, plan.ModelId, plan.ModelPath, true);
        using var system = new PcmTimeline(Path.Combine(session.Folder, "audio"), "system", 30, session.AddPart, p => session.AudioCheckpoint(p, false));
        using var microphone = new PcmTimeline(Path.Combine(session.Folder, "audio"), "microphone", 30, session.AddPart, p => session.AudioCheckpoint(p, false));
        // Exercise the actual XAML controls, live-text updates and layout, without displaying a window.
        var window = new MainWindow(); window.AttachSoakSessionForTest(session, streaming);
        var started = DateTimeOffset.Now; var clock = Stopwatch.StartNew(); streaming.Start();
        long written = 0; double lastState = -10, lastLayout = -1, maxDelay = 0;
        var killed = new bool[2]; var recovered = new bool[2]; var savedAtKill = new long[2];
        var crashAt = new[] { plan.DurationSeconds / 6d, plan.DurationSeconds * 2d / 3 };
        while (true)
        {
            var elapsed = clock.Elapsed.TotalSeconds;
            var target = (long)(Math.Min(elapsed, plan.DurationSeconds) * 10) * 1600;
            while (written + 1600 <= target)
            {
                maxDelay = Math.Max(maxDelay, elapsed * 1000 - (written + 1600) * 1000d / PcmTimeline.SampleRate);
                var offset = (int)(written * 2 % pcm["system"].Length);
                system.Append(pcm["system"].AsSpan(offset, 3200), written);
                microphone.Append(pcm["microphone"].AsSpan(offset, 3200), written);
                written += 1600;
            }
            system.Checkpoint(); microphone.Checkpoint();
            if (elapsed - lastLayout >= 0.25) { window.RefreshSoakLayoutForTest(); lastLayout = elapsed; }
            for (var i = 0; i < 2; i++)
            {
                if (!killed[i] && elapsed >= crashAt[i] && streaming.WorkerProcessIdForTest is not null)
                {
                    JsonData.Save(Path.Combine(root, $"worker-crash-{i + 1}.json"), TakeReceipt(session, written, elapsed));
                    savedAtKill[i] = session.SavedAudioMs; streaming.KillWorkerForTest(); killed[i] = true;
                }
                if (killed[i] && !recovered[i] && streaming.RestartCount >= i + 1 && session.SavedAudioMs > savedAtKill[i] + 10000 && session.PendingAudioMs(plan.ModelId) < 2000)
                {
                    recovered[i] = true;
                    JsonData.Save(Path.Combine(root, $"worker-recovered-{i + 1}.json"), new { At = DateTimeOffset.Now, ElapsedSeconds = elapsed, SavedMs = session.SavedAudioMs, PendingMs = session.PendingAudioMs(plan.ModelId), streaming.RestartCount, streaming.ModelLoadCount });
                }
            }
            if (elapsed - lastState >= 5)
            {
                var state = new State(DateTimeOffset.Now, started, elapsed, Environment.ProcessId, streaming.WorkerProcessIdForTest, session.Folder, written, session.SavedAudioMs, session.PendingAudioMs(plan.ModelId), session.Data.Parts.Count, streaming.RestartCount, streaming.ModelLoadCount, streaming.Backend, streaming.HasFailed, MemorySample.Read(), OsMemory.Read(streaming.WorkerProcessIdForTest), maxDelay);
                JsonData.Save(Path.Combine(root, "host-state.json"), state); AppendJson(Path.Combine(root, "memory.jsonl"), state); lastState = elapsed;
                Require(!streaming.HasFailed, "Die Streaming-Erkennung blieb nach wiederholten Fehlern stehen.");
            }
            if (elapsed >= plan.DurationSeconds)
            {
                Require(killed.All(v => v) && recovered.All(v => v), "Nicht alle geplanten Erkennungsabbrüche wurden aufgeholt.");
                var receipt = TakeReceipt(session, written, clock.Elapsed.TotalSeconds);
                JsonData.Save(Path.Combine(root, "host-crash-ready.json"), receipt);
                // Deliberately do not close, finalize, flush, or dispose the live recording here.
                await Task.Delay(Timeout.Infinite); return;
            }
            await Task.Delay(20);
        }
    }
    public static async Task RunAsync(string root, int seconds)
    {
        Require(seconds is >= 60 and <= 14400, "Dauer muss zwischen 60 und 14400 Sekunden liegen.");
        Require(!File.Exists(Path.Combine(root, "plan.json")), "Für jeden Dauertest ist ein neuer Ausgabeordner erforderlich.");
        Require(new DriveInfo(Path.GetPathRoot(root)!).AvailableFreeSpace > 2_000_000_000L, "Für den Dauertest sind mindestens 2 GB freier Platz erforderlich.");
        var monitorChecks = Path.Combine(root, "monitor-checks"); Directory.CreateDirectory(monitorChecks);
        await MonitorChecksAsync(monitorChecks);
        JsonData.Save(Path.Combine(root, "status.json"), new { Phase = "preparing", At = DateTimeOffset.Now });
        var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
        var model = (await new HandyClient(handy).ModelsAsync(CancellationToken.None)).First(m => m.SupportsStreaming && m.Id.Contains("nemotron-3.5"));
        MakeFixture(root, "Dies ist die Systemspur. Am Montag besprechen wir den Projektplan. Die Aufnahme bleibt auf diesem Computer.", "system");
        MakeFixture(root, "Dies ist meine Mikrofonspur. Heute kaufen wir Bananen und Äpfel. Meine Stimme bleibt von der Systemspur getrennt.", "microphone");
        var plan = new Plan(seconds, model.Id, ModelPaths.Resolve(model, handy), DateTimeOffset.Now,
            TestedAssemblyVersion: typeof(RealtimeSoakTest).Assembly.GetName().Version?.ToString());
        JsonData.Save(Path.Combine(root, "plan.json"), plan);
        JsonData.SaveText(Path.Combine(root, "tested-binary.sha256"), Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(typeof(RealtimeSoakTest).Assembly.Location))));
        var powerRequest = SetThreadExecutionState(0x80000001); // Keep the system awake, leave display policy alone.
        var info = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true };
        info.ArgumentList.Add("--soak-host"); info.ArgumentList.Add(root);
        using var host = Process.Start(info) ?? throw new InvalidOperationException("Dauertestprozess konnte nicht starten.");
        using var job = new WorkerJob(host);
        var watch = Stopwatch.StartNew();
        try
        {
            while (!File.Exists(Path.Combine(root, "host-crash-ready.json")))
            {
                if (host.HasExited)
                {
                    var failure = Path.Combine(root, "failure.json");
                    throw new InvalidOperationException("Der Aufnahmeprozess wurde ungeplant beendet. " + (File.Exists(failure) ? File.ReadAllText(failure) : "Keine weitere Diagnose vorhanden."));
                }
                Require(watch.Elapsed.TotalSeconds < seconds + 180, "Der Echtzeittest überschreitet seine zulässige Dauer.");
                if (File.Exists(Path.Combine(root, "host-state.json")))
                {
                    var state = await ReadMonitorStateAsync<State>(Path.Combine(root, "host-state.json"), true);
                    Require((DateTimeOffset.Now - state.At).TotalSeconds < 60, "Seit über einer Minute kein Testfortschritt.");
                    JsonData.Save(Path.Combine(root, "status.json"), new { Phase = "recording", At = DateTimeOffset.Now, StartedAt = state.StartedAt, ExpectedEndAt = state.StartedAt.AddSeconds(seconds), state.ElapsedSeconds, state.HostPid, state.WorkerPid, state.SavedMs, state.PendingMs, state.Parts, state.Restarts, state.Backend, HostPrivateMiB = state.Host.PrivateBytes / 1048576d, HostWorkingMiB = state.Host.WorkingBytes / 1048576d, WorkerPrivateMiB = state.Worker?.PrivateBytes / 1048576d, WorkerWorkingMiB = state.Worker?.WorkingBytes / 1048576d });
                }
                await Task.Delay(1000);
            }
            var beforeCrash = JsonData.Read<Receipt>(Path.Combine(root, "host-crash-ready.json"))!;
            host.Kill(true); await host.WaitForExitAsync();
            JsonData.Save(Path.Combine(root, "status.json"), new { Phase = "recovering", At = DateTimeOffset.Now });
            var recovered = SessionStore.Open(Path.Combine(beforeCrash.Session, "session.json"));
            VerifyReceipt(beforeCrash);
            for (var i = 1; i <= 2; i++) VerifyReceipt(JsonData.Read<Receipt>(Path.Combine(root, $"worker-crash-{i}.json"))!);
            var parts = recovered.PartsSnapshot();
            var tails = new Dictionary<string, long>();
            foreach (var source in new[] { "system", "microphone" })
            {
                long end = 0; var fixture = File.ReadAllBytes(Path.Combine(root, "fixtures", source + ".pcm"));
                foreach (var part in parts.Where(p => p.Source == source).OrderBy(p => p.StartSample))
                {
                    Require(part.StartSample == end, "Eine Audiospur enthält eine Lücke oder Überlappung.");
                    VerifyPcm(recovered.AudioPath(part), part.StartSample, part.SampleCount, fixture); end += part.SampleCount;
                }
                var tail = beforeCrash.WrittenSamples - end;
                Require(tail >= 0 && tail <= PcmTimeline.SampleRate / 2, "Mehr als 0,5 Sekunden unbestätigter Audio-Schluss fehlen nach dem Abbruch.");
                if (seconds >= 7200) Require(end >= 7200L * PcmTimeline.SampleRate, "Weniger als zwei Stunden Audio wiederhergestellt.");
                tails[source] = tail;
            }
            var reprocessing = Stopwatch.StartNew();
            using (recovered.AcquireLease())
            {
                await using var streaming = new StreamingCoordinator(recovered, plan.ModelId, plan.ModelPath, false); streaming.Start(); await streaming.FinishAsync();
                Require(recovered.Data.Status == "complete" && recovered.AllPartsComplete(plan.ModelId), "Offene Abschnitte konnten nicht vollständig wiederhergestellt werden.");
                recovered.Data.EndedAt = DateTimeOffset.Now; recovered.Save(); recovered.ExportSnapshot();
            }
            var semanticErrors = new List<string>();
            VerifyReceipt(beforeCrash);
            for (var i = 1; i <= 2; i++) VerifyReceipt(JsonData.Read<Receipt>(Path.Combine(root, $"worker-crash-{i}.json"))!);
            foreach (var part in recovered.PartsSnapshot().Where(p => p.SampleCount >= 29 * PcmTimeline.SampleRate))
            {
                var own = part.Source == "system" ? "Projektplan" : "Bananen"; var other = part.Source == "system" ? "Bananen" : "Projektplan";
                if (part.Text?.Contains(own, StringComparison.OrdinalIgnoreCase) != true || part.Text.Contains(other, StringComparison.OrdinalIgnoreCase)) semanticErrors.Add(part.File);
            }
            Require(semanticErrors.Count == 0, "Testtexte fehlen oder sind vermischt: " + string.Join(", ", semanticErrors.Take(10)));
            var states = File.ReadLines(Path.Combine(root, "memory.jsonl")).Select(line => JsonSerializer.Deserialize<State>(line, JsonData.Options)!).ToList();
            var trends = EvaluateMemory(states); var concerns = MemoryConcerns(plan, trends);
            var result = new { Success = concerns.Count == 0, DataRecoveryPassed = true, WallClockSeconds = beforeCrash.ElapsedSeconds, TwoHourRealtime = seconds >= 7200, AudioBytes = parts.Sum(p => p.SampleCount * 2), AudioParts = parts.Count, WorkerCrashes = 2, RecordingProcessCrashed = true, ProtectedAudioHashesPreserved = true, TextJournalPrefixesPreserved = true, AudioMatchesBothReferenceStreams = true, FinalTranscriptComplete = true, UnflushedTailSamples = tails, ReprocessingMs = reprocessing.ElapsedMilliseconds, MemoryVerdict = seconds < 1800 ? "Kurzprüfung; kein Langzeiturteil" : concerns.Count == 0 ? "Unter den vorab festgelegten Prüfgrenzen unauffällig" : "Auffälligkeiten prüfen", MemoryConcerns = concerns, Trends = trends, Session = recovered.Folder };
            JsonData.Save(Path.Combine(root, "result.json"), result);
            JsonData.SaveText(Path.Combine(root, "REPORT.md"), RenderReport(plan, beforeCrash, parts, tails, trends, concerns, recovered.Folder));
            JsonData.Save(Path.Combine(root, "status.json"), new { Phase = "complete", At = DateTimeOffset.Now, result.Success, result.DataRecoveryPassed, result.MemoryVerdict, Report = Path.Combine(root, "REPORT.md") });
        }
        catch (Exception error)
        {
            JsonData.Save(Path.Combine(root, "status.json"), new { Phase = "failed", At = DateTimeOffset.Now, Error = error.ToString() });
            throw;
        }
        finally
        {
            if (!host.HasExited) { host.Kill(true); await host.WaitForExitAsync(); }
            if (powerRequest != 0) SetThreadExecutionState(0x80000000);
        }
    }
    public static async Task RecoverInterruptedAsync(string root)
    {
        Require(!File.Exists(Path.Combine(root, "interrupted-recovery.json")) && !File.Exists(Path.Combine(root, "REPORT-INTERRUPTED.md")),
            "Ein Wiederherstellungsnachweis ist bereits vorhanden; frühere Berichte bleiben erhalten.");
        var plan = JsonData.Read<Plan>(Path.Combine(root, "plan.json"))!;
        var last = JsonData.Read<State>(Path.Combine(root, "host-state.json"))!;
        try { using var active = Process.GetProcessById(last.HostPid); Require(active.HasExited, "Eine noch laufende Testsitzung darf nicht repariert werden."); }
        catch (ArgumentException) { }
        var receipts = Directory.EnumerateFiles(root, "worker-crash-*.json").Order().Select(p => JsonData.Read<Receipt>(p)!).ToList();
        foreach (var receipt in receipts) VerifyReceipt(receipt);
        var recovered = SessionStore.Open(Path.Combine(last.Session, "session.json"));
        var samples = new Dictionary<string, long>();
        foreach (var source in new[] { "system", "microphone" })
        {
            long end = 0; var fixture = File.ReadAllBytes(Path.Combine(root, "fixtures", source + ".pcm"));
            foreach (var part in recovered.PartsSnapshot().Where(p => p.Source == source).OrderBy(p => p.StartSample))
            {
                Require(part.StartSample == end, "Lücke oder Überlappung nach ungeplantem Testabbruch.");
                VerifyPcm(recovered.AudioPath(part), part.StartSample, part.SampleCount, fixture); end += part.SampleCount;
            }
            samples[source] = end;
        }
        using (recovered.AcquireLease())
        {
            await using var streaming = new StreamingCoordinator(recovered, plan.ModelId, plan.ModelPath, false);
            streaming.Start(); await streaming.FinishAsync();
            Require(recovered.Data.Status == "complete" && recovered.AllPartsComplete(plan.ModelId), "Nach Verarbeitung des unterbrochenen Tests fehlen Textabschnitte.");
            recovered.Data.EndedAt = DateTimeOffset.Now; recovered.Save(); recovered.ExportSnapshot();
        }
        foreach (var receipt in receipts) VerifyReceipt(receipt);
        var referenceParts = recovered.PartsSnapshot().Where(p => p.SampleCount >= 29 * PcmTimeline.SampleRate).ToList();
        var semanticErrors = referenceParts.Where(p => p.Text?.Contains(p.Source == "system" ? "Projektplan" : "Bananen", StringComparison.OrdinalIgnoreCase) != true
                || p.Text.Contains(p.Source == "system" ? "Bananen" : "Projektplan", StringComparison.OrdinalIgnoreCase)).Select(p => p.File).ToList();
        Require(semanticErrors.Count == 0, "Referenztexte sind unvollständig oder vermischt: " + string.Join(", ", semanticErrors.Take(10)));
        var states = File.ReadLines(Path.Combine(root, "memory.jsonl")).Select(line => JsonSerializer.Deserialize<State>(line, JsonData.Options)!).ToList();
        var trends = EvaluateMemory(states);
        JsonData.Save(Path.Combine(root, "interrupted-recovery.json"), new { DataRecoveryPassed = true, TwoHourRealtime = false, LastObservedSeconds = last.ElapsedSeconds, RecoveredSamples = samples, ProtectedCheckpointCount = receipts.Count, ProtectedAudioAndJournalPreserved = receipts.Count > 0 ? (bool?)true : null, AllSavedAudioMatchesReferences = true, FinalTranscriptComplete = true, ReferenceTermSectionsChecked = referenceParts.Count, Trends = trends, Session = recovered.Folder });
        JsonData.SaveText(Path.Combine(root, "REPORT-INTERRUPTED.md"), RenderInterruptedReport(plan, last.ElapsedSeconds, receipts.Count, samples, recovered.Folder, referenceParts.Count));
    }
    private static string TestedVersion(Plan plan) => plan.TestedAssemblyVersion ?? "Version im Testplan nicht gespeichert";
    private static string RenderInterruptedReport(Plan plan, double lastObservedSeconds, int receiptCount, Dictionary<string, long> samples, string session, int referenceSections)
    {
        var checkpoints = receiptCount == 0
            ? "Es lag keine gespeicherte Abbruchquittung vor. Der Erhalt früher bestätigter Audio- und Journalpräfixe lässt sich daher nicht anhand solcher Quittungen nachweisen."
            : $"Für {receiptCount} gespeicherte Abbruchquittung(en) blieben die darin bestätigten Audio- und Journalpräfixe hashgleich erhalten.";
        var referenceCheck = referenceSections == 0 ? "Es gab keinen Abschnitt ab 29 Sekunden für die Referenzbegriffsprüfung; eine korrekte Begriffserkennung oder Spurzuordnung ist dadurch nicht nachgewiesen." : $"In {referenceSections} geprüften Abschnitten ab 29 Sekunden waren die Referenzbegriffe vorhanden und der richtigen Spur zugeordnet. Kürzere Abschnitte waren nicht Teil dieser Begriffsprüfung.";
        return $"# Unterbrochener Dauertest HandyMeeting {TestedVersion(plan)}\n\nLetzter beobachteter Teststand: {lastObservedSeconds / 60:F2} Minuten; geplante Dauer: {plan.DurationSeconds / 60d:F2} Minuten. Der reguläre Testabschluss ist nicht nachgewiesen. Ungeklärte Abbruchursache: Die Ursache wird aus den Messständen und Abbruchquittungen nicht abgeleitet. Vorhandene ursprüngliche failure.json und status.json bleiben als Diagnose erhalten.\n\nDie anschließende Wiederherstellungsprüfung bestand: Alle gespeicherten PCM-Samples entsprechen ihrer jeweiligen Referenz; keine Lücken oder Überlappungen. {checkpoints} Alle vorhandenen Audioabschnitte wurden fertig transkribiert. {referenceCheck}\n\nWiederhergestellter Zeitumfang: Computerton {samples["system"] / (double)PcmTimeline.SampleRate:F3} Sekunden; Mikrofon {samples["microphone"] / (double)PcmTimeline.SampleRate:F3} Sekunden. Ein genauer unbestätigter Schluss lässt sich mangels abschließender Absturzquittung in diesem ungeplanten Abbruch nicht bestimmen. Die Rohdaten und Speichertrends stehen in interrupted-recovery.json und memory.jsonl.\n\nTestsitzung: `{session}`. Dies ist ein Teilergebnis und ersetzt keinen regulär abgeschlossenen Echtzeit-Dauertest. Ein Prozessabbruch ist kein Stromausfall.\n";
    }
    private static void ReportChecks()
    {
        var plan = new Plan(7200, "synthetic-model", "unused-fixture-model", DateTimeOffset.UnixEpoch, TestedAssemblyVersion: "9.8.7.6");
        var receipt = new Receipt(DateTimeOffset.UnixEpoch, 120, "unused-fixture-session", 0, 0, "", []);
        var complete = RenderReport(plan, receipt, [], [], [], [], "unused-fixture-session");
        Require(complete.Contains("HandyMeeting 9.8.7.6", StringComparison.Ordinal) && !complete.Contains("0.2.1", StringComparison.Ordinal), "Report must identify the assembly version saved before the test.");
        var legacy = JsonSerializer.Deserialize<Plan>("{\"DurationSeconds\":7200,\"ModelId\":\"fixture\",\"ModelPath\":\"unused\",\"PreparedAt\":\"2026-01-01T00:00:00Z\"}", JsonData.Options)!;
        Require(RenderReport(legacy, receipt, [], [], [], [], "unused").Contains("Version im Testplan nicht gespeichert", StringComparison.Ordinal), "An old test plan must not inherit the reporting program's version.");
        foreach (var count in new[] { 0, 1, 3 })
        {
            var interrupted = RenderInterruptedReport(plan, 120, count, new() { ["system"] = 16000, ["microphone"] = 32000 }, "unused", 0);
            Require(interrupted.Contains("Ungeklärte Abbruchursache", StringComparison.Ordinal) && !interrupted.Contains("Dateisperre", StringComparison.Ordinal) && !interrupted.Contains("beiden Erkennungsabbrüchen", StringComparison.Ordinal), "An interrupted report must not invent a cause or two worker crashes.");
            Require(interrupted.Contains("keinen Abschnitt ab 29 Sekunden", StringComparison.Ordinal) && !interrupted.Contains("waren die Referenzbegriffe vorhanden", StringComparison.Ordinal), "An interrupted report cannot claim word recognition when no sufficiently long source section was checked.");
            Require(count == 0 ? interrupted.Contains("keine gespeicherte Abbruchquittung", StringComparison.Ordinal) : interrupted.Contains($"Für {count} gespeicherte Abbruchquittung(en)", StringComparison.Ordinal), "Interrupted reports must describe only the available receipt count.");
        }
        var checkedReference = RenderInterruptedReport(plan, 120, 1, new() { ["system"] = 480000, ["microphone"] = 32000 }, "unused", 1);
        Require(checkedReference.Contains("In 1 geprüften Abschnitten ab 29 Sekunden", StringComparison.Ordinal) && checkedReference.Contains("Kürzere Abschnitte waren nicht Teil", StringComparison.Ordinal), "The report must keep checked reference sections separate from shorter completed transcripts.");
    }
    private static void VerifyPcm(string path, long start, long samples, byte[] fixture)
    {
        using var audio = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.Read);
        Require(audio.Length == 44 + samples * 2, "Unerwartete Audiodateilänge."); audio.Position = 44;
        var buffer = new byte[16_000]; long done = 0;
        while (done < samples * 2)
        {
            var offset = (int)((start * 2 + done) % fixture.Length);
            var count = (int)Math.Min(Math.Min(buffer.Length, samples * 2 - done), fixture.Length - offset);
            audio.ReadExactly(buffer.AsSpan(0, count));
            Require(buffer.AsSpan(0, count).SequenceEqual(fixture.AsSpan(offset, count)), "Gespeichertes Audio weicht von der Referenz ab: " + path); done += count;
        }
    }
    private static double Median(IEnumerable<double> values)
    {
        var array = values.Order().ToArray(); return array.Length == 0 ? 0 : array[array.Length / 2];
    }
    private static Trend Analyze(string name, List<(double Time, double Value)> points)
    {
        var start = points[0].Time; var end = points[^1].Time; var window = Math.Min(600, (end - start) / 3);
        var first = Median(points.Where(p => p.Time <= start + window).Select(p => p.Value));
        var last = Median(points.Where(p => p.Time >= end - window).Select(p => p.Value));
        var meanTime = points.Average(p => p.Time); var meanValue = points.Average(p => p.Value);
        var divisor = points.Sum(p => (p.Time - meanTime) * (p.Time - meanTime));
        var slope = divisor == 0 ? 0 : points.Sum(p => (p.Time - meanTime) * (p.Value - meanValue)) / divisor * 3600;
        return new(name, points.Count, first, last, last - first, slope, points.Max(p => p.Value));
    }
    private static List<Trend> EvaluateMemory(List<State> states)
    {
        var trends = new List<Trend>(); var host = states.Where(s => s.ElapsedSeconds >= 300).ToList();
        if (host.Count >= 3)
        {
            trends.Add(Analyze("Host Private", host.Select(s => (s.ElapsedSeconds, s.Host.PrivateBytes / 1048576d)).ToList()));
            trends.Add(Analyze("Host Working Set", host.Select(s => (s.ElapsedSeconds, s.Host.WorkingBytes / 1048576d)).ToList()));
            trends.Add(Analyze("Host Managed", host.Select(s => (s.ElapsedSeconds, s.Host.ManagedBytes / 1048576d)).ToList()));
        }
        foreach (var epoch in states.Where(s => s.WorkerPid is not null && s.Worker is not null).GroupBy(s => s.WorkerPid))
        {
            var warm = epoch.First().ElapsedSeconds + 120; var points = epoch.Where(s => s.ElapsedSeconds >= warm).ToList();
            if (points.Count < 3) continue;
            trends.Add(Analyze($"Worker {epoch.Key} {epoch.First().Backend} Private", points.Select(s => (s.ElapsedSeconds, s.Worker!.PrivateBytes / 1048576d)).ToList()));
            trends.Add(Analyze($"Worker {epoch.Key} {epoch.First().Backend} Working Set", points.Select(s => (s.ElapsedSeconds, s.Worker!.WorkingBytes / 1048576d)).ToList()));
        }
        return trends;
    }
    private static List<string> MemoryConcerns(Plan plan, List<Trend> trends)
    {
        var concerns = new List<string>();
        foreach (var trend in trends)
        {
            if (trend.Name == "Host Managed" && trend.GrowthMiB > plan.ManagedGrowthLimitMiB) concerns.Add("Verwalteter Host-Speicher steigt um mehr als 32 MiB.");
            if (trend.Name == "Host Private" && (trend.GrowthMiB > plan.HostPrivateGrowthLimitMiB || trend.SlopeMiBPerHour > plan.HostPrivateSlopeLimitMiBPerHour)) concerns.Add("Host Private Bytes überschreiten die vorab gesetzten Wachstumsgrenzen.");
            if (trend.Name.StartsWith("Worker") && trend.Name.EndsWith("Private") && (trend.GrowthMiB > plan.WorkerPrivateGrowthLimitMiB || trend.SlopeMiBPerHour > plan.WorkerPrivateGrowthLimitMiB)) concerns.Add(trend.Name + ": auffälliger Anstieg.");
            if (trend.Name.EndsWith("Working Set") && trend.GrowthMiB > plan.WorkingSetGrowthLimitMiB) concerns.Add(trend.Name + ": mehr als 128 MiB Wachstum.");
        }
        return concerns;
    }
    private static string RenderReport(Plan plan, Receipt receipt, List<AudioPart> parts, Dictionary<string, long> tails, List<Trend> trends, List<string> concerns, string session)
    {
        var output = new StringBuilder($"# Echtzeit-Dauertest HandyMeeting {TestedVersion(plan)}\n\n");
        output.AppendLine($"Abgeschlossen: {DateTimeOffset.Now:dd.MM.yyyy HH:mm:ss zzz}. Tatsächliche Aufnahmelaufzeit: {TimeSpan.FromSeconds(receipt.ElapsedSeconds)}. Zwei getrennte synthetische Spuren mit lokalem Nemotron Streaming 3.5.\n");
        output.AppendLine("Produktive PCM-Sicherung, Textjournal, Streaming-Koordination und WPF-Live-Text inklusive Layout wurden verwendet. Die Testoberfläche blieb unsichtbar. WASAPI-Geräte, echte Mikrofone, Lautsprechertreiber und die Grafikdarstellung eines sichtbaren Fensters waren nicht Teil dieser Prüfung.\n");
        output.AppendLine($"{parts.Count} Audioabschnitte, {parts.Sum(p => p.SampleCount * 2):N0} PCM-Bytes. Alle gespeicherten Samples entsprechen bytegenau der jeweiligen Referenzspur. Keine Lücken oder Überlappungen.\n");
        output.AppendLine("Zwei harte Abbrüche des Erkennungsprozesses wurden bei weiterlaufender Aufnahme aufgeholt. Anschließend wurde der gesamte Test-Aufnahmeprozess ohne reguläres Beenden abgebrochen. Alle vorher bestätigten Audiopräfixe und Textjournal-Präfixe blieben hashgleich erhalten. Die offenen Abschnitte wurden nach dem Neustart vollständig erkannt. Der Referenzbegriff war in jedem vollständigen 30-Sekunden-Abschnitt vorhanden und blieb der richtigen Spur zugeordnet.\n");
        output.AppendLine("Nach dem zweiten Erkennungsabbruch verwendet die bestehende Wiederanlauflogik den lokalen CPU-Ersatzbetrieb. Die Speichertrends werden deshalb je Prozess und Rechenweg getrennt ausgewertet. Der höhere Modellbedarf des CPU-Betriebs darf nicht als Wachstum desselben GPU-Prozesses interpretiert werden.\n");
        foreach (var (source, tail) in tails) output.AppendLine($"Unbestätigter Schluss nach Prozessabbruch, {source}: {tail} Samples / {tail * 1000d / PcmTimeline.SampleRate:F1} ms. Dieser Teil war vor dem Abbruch noch nicht als dauerhaft gesichert bestätigt.");
        output.AppendLine("\n## Speicherwerte\n\nMiB, getrennte Messgrößen; Working Set und Private Bytes werden nicht addiert. Messpunkte alle fünf Sekunden. Für die Trendberechnung wurden beim Host die ersten fünf Minuten, je Erkennungsprozess die ersten zwei Minuten ausgeschlossen. Mediane des Anfangs-/Endbereichs und lineare Trends können Schwankungen der Speicherbereinigung einordnen, beweisen aber keine allgemeine Speicherleckfreiheit.\n");
        output.AppendLine("| Messreihe | Anfangsmedian | Endmedian | Wachstum | Trend MiB/h | Spitze |\n| --- | ---: | ---: | ---: | ---: | ---: |");
        foreach (var t in trends) output.AppendLine(FormattableString.Invariant($"| {t.Name} | {t.FirstMedianMiB:F1} | {t.LastMedianMiB:F1} | {t.GrowthMiB:F1} | {t.SlopeMiBPerHour:F1} | {t.PeakMiB:F1} |"));
        output.AppendLine("\n" + (plan.DurationSeconds < 1800 ? "Kurzprüfung des Testablaufs; kein Langzeiturteil." : concerns.Count == 0 ? "Unter den vorab festgelegten Prüfgrenzen unauffällig." : "Auffälligkeiten: " + string.Join(" ", concerns)));
        output.AppendLine("\nPrüfgrenzen vor Testbeginn in plan.json: Host Private maximal 128 MiB Medianwachstum und 64 MiB/h Trend; verwalteter Host-Speicher maximal 32 MiB Medianwachstum; Worker Private maximal 128 MiB Medianwachstum und 128 MiB/h Trend; Working Set maximal 128 MiB Medianwachstum. Positive Trends unterhalb dieser Grenzen bleiben in der Tabelle sichtbar. Der Grafikspeicher wurde nicht separat gemessen.\n");
        output.AppendLine("Ein Prozessabbruch ist kein Stromausfall. OS- und Datenträgercaches bleiben bei diesem Test vorhanden. Der Versuch belegt weder Schutz gegen physische Datenträgerfehler noch eine absolute Zusage gegen jeden Datenverlust. Originale Nutzeraufnahmen wurden nicht benutzt oder verändert.\n");
        output.AppendLine($"Wiederhergestellte Testsitzung: `{session}`. Rohdaten: `memory.jsonl`, `plan.json`, `result.json`, `worker-crash-*.json`, `worker-recovered-*.json`, `host-crash-ready.json`. Das Testprogramm wird durch `tested-binary.sha256` identifiziert.");
        return output.ToString();
    }
}
