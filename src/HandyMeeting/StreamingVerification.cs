using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using NAudio.CoreAudioApi;
using NAudio.Wave;

namespace HandyMeeting;

public static class StreamingVerification
{
    private const string Phrase = "Dies ist ein lokaler Test. Am Montag besprechen wir den Projektplan. Die Aufnahme bleibt auf diesem Computer.";
    private static async Task<(HandyModel Model, string Path)> ModelAsync()
    {
        var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
        var installed = await new HandyClient(handy).ModelsAsync(CancellationToken.None);
        var model = installed.FirstOrDefault(m => m.SupportsStreaming && m.Id.Contains("nemotron-3.5")) ?? installed.First(m => m.SupportsStreaming);
        return (model, ModelPaths.Resolve(model, handy));
    }
    internal static string Speech(string root, string phrase = Phrase)
    {
        var path = Path.Combine(root, "synthetic-speech.wav");
        dynamic voice = Activator.CreateInstance(Type.GetTypeFromProgID("SAPI.SpVoice")!)!;
        dynamic stream = Activator.CreateInstance(Type.GetTypeFromProgID("SAPI.SpFileStream")!)!;
        try
        {
            dynamic german = voice.GetVoices("Language=407"); if (german.Count > 0) voice.Voice = german.Item(0);
            stream.Format.Type = 18; stream.Open(path, 3, false); voice.AudioOutputStream = stream; voice.Rate = -1; voice.Speak(phrase); stream.Close();
        }
        finally { Marshal.FinalReleaseComObject(stream); Marshal.FinalReleaseComObject(voice); }
        return path;
    }
    private static byte[] SpeechPcm(string path)
    {
        using var reader = new WaveFileReader(path);
        var bytes = new byte[checked((int)reader.Length)]; reader.ReadExactly(bytes); return bytes;
    }
    private static void Require(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    private static async Task WaitUntilAsync(Func<bool> condition, TimeSpan timeout)
    {
        var watch = Stopwatch.StartNew();
        while (!condition()) { if (watch.Elapsed > timeout) throw new TimeoutException("Prüfbedingung nicht rechtzeitig erreicht."); await Task.Delay(100); }
    }
    public static Task ChecksAsync(string root)
    {
        var checks = new List<string>();
        void Check(bool condition, string message) { Require(condition, message); checks.Add(message); }
        var run = Path.Combine(root, "run-" + Guid.NewGuid().ToString("N")[..10]);
        var session = SessionStore.Create(run, "Journal-Prüfung", "stream-model", new("fixture", "Synthetisch"), null, 1);
        using (var writer = new PcmTimeline(Path.Combine(session.Folder, "audio"), "system", 1, session.AddPart, p => session.AudioCheckpoint(p, false)))
        {
            writer.Append(new byte[3200], 0); writer.Complete(1600);
        }
        var part = session.PartsSnapshot().Single();
        session.PersistText(part, "stream-model", new() { Full = "Gesicherter erster Satz", Committed = "Gesicherter erster", Tentative = "Satz" }, 1600, false);
        File.AppendAllText(session.JournalPath, "{unterbrochen");
        File.WriteAllText(session.ManifestPath, "{kaputte Sitzungsübersicht");
        var restored = SessionStore.Open(session.ManifestPath);
        Check(restored.PartsSnapshot().Single().Text == "Gesicherter erster Satz", "Textjournal restores confirmed and tentative text despite a damaged manifest");
        Check(Directory.EnumerateFiles(restored.Folder, "session.json.damaged-*").Any(), "Damaged manifests are retained for inspection");
        restored.PersistText(restored.PartsSnapshot().Single(), "stream-model", new() { Full = "Gesicherter zweiter Satz", Committed = "Gesicherter zweiter Satz" }, 1600, true);
        var again = SessionStore.Open(restored.ManifestPath);
        Check(again.PartsSnapshot().Single().Text == "Gesicherter zweiter Satz", "Appending after a torn journal tail preserves later checkpoints");
        Check(File.ReadAllText(again.JournalPath).Contains("Gesicherter erster Satz"), "Previous text revisions remain in the append-only journal");
        again.PersistText(again.PartsSnapshot().Single(), "another-model", new() { Full = "Neue Modellfassung", Committed = "Neue Modellfassung" }, 1600, true);
        Check(Directory.EnumerateFiles(Path.Combine(again.Folder, "text-history"), "*.json").Any(), "Changing models archives the previous transcript before replacement");
        using (again.AcquireLease())
        {
            try { SessionStore.Open(again.ManifestPath); throw new InvalidOperationException("Aktive Sitzung ließ sich parallel öffnen."); }
            catch (IOException) { checks.Add("Active recordings cannot be repaired or overwritten by a second opener"); }
        }
        Check(SessionHistoryItem.Read(run).Count == 1, "Session history lists a recovered session");
        var longSession = SessionStore.Create(run, "Zwei-Stunden-Zeitachse", "stream-model", new("fixture", "Synthetisch"), null);
        Require(new DriveInfo(Path.GetPathRoot(run)!).AvailableFreeSpace > 700_000_000, "Für den Zwei-Stunden-Dateitest sind mindestens 700 MB freier Platz erforderlich.");
        foreach (var source in new[] { "system", "microphone" })
        {
            using var writer = new PcmTimeline(Path.Combine(longSession.Folder, "audio"), source, 30, longSession.AddPart);
            writer.Complete(2L * 60 * 60 * PcmTimeline.SampleRate);
        }
        var pieces = longSession.PartsSnapshot();
        Check(pieces.Count == 480 && pieces.Sum(p => p.SampleCount) == 4L * 60 * 60 * PcmTimeline.SampleRate, "Two-hour dual-track timeline remains split into 480 bounded audio files");
        var audioBytes = pieces.Sum(p => new FileInfo(longSession.AudioPath(p)).Length - 44);
        Check(audioBytes == 460_800_000, "Two-hour dual-track PCM byte count is exact");
        var longRecovered = SessionStore.Open(longSession.ManifestPath);
        Check(longRecovered.PartsSnapshot().Count == 480, "All two-hour audio segments are discoverable on recovery");
        JsonData.Save(Path.Combine(root, "streaming-checks.json"), new { passed = checks.Count, checks, simulatedDurationHours = 2, audioBytes, longSession = longSession.Folder });
        return Task.CompletedTask;
    }
    public static async Task SmokeAsync(string root, string? outputId, bool killWorker = false)
    {
        var (model, path) = await ModelAsync(); var speech = Speech(root);
        if (killWorker)
        {
            var pcm = SpeechPcm(speech); var fixtureRoot = Path.Combine(root, "continuous-fixture");
            using (var writer = new PcmTimeline(fixtureRoot, "system", 30, _ => { }))
            {
                writer.Append(pcm, 0); writer.Append(pcm, pcm.Length / 2 + PcmTimeline.SampleRate);
                writer.Complete(pcm.Length + PcmTimeline.SampleRate);
            }
            speech = Path.Combine(fixtureRoot, "system-000000000000.wav");
        }
        var outputs = AudioDevices.List(DataFlow.Render); var output = outputId is null ? outputs.First() : outputs.Single(d => d.Id == outputId);
        var session = SessionStore.Create(Path.Combine(root, "sessions"), "Streaming-Funktionstest", model.Id, output, null);
        using (session.AcquireLease())
        {
            var capture = new CaptureController(session); await capture.StartAsync(output, null);
            await using var streaming = new StreamingCoordinator(session, model.Id, path, true); streaming.Start();
            var liveBeforeStop = false;
            var audioContinuedAfterWorkerKill = false;
            var killed = false; long savedAtKill = 0;
            try
            {
                await Task.Delay(1000);
                using var enumerator = new MMDeviceEnumerator(); using var device = enumerator.GetDevice(output.Id);
                using var reader = new WaveFileReader(speech); await using var player = new WasapiPlayerBuilder().WithDevice(device).Build();
                player.Init(reader); player.Volume = 0.9f; player.Play();
                while (player.PlaybackState == PlaybackState.Playing)
                {
                    liveBeforeStop |= session.PartsSnapshot().Any(p => !string.IsNullOrWhiteSpace(p.Text));
                    if (killWorker && liveBeforeStop && !killed)
                    { savedAtKill = session.SavedAudioMs; streaming.KillWorkerForTest(); killed = true; }
                    await Task.Delay(100);
                }
                await Task.Delay(1500); liveBeforeStop |= session.PartsSnapshot().Any(p => !string.IsNullOrWhiteSpace(p.Text));
                if (killWorker)
                {
                    audioContinuedAfterWorkerKill = killed && session.SavedAudioMs > savedAtKill + 5000 && !session.Data.CaptureIncomplete;
                }
            }
            finally { await capture.StopAsync(); }
            await streaming.FinishAsync();
            var text = string.Join(" ", session.PartsSnapshot().Select(p => p.Text));
            Require(liveBeforeStop, "Kein Text vor dem Beenden der Aufnahme.");
            Require(new[] { "lokaler", "Montag", "Projektplan" }.Count(w => text.Contains(w, StringComparison.OrdinalIgnoreCase)) >= 2, "Streaming-Mitschnitt passt nicht zur Teststimme.");
            Require(session.Data.Status == "complete" && (killWorker ? streaming.ModelLoadCount >= 2 && audioContinuedAfterWorkerKill : streaming.ModelLoadCount == 1), "Modellneustart oder fortlaufende Audiosicherung hat nicht funktioniert.");
            JsonData.Save(Path.Combine(root, "streaming-smoke.json"), new { success = true, liveBeforeStop, backend = streaming.Backend, modelLoadCount = streaming.ModelLoadCount, workerRestarts = streaming.RestartCount, audioContinuedAfterWorkerKill, session = session.Folder, transcript = text, microphoneRecorded = false });
        }
    }
    public static async Task CrashHostAsync(string root)
    {
        var (model, path) = await ModelAsync(); var speech = SpeechPcm(Speech(root));
        var session = SessionStore.Create(Path.Combine(root, "sessions"), "Absturzprüfung", model.Id, new("fixture", "Synthetisch"), null);
        using var lease = session.AcquireLease();
        await using var streaming = new StreamingCoordinator(session, model.Id, path, true); streaming.Start();
        using var writer = new PcmTimeline(Path.Combine(session.Folder, "audio"), "system", 30, session.AddPart, p => session.AudioCheckpoint(p, false));
        var ready = false;
        for (long sample = 0; ; sample += 8000)
        {
            var block = new byte[16_000];
            for (var i = 0; i < block.Length; i++) block[i] = speech[(sample * 2 + i) % speech.Length];
            writer.Append(block, sample); writer.Checkpoint();
            if (!ready && session.PartsSnapshot().Any(p => !string.IsNullOrWhiteSpace(p.Text)))
            {
                JsonData.Save(Path.Combine(root, "crash-ready.json"), new { session = session.Folder, model = model.Id, modelPath = path, savedMs = session.SavedAudioMs, sequence = session.Data.SnapshotSequence }); ready = true;
            }
            await Task.Delay(100);
        }
    }
    public static async Task RecoveryAsync(string root)
    {
        var childRoot = Path.Combine(root, "crash-host"); Directory.CreateDirectory(childRoot);
        var start = new ProcessStartInfo(Environment.ProcessPath!) { UseShellExecute = false, CreateNoWindow = true };
        start.ArgumentList.Add("--crash-host"); start.ArgumentList.Add(childRoot);
        using var child = Process.Start(start) ?? throw new InvalidOperationException("Absturz-Testprozess startet nicht.");
        var readyPath = Path.Combine(childRoot, "crash-ready.json");
        try { await WaitUntilAsync(() => File.Exists(readyPath) || child.HasExited, TimeSpan.FromMinutes(2)); }
        catch { if (!child.HasExited) child.Kill(true); throw; }
        Require(File.Exists(readyPath), "Testprozess hat keinen gespeicherten Live-Text erzeugt.");
        using var ready = JsonDocument.Parse(File.ReadAllText(readyPath));
        var folder = ready.RootElement.GetProperty("session").GetString()!;
        var savedMs = ready.RootElement.GetProperty("savedMs").GetInt64();
        byte[] before;
        using (var journal = new FileStream(Path.Combine(folder, "transcript-journal.jsonl"), FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
        { before = new byte[checked((int)journal.Length)]; journal.ReadExactly(before); }
        child.Kill(true); await child.WaitForExitAsync();
        var recovered = SessionStore.Open(Path.Combine(folder, "session.json"));
        Require(recovered.SavedAudioMs >= savedMs, "Bereits bestätigtes Audio fehlt nach dem Prozessabbruch.");
        Require(recovered.PartsSnapshot().Any(p => !string.IsNullOrWhiteSpace(p.Text)), "Gespeicherter Live-Text fehlt nach dem Prozessabbruch.");
        var model = ready.RootElement.GetProperty("model").GetString()!; var path = ready.RootElement.GetProperty("modelPath").GetString()!;
        using (recovered.AcquireLease())
        {
            await using var resumed = new StreamingCoordinator(recovered, model, path, false); resumed.Start(); await resumed.FinishAsync();
            Require(recovered.Data.Status == "complete", "Wiederhergestellte Sitzung wurde nicht fertig transkribiert.");
            var after = File.ReadAllBytes(recovered.JournalPath);
            Require(after.AsSpan(0, before.Length).SequenceEqual(before), "Vor dem Absturz gespeicherte Textstände wurden verändert.");
        }
        // Reprocess two distinct audio tracks with one loaded model and kill only its worker.
        var dual = SessionStore.Create(Path.Combine(root, "sessions"), "Zwei Spuren und Worker-Neustart", model, new("fixture", "Synthetisch"), new("fixture-mic", "Synthetische Mikrofonspur"), 10);
        var speech = SpeechPcm(Speech(root));
        var micRoot = Path.Combine(root, "microphone-fixture"); Directory.CreateDirectory(micRoot);
        var micSpeech = SpeechPcm(Speech(micRoot, "Das Mikrofon nimmt meine Stimme auf. Heute kaufen wir Bananen und Äpfel. Diese Spur bleibt getrennt."));
        foreach (var source in new[] { "system", "microphone" })
        {
            using var writer = new PcmTimeline(Path.Combine(dual.Folder, "audio"), source, 10, dual.AddPart);
            var sourcePcm = source == "system" ? speech : micSpeech;
            for (long offset = 0; offset < 45L * PcmTimeline.SampleRate; offset += sourcePcm.Length / 2)
                writer.Append(sourcePcm.AsSpan(0, (int)Math.Min(sourcePcm.Length, (45L * PcmTimeline.SampleRate - offset) * 2)), offset);
            writer.Complete(45L * PcmTimeline.SampleRate);
        }
        using (dual.AcquireLease())
        {
            await using var streaming = new StreamingCoordinator(dual, model, path, false); streaming.Start();
            await WaitUntilAsync(() => dual.PartsSnapshot().Any(p => !string.IsNullOrWhiteSpace(p.Text)), TimeSpan.FromMinutes(2));
            streaming.KillWorkerForTest(); await streaming.FinishAsync();
            Require(streaming.RestartCount > 0 && dual.Data.Status == "complete", "ASR-Worker wurde nicht aus dem gesicherten Audio wiederhergestellt.");
            Require(dual.PartsSnapshot().Where(p => p.Source == "system").All(p => p.Status == "complete") && dual.PartsSnapshot().Where(p => p.Source == "microphone").All(p => p.Status == "complete"), "Eine der zwei Spuren ist unvollständig.");
            var systemText = string.Join(" ", dual.PartsSnapshot().Where(p => p.Source == "system").Select(p => p.Text));
            var micText = string.Join(" ", dual.PartsSnapshot().Where(p => p.Source == "microphone").Select(p => p.Text));
            Require(systemText.Contains("Projektplan", StringComparison.OrdinalIgnoreCase) && !systemText.Contains("Bananen", StringComparison.OrdinalIgnoreCase)
                && micText.Contains("Bananen", StringComparison.OrdinalIgnoreCase) && !micText.Contains("Projektplan", StringComparison.OrdinalIgnoreCase), "Die beiden Streaming-Spuren wurden vermischt oder nicht korrekt erkannt.");
            JsonData.Save(Path.Combine(root, "streaming-recovery.json"), new { success = true, savedAudioSurvivedProcessKill = true, textJournalPrefixPreserved = true, recoveredSession = recovered.Folder, dualTrackSession = dual.Folder, workerRestarts = streaming.RestartCount, modelLoads = streaming.ModelLoadCount });
        }
    }
}
