using System.Diagnostics;
using System.IO;
using NAudio.Wave;

namespace HandyMeeting;

public sealed record MemorySample(long PrivateBytes, long WorkingBytes, long ManagedBytes, long AllocatedBytes, int Gen0, int Gen1, int Gen2)
{
    public static MemorySample Read()
    {
        using var process = Process.GetCurrentProcess();
        return new(process.PrivateMemorySize64, process.WorkingSet64, GC.GetTotalMemory(false), GC.GetTotalAllocatedBytes(), GC.CollectionCount(0), GC.CollectionCount(1), GC.CollectionCount(2));
    }
}

/// <summary>Explicit synthetic profiling: no microphone, speaker playback or user sessions.</summary>
public static class MemoryVerification
{
    public static async Task RunAsync(string root, bool cpuOnly, int windows = 12)
    {
        if (windows is < 1 or > 240) throw new ArgumentOutOfRangeException(nameof(windows));
        var reads = new List<object>();
        foreach (var count in new[] { 4, 480, 2880 })
        {
            var data = new SessionData { ModelId = "profile" };
            for (var i = count - 1; i >= 0; i--) // Intentionally unsorted, as recovered manifests may be.
                data.Parts.Add(new() { File = $"{(i % 2 == 0 ? "system" : "microphone")}-{i / 2 * 480000L:D12}.wav", Source = i % 2 == 0 ? "system" : "microphone", StartSample = i / 2 * 480000L, SampleCount = 480000, ProcessedSamples = i >= count - 2 ? 8000 : 480000, Status = i >= count - 2 ? "pending" : "complete", ModelId = "profile", Text = "Synthetischer Meetingtext für die Speicherprüfung.", HasSignal = true });
            var session = new SessionStore(root, data);
            var expectedRecent = data.Parts.Where(p => !string.IsNullOrWhiteSpace(p.Text)).OrderBy(p => p.StartSample).ThenBy(p => p.Source).TakeLast(16).Select(p => p.File);
            if (!session.RecentTextPartsSnapshot().Select(p => p.File).SequenceEqual(expectedRecent)) throw new InvalidOperationException("Die begrenzte Live-Ansicht hat eine andere Reihenfolge.");
            foreach (var source in new[] { "system", "microphone" })
            {
                var expected = data.Parts.Where(p => p.Source == source && p.Status != "complete").OrderBy(p => p.StartSample).First();
                var actual = session.NextPendingPart(source, "profile")!;
                if (actual.File != expected.File || ReferenceEquals(actual, expected)) throw new InvalidOperationException("Offene Abschnitte sind nicht korrekt isoliert.");
            }
            long Loop(int iterations)
            {
                long checksum = 0;
                for (var i = 0; i < iterations; i++)
                {
                    checksum += session.SavedAudioMs + session.PendingAudioMs("profile");
                    checksum += session.NextPendingPart("system", "profile")!.StartSample;
                    checksum += session.NextPendingPart("microphone", "profile")!.StartSample;
                    checksum += session.AllPartsComplete("profile") ? 1 : 0;
                    if (i % 4 == 0) checksum += Transcription.RenderLive(session).Length;
                }
                return checksum;
            }
            _ = Loop(20);
            var allocated = GC.GetAllocatedBytesForCurrentThread(); var clock = Stopwatch.StartNew();
            var checksum = Loop(1000);
            var bytes = GC.GetAllocatedBytesForCurrentThread() - allocated;
            if (bytes > 12_000_000) throw new InvalidOperationException("Die Abfragen überschreiten das Budget für temporäre Speicherzuweisungen.");
            reads.Add(new { parts = count, simulatedHours = count / 240d, iterations = 1000, allocatedBytes = bytes, elapsedMs = clock.ElapsedMilliseconds, checksum });
        }
        JsonData.Save(Path.Combine(root, "read-profile.json"), reads);

        var handy = HandyClient.FindInstalled() ?? throw new FileNotFoundException("Handy fehlt.");
        var model = (await new HandyClient(handy).ModelsAsync(CancellationToken.None)).First(m => m.SupportsStreaming && m.Id.Contains("nemotron-3.5"));
        var fixture = StreamingVerification.Speech(root);
        using var reader = new WaveFileReader(fixture);
        if (reader.WaveFormat.SampleRate != 16000 || reader.WaveFormat.BitsPerSample != 16 || reader.WaveFormat.Channels != 1) throw new InvalidDataException("Ungültiges Testformat.");
        var speech = new byte[checked((int)reader.Length)]; reader.ReadExactly(speech);
        var pcm = new byte[30 * PcmTimeline.SampleRate * 2];
        for (var offset = 0; offset < pcm.Length; offset += speech.Length + PcmTimeline.SampleRate * 2)
            speech.AsSpan(0, Math.Min(speech.Length, pcm.Length - offset)).CopyTo(pcm.AsSpan(offset));
        var audio = Path.Combine(root, "synthetic-30s.wav");
        using (var output = File.Create(audio)) { PcmTimeline.WriteHeader(output, pcm.Length); output.Write(pcm); }
        await using var worker = await StreamingProcess.StartAsync(ModelPaths.Resolve(model, handy), cpuOnly, CancellationToken.None);
        var samples = new List<object>(); var clock2 = Stopwatch.StartNew(); var recognized = 0;
        async Task Sample(int window)
        {
            var memory = (await worker.CallAsync(new() { Operation = "metrics" }, CancellationToken.None)).Memory;
            samples.Add(new { completedWindows = window, audioMinutesPerTrack = window / 2d, elapsedMs = clock2.ElapsedMilliseconds, parent = MemorySample.Read(), worker = memory });
            JsonData.Save(Path.Combine(root, "memory-profile.json"), new { version = typeof(MemoryVerification).Assembly.GetName().Version?.ToString(), backend = worker.Backend, tracks = 2, samples, recognizedWindows = recognized });
        }
        await Sample(0);
        for (var window = 0; window < windows; window++)
        {
            foreach (var source in new[] { "system", "microphone" })
                await worker.CallAsync(new() { Operation = "begin", Source = source }, CancellationToken.None);
            for (var offset = 0; offset < 30 * PcmTimeline.SampleRate; offset += 8000)
                foreach (var source in new[] { "system", "microphone" })
                    await worker.CallAsync(new() { Operation = "feed", Source = source, AudioPath = audio, OffsetSamples = offset, SampleCount = 8000 }, CancellationToken.None);
            foreach (var source in new[] { "system", "microphone" })
            {
                var result = await worker.CallAsync(new() { Operation = "finalize", Source = source }, CancellationToken.None);
                if (result.Full.Contains("Projektplan", StringComparison.OrdinalIgnoreCase)) recognized++;
            }
            await Sample(window + 1);
        }
        if (recognized != windows * 2) throw new InvalidOperationException("Der synthetische Referenztext wurde nicht in jedem Fenster erkannt.");
    }
}
