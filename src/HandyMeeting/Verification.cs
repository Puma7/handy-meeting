using System.Buffers.Binary;
using System.IO;
using System.Runtime.InteropServices;
using NAudio.CoreAudioApi;
using NAudio.Wave;

namespace HandyMeeting;

/// <summary>Explicit diagnostic modes; never records a microphone automatically.</summary>
public static class Verification
{
    public static async Task RunAsync(string[] args)
    {
        var root = Path.GetFullPath(args[1]); Directory.CreateDirectory(root);
        switch (args[0])
        {
            case "--self-test": await SelfTestAsync(root); break;
            case "--diagnose":
                var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
                JsonData.Save(Path.Combine(root, "diagnostics.json"), new { handy, outputs = AudioDevices.List(DataFlow.Render), microphones = AudioDevices.List(DataFlow.Capture), models = await new HandyClient(handy).ModelsAsync(CancellationToken.None) });
                break;
            case "--smoke-test": await SmokeTestAsync(root, args.Length > 2 ? args[2] : null); break;
            case "--stream-probe": await StreamingProbeAsync(root, args.Length > 2 ? args[2] : null); break;
            case "--stream-probe-cpu": await StreamingProbeAsync(root, args.Length > 2 ? args[2] : null, true); break;
            case "--streaming-checks": await StreamingVerification.ChecksAsync(root); break;
            case "--memory-profile": await MemoryVerification.RunAsync(root, args.Length > 2 && args[2] == "cpu", args.Length > 3 ? int.Parse(args[3]) : 12); break;
            case "--soak-test": await RealtimeSoakTest.RunAsync(root, args.Length > 2 ? int.Parse(args[2]) : 7207); break;
            case "--soak-host": await RealtimeSoakTest.HostAsync(root); break;
            case "--soak-monitor-checks": await RealtimeSoakTest.MonitorChecksAsync(root); break;
            case "--soak-recover-interrupted": await RealtimeSoakTest.RecoverInterruptedAsync(root); break;
            case "--notes-checks": await NotesVerification.ChecksAsync(root); break;
            case "--agent-checks": await AgentVerification.RunAsync(root); break;
            case "--template-checks": await TemplateVerification.ChecksAsync(root); break;
            case "--template-crash-host": await TemplateVerification.CrashHostAsync(root); break;
            case "--template-local-smoke": await TemplateVerification.LocalAsync(root, args.Length > 2 ? args[2] : ""); break;
            case "--notes-crash-host": await NotesVerification.CrashHostAsync(root); break;
            case "--notes-local-smoke": await NotesVerification.LocalSmokeAsync(root, args.Length > 2 ? args[2] : ""); break;
            case "--streaming-smoke": await StreamingVerification.SmokeAsync(root, args.Length > 2 ? args[2] : null); break;
            case "--live-worker-crash": await StreamingVerification.SmokeAsync(root, args.Length > 2 ? args[2] : null, true); break;
            case "--streaming-recovery": await StreamingVerification.RecoveryAsync(root); break;
            case "--crash-host": await StreamingVerification.CrashHostAsync(root); break;
            default: throw new ArgumentException("Unbekannter Diagnosemodus.");
        }
    }
    private static async Task StreamingProbeAsync(string root, string? audioPath, bool cpuOnly = false)
    {
        var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
        var models = await new HandyClient(handy).ModelsAsync(CancellationToken.None);
        var model = models.First(m => m.SupportsStreaming && m.Id.Contains("nemotron-3.5"));
        var modelPath = ModelPaths.Resolve(model, handy);
        var watch = System.Diagnostics.Stopwatch.StartNew();
        await using var worker = await StreamingProcess.StartAsync(modelPath, cpuOnly, CancellationToken.None);
        var loadMs = watch.ElapsedMilliseconds;
        var updates = new List<object>(); StreamReply? last = null;
        if (audioPath is not null)
        {
            var (_, _) = PcmTimeline.InspectAndRepair(audioPath);
            var samples = (new FileInfo(audioPath).Length - 44) / 2;
            await worker.CallAsync(new() { Operation = "begin" }, CancellationToken.None);
            for (long offset = 0; offset < samples; offset += 8000)
            {
                last = await worker.CallAsync(new() { Operation = "feed", AudioPath = audioPath, OffsetSamples = offset, SampleCount = (int)Math.Min(8000, samples - offset) }, CancellationToken.None);
                if (!string.IsNullOrWhiteSpace(last.Full)) updates.Add(new { offset, elapsedMs = watch.ElapsedMilliseconds, last.Full, last.Committed, last.Tentative });
            }
            last = await worker.CallAsync(new() { Operation = "finalize" }, CancellationToken.None);
        }
        JsonData.Save(Path.Combine(root, "stream-probe.json"), new { backend = worker.Backend, loadMs, totalMs = watch.ElapsedMilliseconds, final = last?.Full, updates });
    }
    private static byte[] Signal(int samples, short amplitude = 2000)
    {
        var data = new byte[samples * 2];
        for (var i = 0; i < samples; i++) BinaryPrimitives.WriteInt16LittleEndian(data.AsSpan(i * 2), amplitude);
        return data;
    }
    private static async Task SelfTestAsync(string root)
    {
        var reportRoot = root;
        root = Path.Combine(root, "run-" + Guid.NewGuid().ToString("N")[..10]);
        Directory.CreateDirectory(root);
        var passed = new List<string>();
        void Check(bool condition, string name) { if (!condition) throw new InvalidOperationException("Test fehlgeschlagen: " + name); passed.Add(name); }
        var parts = new List<AudioPart>(); var folder = Path.Combine(root, "timeline");
        using (var timeline = new PcmTimeline(folder, "system", 1, parts.Add))
        {
            timeline.Append(Signal(4000), 4000); // 0.25 s of silence, then 0.25 s signal.
            timeline.Append(Signal(1600, 1000), 7200); // First 800 samples overlap and must be dropped.
            timeline.Complete(40_000); // Exactly 2.5 seconds, including silent loopback intervals.
        }
        Check(parts.Count == 3 && parts.Sum(p => p.SampleCount) == 40_000, "Chunk boundaries preserve exact duration");
        var bytes = File.ReadAllBytes(Path.Combine(folder, parts[0].File)).AsSpan(44).ToArray();
        Check(bytes.AsSpan(0, 8000).ToArray().All(b => b == 0), "Initial loopback silence is retained");
        Check(BinaryPrimitives.ReadInt16LittleEndian(bytes.AsSpan(8000)) == 2000, "Speech begins at its original sample offset");
        Check(BinaryPrimitives.ReadInt16LittleEndian(bytes.AsSpan(15_998)) == 2000 && BinaryPrimitives.ReadInt16LittleEndian(bytes.AsSpan(16_000)) == 1000, "Overlapping packets do not duplicate audio");
        Check(parts[0].HasSignal && !parts[1].HasSignal && !parts[2].HasSignal, "Silent chunks are identified without ASR");
        var quiet = new List<AudioPart>();
        using (var quietWriter = new PcmTimeline(Path.Combine(root, "quiet"), "system", 1, quiet.Add))
        { quietWriter.Append(Signal(160, 1), 0); quietWriter.Complete(160); }
        Check(quiet.Single().HasSignal, "Quiet nonzero audio is retained for recognition instead of being discarded as silence");
        var locator = new PacketPosition(); const long origin = 10_000_000;
        Check(locator.Locate(origin, 0, origin + 100_000, 160, true, 0) == 0
            && locator.Locate(origin, 0, origin + 100_001, 160, true, 160) == 160
            && locator.Locate(origin, 0, origin + 100_002, 160, true, 320) == 320, "Timestamp fallback preserves every queued audio packet");
        Check(locator.Locate(origin, 0, origin + 10_000_000, 160, true, 480) == 15_840, "Timestamp fallback retains real pauses between packets");
        using (var microphone = new PcmTimeline(folder, "microphone", 1, _ => { })) { microphone.Append(Signal(1600, -3000), 0); microphone.Complete(1600); }
        Check(BinaryPrimitives.ReadInt16LittleEndian(File.ReadAllBytes(Path.Combine(folder, "microphone-000000000000.wav")).AsSpan(44)) == -3000
            && BinaryPrimitives.ReadInt16LittleEndian(bytes.AsSpan(8000)) == 2000, "Microphone and system tracks remain separate");

        var recovery = SessionStore.Create(Path.Combine(root, "sessions"), "Recovery", "model", new("device", "Test"), null, 1);
        var partial = Path.Combine(recovery.Folder, "audio", "system-000000000000.wav");
        using (var stream = File.Create(partial)) { PcmTimeline.WriteHeader(stream, 0); stream.Write(Signal(1600)); }
        var recovered = SessionStore.Open(recovery.ManifestPath);
        Check(recovered.Data.Parts.Count == 1 && recovered.Data.Parts[0].SampleCount == 1600 && recovered.Data.CaptureIncomplete, "Interrupted capture is recovered and marked incomplete");
        Check(BitConverter.ToUInt32(File.ReadAllBytes(partial), 40) == 3200, "Unfinalized WAV header is repaired from saved bytes");
        Check(SessionStore.Open(recovery.ManifestPath).Data.Parts.Count == 1, "Recovery does not create duplicate chunks");
        try { _ = recovered.AudioPath(new() { File = "..\\outside.wav" }); Check(false, "Path traversal rejected"); }
        catch (InvalidDataException) { passed.Add("Manifest cannot escape the audio directory"); }

        var silentSession = SessionStore.Create(Path.Combine(root, "sessions"), "Silence", "test-model", new("device", "Test"), null, 1);
        using (var silence = new PcmTimeline(Path.Combine(silentSession.Folder, "audio"), "system", 1, silentSession.AddPart)) silence.Complete(16_000);
        silentSession.Data.DurationMs = 1000;
        await Transcription.RunAsync(silentSession, new HandyClient(Path.Combine(root, "does-not-exist.exe")), "test-model", _ => { }, CancellationToken.None);
        Check(silentSession.Data.Status == "complete" && silentSession.Data.Parts.All(p => p.Text == ""), "Digital silence never launches Handy");
        var cached = silentSession.Data.Parts[0]; cached.HasSignal = true; cached.Text = "Bereits verarbeitet.";
        await Transcription.RunAsync(silentSession, new HandyClient(Path.Combine(root, "does-not-exist.exe")), "test-model", _ => { }, CancellationToken.None);
        Check(cached.Text == "Bereits verarbeitet.", "Retry reuses completed chunks for the same model");
        try { await Transcription.RunAsync(silentSession, new HandyClient(Path.Combine(root, "does-not-exist.exe")), "new-model", _ => { }, CancellationToken.None); Check(false, "Expected local process failure"); }
        catch (FileNotFoundException) { Check(silentSession.Data.Status == "error" && File.Exists(silentSession.AudioPath(cached)), "Failed transcription preserves audio and retry status"); }
        using (var cancellation = new CancellationTokenSource())
        {
            cancellation.Cancel();
            try { await Transcription.RunAsync(silentSession, new HandyClient("unused"), "new-model", _ => { }, cancellation.Token); Check(false, "Expected cancellation"); }
            catch (OperationCanceledException) { Check(silentSession.Data.Status == "recorded" && cached.Status == "pending", "Cancellation leaves recoverable pending work"); }
        }
        var exported = File.ReadAllText(Path.Combine(silentSession.Folder, "transcript.txt"));
        Check(exported.Contains("UNVOLLSTÄNDIG") && exported.Contains("keine exakten Wortzeiten"), "Partial exports disclose completeness and timestamp precision");
        SecurityVerification.Run(root, Check);
        JsonData.Save(Path.Combine(reportRoot, "tests.json"), new { passed = passed.Count, checks = passed });
    }
    private static async Task SmokeTestAsync(string root, string? outputId)
    {
        var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
        var client = new HandyClient(handy); var models = await client.ModelsAsync(CancellationToken.None);
        var model = models.Find(m => m.Id == "parakeet-tdt-0.6b-v3") ?? models.Find(m => m.Id == "turbo") ?? models.First();
        var outputs = AudioDevices.List(DataFlow.Render);
        var output = outputId is null ? outputs.First() : outputs.Single(d => d.Id == outputId);
        var synthesized = Path.Combine(root, "synthetic-speech.wav");
        const string phrase = "Dies ist ein lokaler Test. Am Montag besprechen wir den Projektplan. Die Aufnahme bleibt auf diesem Computer.";
        // SAPI synthesis is local and uses an installed Windows voice, with no microphone input.
        var voiceType = Type.GetTypeFromProgID("SAPI.SpVoice") ?? throw new InvalidOperationException("Windows-Sprachausgabe fehlt.");
        var streamType = Type.GetTypeFromProgID("SAPI.SpFileStream") ?? throw new InvalidOperationException("SAPI-Dateiausgabe fehlt.");
        dynamic voice = Activator.CreateInstance(voiceType)!; dynamic stream = Activator.CreateInstance(streamType)!;
        try
        {
            stream.Format.Type = 18; stream.Open(synthesized, 3, false); voice.AudioOutputStream = stream; voice.Rate = -1; voice.Speak(phrase); stream.Close();
        }
        finally { Marshal.FinalReleaseComObject(stream); Marshal.FinalReleaseComObject(voice); }
        var session = SessionStore.Create(Path.Combine(root, "sessions"), "Lokaler Funktionstest", model.Id, output, null, 30);
        var capture = new CaptureController(session);
        await capture.StartAsync(output, null);
        try
        {
            await Task.Delay(1000);
            using var enumerator = new MMDeviceEnumerator(); using var device = enumerator.GetDevice(output.Id);
            using var reader = new WaveFileReader(synthesized);
            await using var player = new WasapiPlayerBuilder().WithDevice(device).Build();
            player.Init(reader); player.Volume = 0.9f; player.Play();
            while (player.PlaybackState == PlaybackState.Playing) await Task.Delay(100);
            await Task.Delay(1000);
        }
        finally { await capture.StopAsync(); }
        if (session.Data.CaptureIncomplete) throw new InvalidOperationException(string.Join("; ", session.Data.Warnings));
        if (!session.Data.Parts.Any(p => p.HasSignal)) throw new InvalidOperationException("Loopback hat kein Signal erfasst.");
        await Transcription.RunAsync(session, client, model.Id, _ => { }, CancellationToken.None);
        var text = string.Join(" ", session.Data.Parts.Select(p => p.Text));
        var recognizedKeywords = new[] { "lokaler", "Montag", "Projektplan" }.Count(word => text.Contains(word, StringComparison.OrdinalIgnoreCase));
        if (recognizedKeywords < 2) throw new InvalidOperationException("Der Mitschnitt stimmt nicht ausreichend mit der Teststimme überein. Andere Programmtöne auf dieser Ausgabe oder die Aufnahme müssen geprüft werden.");
        JsonData.Save(Path.Combine(root, "smoke-test.json"), new { success = true, microphoneRecorded = false, handy, model = model.Id, output = output.Name, session = session.Folder, session.Data.DurationMs, phrase, transcript = text, session.Data.Warnings });
    }
}
